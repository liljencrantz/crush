use crate::lang::command::OutputType::Known;
use crate::lang::help::Help;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::state::contexts::CommandContext;
use crate::{CrushResult, Printer};
use markdown::mdast::Node;
use markdown::{ParseOptions, to_mdast};
use signature::signature;

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
}

static HEADER_START: &str = "\x1b[4m";
static HEADER_END: &str = "\x1b[0m";

static CODE_START: &str = "\x1b[32m";
static CODE_END: &str = "\x1b[0m";

fn render(s: &str, width: usize) -> CrushResult<String> {
    let tree = to_mdast(s, &ParseOptions::default())?;
//    println!("{}", s);
//    println!("{:?}", tree);
    let mut state = State {
        pos: 0,
        width,
        indentation: 0,
        out: String::new(),
        is_list: false,
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
            for child in n.children {
                recurse(child, state)?;
            }
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
                    continue
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
            state.out.push_str(CODE_START);
            state.out.push_str(&n.value);
            state.out.push_str(CODE_END);
            state.newline();
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
            state.indentation += 3;
            state.out.push_str(" * ");
            state.pos += 3;
            for child in n.children {
                recurse(child, state)?;
            }
            state.indentation -= 3;
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

pub fn help(mut context: CommandContext) -> CrushResult<()> {
    let cfg: HelpSignature =
        HelpSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    match cfg.topic {
        None => {
            context.global_state.printer().line(
                render(
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
"#, 80)?.as_str(),
            );
            context.output.send(Value::Empty)
        }
        Some(o) => {
            context.global_state.printer().line(
                match o.long_help() {
                    None => render(&format!("    {}\n\n{}", o.signature(), o.short_help()), 80)?,
                    Some(long_help) => render(
                        &format!(
                            "    {}\n\n{}\n\n{}",
                            o.signature(),
                            o.short_help(),
                            long_help
                        ),
                        80,
                    )?,
                }
                .as_str(),
            );
            context.output.send(Value::Empty)
        }
    }
}
