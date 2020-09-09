use crate::lang::ast::{Node, CommandNode, JobListNode};
use crate::lang::ast::{TokenNode, TokenType};
use crate::lang::parser::tokenize;
use crate::lang::errors::{error, CrushResult, mandate};
use crate::lang::value::{Field, ValueType};
use std::path::PathBuf;
use crate::lang::command::Command;
use crate::lang::data::scope::Scope;

pub enum CompletionCommand {
    Unknown,
    Known(Command),
}

impl Clone for CompletionCommand {
    fn clone(&self) -> Self {
        match self {
            CompletionCommand::Unknown => CompletionCommand::Unknown,
            CompletionCommand::Known(c) => CompletionCommand::Known(c.copy()),
        }
    }
}

#[derive(Clone)]
pub enum LastArgument {
    Unknown,
    Field(Field),
    Path(PathBuf),
    QuotedString(String),
}

#[derive(Clone)]
pub struct PartialCommandResult {
    pub command: CompletionCommand,
    pub previous_arguments: Vec<(Option<String>, ValueType)>,
    pub last_argument_name: Option<String>,
    pub last_argument: LastArgument,
}

#[derive(Clone)]
pub enum ParseResult {
    Nothing,
    PartialCommand(Field),
    PartialPath(PathBuf),
    PartialArgument(PartialCommandResult),
}

fn simple_attr(node: &Node) -> CrushResult<Field> {
    match node {
        Node::Label(label) => Ok(vec![label.string.clone()]),
        Node::GetAttr(p, a) => {
            let mut res = simple_attr(p.as_ref())?;
            res.push(a.string.clone());
            Ok(res)
        }
        _ => {
            error("Invalid path")
        }
    }
}

fn simple_path(node: &Node) -> CrushResult<PathBuf> {
    match node {
        Node::Label(label) => Ok(PathBuf::from(&label.string)),
        Node::Path(p, a) => {
            let mut res = simple_path(p.as_ref())?;
            Ok(res.join(&a.string))
        }
        _ => {
            error("Invalid path")
        }
    }
}

fn find_command_in_job_list(mut ast: JobListNode, cursor: usize) -> CrushResult<CommandNode> {
    for job in &ast.jobs {
        if job.location.contains(cursor) {
            for cmd in &job.commands {
                if cmd.location.contains(cursor) {
                    return Ok(cmd.clone());
                }
            }
        }
    }
    mandate(ast.jobs.last().and_then(|j| j.commands.last().map(|c| c.clone())), "Nothing to complete")
}

pub fn complete_parse(line: &str, cursor: usize, scope: &Scope) -> CrushResult<ParseResult> {
    let ast = crate::lang::parser::ast(&line[0..cursor])?;

    if ast.jobs.len() == 0 {
        return Ok(ParseResult::Nothing);
    }

    let cmd = find_command_in_job_list(ast, cursor)?;

    if cmd.expressions.len() == 0 {
        return Ok(ParseResult::Nothing);
    } else if cmd.expressions.len() == 1 {
        let cmd = &cmd.expressions[0];
        if cmd.location().contains(cursor) {
            match cmd {
                Node::Label(_) |
                Node::GetAttr(_, _) => {
                    return Ok(ParseResult::PartialCommand(simple_attr(cmd)?));
                }
                Node::Path(_, _) => {
                    return Ok(ParseResult::PartialPath(simple_path(cmd)?));
                }
                Node::File(path, _) => { panic!("AAA"); }
                Node::String(string) => { panic!("AAA"); }
                Node::GetItem(_, _) => { panic!("AAA"); }

                _ => { return error("Can't extract command to complete"); }
            }
        } else {
            return Ok(ParseResult::PartialArgument(
                PartialCommandResult {
                    command: CompletionCommand::Unknown,
                    previous_arguments: vec![],
                    last_argument: LastArgument::Unknown,
                    last_argument_name: None,
                }));
        }
    } else {
        let (arg, last_argument_name) = if let Node::Assignment(name, op, value) = cmd.expressions.last().unwrap() {
            if let Node::Label(name) = name.as_ref() {
                (value.clone(), Some(name.string.clone()))
            } else {
                (value.clone(), None)
            }
        } else {
            (Box::from(cmd.expressions.last().unwrap().clone()), None)
        };

        match arg.as_ref() {
            Node::Label(l) => {
                return Ok(ParseResult::PartialArgument(
                    PartialCommandResult {
                        command: CompletionCommand::Unknown,
                        previous_arguments: vec![],
                        last_argument: LastArgument::Field(vec![l.string.clone()]),
                        last_argument_name,
                    }));
            }

            Node::GetAttr(_, _) => {
                return Ok(ParseResult::PartialArgument(
                    PartialCommandResult {
                        command: CompletionCommand::Unknown,
                        previous_arguments: vec![],
                        last_argument: LastArgument::Field(simple_attr(arg.as_ref())?),
                        last_argument_name,
                    }));
            }

            Node::Path(_, _) => {
                return Ok(ParseResult::PartialArgument(
                    PartialCommandResult {
                        command: CompletionCommand::Unknown,
                        previous_arguments: vec![],
                        last_argument: LastArgument::Path(simple_path(arg.as_ref())?),
                        last_argument_name,
                    }));
            }

            Node::String(_) => { error("String completions not yet impemented") }

            _ => {
                error("Can't extract argument to complete")
            }
        }
    }
}
