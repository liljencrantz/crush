use crate::lang::command::OutputType::Known;
use crate::lang::help::Help;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::state::contexts::CommandContext;
use crate::{CrushResult, Printer};
use markdown::mdast::Node;
use markdown::{ParseOptions, to_mdast};
use signature::signature;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::iter::Map;
use crate::lang::ast::lexer::{Lexer, TokenizerMode};
use crate::lang::ast::lexer::LexerMode::Command;
use crate::lang::ast::token::Token;
use crate::lang::errors::CrushError;

#[signature(
    control.help,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Show help about the specified thing.",
    example = "help $ls",
    example = "help $integer",
    example = "help $help",
)]
pub struct HelpSignature {
    #[description("the topic you want help on.")]
    topic: Option<Value>,
    #[default("terminal")]
    #[description("output format. The default, `terminal`, will render the help directly into the terminal. The other formats return a string containing either an html fragment or markdown."
    )]
    #[values("html", "markdown", "terminal")]
    format: String,
}

static HEADER_START: &str = "\x1b[4m";
static HEADER_END: &str = "\x1b[0m";

static CODE_START: &str = "\x1b[32m";
static CODE_END: &str = "\x1b[0m";

fn render(s: &str, width: usize, colors: HashMap<String, String>) -> CrushResult<String> {
    let tree = to_mdast(s, &ParseOptions::default())?;
    //    println!("{}", s);
    //    println!("{:?}", tree);
    let mut state = State {
        pos: 0,
        width: max(20, min(width, 80)),
        indentation: 0,
        out: String::new(),
        is_list: false,
        named_bullet_width: None,
        colors,
    };
    recurse(tree, &mut state)?;
    Ok(state.out)
}

struct State {
    pos: usize,
    width: usize,
    indentation: usize,
    out: String,
    is_list: bool,
    named_bullet_width: Option<usize>,
    colors: HashMap<String, String>,
}

impl State {
    fn newline(&mut self) {
        self.out.push('\n');
        self.pos = self.indentation;
        self.out.push_str(&" ".repeat(self.indentation));
    }

    fn fits(&mut self, s: &str) -> bool {
        self.pos + s.len() <= self.width
    }
}

