use crate::lang::errors::CrushResult;
use crate::util::highlight::syntax_highlight;
use markdown::mdast::Node;
use markdown::{ParseOptions, to_mdast};
use std::cmp::{max, min};
use std::collections::HashMap;

static HEADER_START: &str = "\x1b[4m";
static HEADER_END: &str = "\x1b[0m";

static INLINE_CODE_START: &str = "\x1b[32m";
static INLINE_CODE_END: &str = "\x1b[0m";

pub fn render(s: &str, width: usize, colors: HashMap<String, String>) -> CrushResult<String> {
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
            state.out.push_str(INLINE_CODE_START);
            if !state.fits(&n.value) {
                state.newline();
            }
            state.out.push_str(&n.value);
            state.out.push_str(INLINE_CODE_END);
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
    match syntax_highlight(code, &state.colors, &None) {
        Ok(res) => {
            state.out.push_str(&res);
        }
        Err(_) => {
            state.out.push_str(code);
        }
    }
    state.pos = 0;
    Ok(())
}