fn code_bulletpoint(node: &Node) -> Option<usize> {
    match node {
        Node::ListItem(li) => match (li.children.first(), li.children.len()) {
            (Some(Node::Paragraph(p)), 1) => match p.children.first() {
                Some(Node::InlineCode(c)) => Some(c.value.len()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn recurse(node: Node, state: &mut State) -> CrushResult<()> {
    match node {
        Node::Root(n) => {
            for child in n.children {
                recurse(child, state)?;
            }
        }
        Node::Blockquote(_) => {}
        Node::FootnoteDefinition(_) => {}
        Node::MdxJsxFlowElement(_) => {}
        Node::List(n) => {
            state.newline();
            state.is_list = true;
            let mut w = Some(0usize);
            for child in n.children.iter() {
                if let Some(nw) = code_bulletpoint(child) {
                    w = Some(max(nw, w.unwrap_or(0)));
                } else {
                    w = None;
                    break;
                }
            }

            if w.is_some() {
                state.named_bullet_width = w;
            }
            for child in n.children {
                recurse(child, state)?;
            }
            state.named_bullet_width = None;
            state.is_list = false;
        }
        Node::MdxjsEsm(_) => {}
        Node::Toml(_) => {}
        Node::Yaml(_) => {}
        Node::Break(_) => {}
        Node::InlineCode(n) => {
            state.out.push_str(CODE_START);
            if !state.fits(&n.value) {
                state.newline();
            }
            state.out.push_str(&n.value);
            state.out.push_str(CODE_END);
        }
        Node::InlineMath(_) => {}
        Node::Delete(_) => {}
        Node::Emphasis(_) => {}
        Node::MdxTextExpression(_) => {}
        Node::FootnoteReference(_) => {}
        Node::Html(_) => {}
        Node::Image(_) => {}
        Node::ImageReference(_) => {}
        Node::MdxJsxTextElement(_) => {}
        Node::Link(_) => {}
        Node::LinkReference(_) => {}
        Node::Strong(_) => {}
        Node::Text(n) => {
            let mut first = true;
            for child in n.value.split(&[' ', '\n', '\r', '\t']) {
                if first {
                    first = false;
                } else {
                    state.out.push(' ');
                    state.pos += 1;
                }
                if child.len() == 0 {
                    continue;
                }
                if !state.fits(child) {
                    state.newline();
                }
                state.out.push_str(child);
                state.pos += child.len();
            }
        }
        Node::Code(n) => {
            state.newline();
            syntax_highlight_code(&n.value, state)?;
            //            state.out.push_str(CODE_START);
            //          state.out.push_str(&n.value);
            //        state.out.push_str(CODE_END);
        }
        Node::Math(_) => {}
        Node::MdxFlowExpression(_) => {}
        Node::Heading(n) => {
            state.newline();
            state.out.push_str(HEADER_START);
            for child in n.children {
                recurse(child, state)?;
            }
            state.out.push_str(HEADER_END);
        }
        Node::Table(_) => {}
        Node::ThematicBreak(_) => {}
        Node::TableRow(_) => {}
        Node::TableCell(_) => {}
        Node::ListItem(n) => {
            state.out.push_str(" * ");
            state.pos += 3;
            if let Some(w) = state.named_bullet_width {
                state.indentation = 4 + w;
                for child in n.children {
                    if let Node::Paragraph(p) = child {
                        let mut first = true;
                        for child in p.children {
                            if first {
                                if let Node::InlineCode(c) = child {
                                    let l = c.value.len();
                                    recurse(Node::InlineCode(c), state)?;
                                    state.out.push_str(&" ".repeat(w - l));
                                }
                                first = false;
                            } else {
                                recurse(child, state)?;
                            }
                        }
                    }
                }
                state.indentation = 0;
            } else {
                state.indentation = 3;
                for child in n.children {
                    recurse(child, state)?;
                }
                state.indentation = 0;
            }
            state.newline();
        }
        Node::Definition(_) => {}
        Node::Paragraph(n) => {
            if !state.is_list {
                state.newline();
            }
            for child in n.children {
                recurse(child, state)?;
            }
            if !state.is_list {
                state.newline();
            }
        }
    }
    Ok(())
}

fn syntax_highlight_code(code: &String, state: &mut State) -> CrushResult<()> {
    let mut res = String::new();
    let mut pos = 0;
    let mut new_command = true;
    let mut prev = None;

    let l = Lexer::new(code, Command, TokenizerMode::IncludeComments);
    let tokens = l.into_iter().map(|item| item.map(|it| it.1).map_err(|e| CrushError::from(e))).collect::<CrushResult<Vec<Token>>>()?;

    for tok in tokens {
        res.push_str(&code[pos..min(tok.location().start, code.len())]);
        let mut do_reset = false;

        new_command = match (new_command, tok, prev) {
            (_, Token::BlockStart(_) | Token::Separator(_, _) | Token::Pipe(_), _) => true,
            (true, Token::String(_, _) | Token::Identifier(_, _), Some(Token::String(_, _) | Token::Identifier(_, _))) => false,
            (true, Token::String(_, _) | Token::Identifier(_, _), _) => true,
            (true, Token::MemberOperator(_), Some(Token::String(_, _) | Token::Identifier(_, _))) => true,
            _ => false,
        };

        match get_color(tok, new_command, state) {
            Some(color) => {
                if !color.is_empty() {
                    do_reset = true;
                    res.push_str(color);
                }
            }
            None => {}
        }

        res.push_str(&code[tok.location().start..min(tok.location().end, code.len())]);

        if do_reset {
            res.push_str("\x1b[0m");
        }
        pos = tok.location().end;
        prev = Some(tok);
    }
    state.out.push_str(&res);
    state.pos = 0;
    Ok(())
}

fn get_color<'a>(token_type: Token, new_command: bool, state: &'a State) -> Option<&'a String> {
    use Token::*;
    match token_type {
        String(_, _) | QuotedString(_, _) =>
            if new_command {
                state.colors.get("command")
            } else {
                state.colors.get("string_literal")
            },
        Flag(_, _) => state.colors.get("string_literal"),
        Regex(_, _) => state.colors.get("regex_literal"),
        Glob(_, _) => state.colors.get("glob_literal"),
        Comment(_, _) => state.colors.get("comment"),
        File(_, _) | QuotedFile(_, _) => state.colors.get("file_literal"),
        Float(_, _) | Integer(_, _) => state.colors.get("numeric_literal"),
        Unnamed(_) | Named(_) | Pipe(_) | LogicalOperator(_, _) | UnaryOperator(_, _) |
        ComparisonOperator(_, _) | Equals(_) | Declare(_) | GetItemEnd(_) | GetItemStart(_) | SubEnd(_) |
        Bang(_) | Plus(_) | Minus(_) | Star(_) | Slash(_) | MemberOperator(_) | ExprModeStart(_) |
        SubStart(_) | BlockEnd(_) | BlockStart(_) =>
            state.colors.get("operator"),
        Identifier(_, _) => None,
        Separator(_, _) => None,
        For(_) |
        While(_) |
        Loop(_) |
        If(_) |
        Else(_) |
        Return(_) |
        Break(_) |
        Continue(_) => state.colors.get("keyword"),
    }
}


pub fn help(mut context: CommandContext) -> CrushResult<()> {
    let cfg: HelpSignature =
        HelpSignature::parse(context.remove_arguments(), &context.global_state.printer())?;

    let map: HashMap<String, String> = if let Ok(Value::Dict(highlight)) = context.scope.get_absolute_path(
        vec!["global".to_string(), "crush".to_string(), "highlight".to_string()]) {
        highlight.elements().into_iter().map(|e| (e.0.to_string(), e.1.to_string())).collect()
    } else {
        HashMap::new()
    };

    let s = match cfg.topic {
        None => {
            r#"
# Welcome to Crush!

If this is your first time using Crush, congratulations on just entering your
first command! If you haven't already, you might want to check out the Readme
for an introduction at https://github.com/liljencrantz/crush/.

Call the help command with the name of any value, including a command or a
type in order to get help about it. For example, you might want to run the
commands `help $help`, `help $string`, `help $if` or `help $where`.

To get a list of everything in your namespace, write `var:env`. To list the
members of a value, write `dir <value>`.
"#
        }
        Some(o) => match o.long_help() {
            None => &format!("    {}\n\n{}", o.signature(), o.short_help()),
            Some(long_help) => &format!(
                "    {}\n\n{}\n\n{}",
                o.signature(),
                o.short_help(),
                long_help
            ),
        },
    };

    match cfg.format.as_str() {
        "markdown" => context.output.send(Value::from(s)),
        "html" => context.output.send(Value::from(markdown::to_html(s))),
        "terminal" => {
            context
                .global_state
                .printer()
                .line(&render(s, context.global_state.printer().width(), map)?);
            context.output.send(Value::Empty)
        }
        _ => unreachable!(),
    }
}
