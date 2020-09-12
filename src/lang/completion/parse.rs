use crate::lang::ast::{Node, CommandNode, JobListNode, JobNode};
use crate::lang::errors::{error, CrushResult, mandate, argument_error};
use crate::lang::value::{Field, ValueType, Value};
use std::path::PathBuf;
use crate::lang::command::Command;
use crate::lang::data::scope::Scope;
use crate::lang::parser::close_command;
use std::ops::Deref;

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
    Switch(String),
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
            let res = simple_path(p.as_ref())?;
            Ok(res.join(&a.string))
        }
        _ => {
            error("Invalid path")
        }
    }
}

fn find_command_in_expression(exp: &Node, cursor: usize) -> CrushResult<Option<CommandNode>> {
    match exp {
        Node::Assignment(a, _, b) => {
            find_command_in_expression(b, cursor)
        }

        Node::Substitution(j) => {
            if j.location.contains(cursor) {
                Ok(Some(find_command_in_job(j.clone(), cursor)?))
            } else {
                Ok(None)
            }
        }

        Node::Closure(_, joblist) => {
            if joblist.location.contains(cursor) {
                Ok(Some(find_command_in_job_list(joblist.clone(), cursor)?))
            } else {
                Ok(None)
            }
        }

        _ => {
            Ok(None)
        }
    }
}

fn find_command_in_command(ast: CommandNode, cursor: usize) -> CrushResult<CommandNode> {
    for exp in &ast.expressions {
        if let Some(res) = find_command_in_expression(exp, cursor)? {
            return Ok(res);
        }
    }
    Ok(ast)
}

fn find_command_in_job(job: JobNode, cursor: usize) -> CrushResult<CommandNode> {
    for cmd in &job.commands {
        if cmd.location.contains(cursor) {
            return find_command_in_command(cmd.clone(), cursor);
        }
    }
    mandate(job.commands.last(), "Nothing to complete").map(|c| c.clone())
}

fn find_command_in_job_list(ast: JobListNode, cursor: usize) -> CrushResult<CommandNode> {
    for job in &ast.jobs {
        if job.location.contains(cursor) {
            return find_command_in_job(job.clone(), cursor);
        }
    }
    mandate(ast.jobs.last().and_then(|j| j.commands.last().map(|c| c.clone())), "Nothing to complete")
}

fn parse_command_node(node: &Node, scope: &Scope) -> CrushResult<CompletionCommand> {
    match node {
        Node::Label(l) => {
            match scope.get(&l.string)? {
                None => Ok(CompletionCommand::Unknown),
                Some(v) => match v {
                    Value::Command(cmd) => Ok(CompletionCommand::Known(cmd)),
                    _ => Ok(CompletionCommand::Unknown),
                },
            }
        }
        _ => Ok(CompletionCommand::Unknown),
    }
}

pub fn parse(line: &str, cursor: usize, scope: &Scope) -> CrushResult<ParseResult> {
    let ast = crate::lang::parser::ast(&close_command(&line[0..cursor])?)?;

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
            return Ok(
                ParseResult::PartialArgument(
                    PartialCommandResult {
                        command: parse_command_node(cmd, scope)?,
                        previous_arguments: vec![],
                        last_argument: LastArgument::Unknown,
                        last_argument_name: None,
                    }
                )
            );
        }
    } else {
        let c = parse_command_node(&cmd.expressions[0], scope)?;

        let (arg, last_argument_name, argument_complete) = if let Node::Assignment(name, op, value) = cmd.expressions.last().unwrap() {
            if name.location().contains(cursor) {
                (name.clone(), None, true)
            } else {
                if let Node::Label(name) = name.as_ref() {
                    (value.clone(), Some(name.string.clone()), false)
                } else {
                    (value.clone(), None, false)
                }
            }
        } else {
            (Box::from(cmd.expressions.last().unwrap().clone()), None, false)
        };

        if argument_complete {
            match arg.deref() {
                Node::Label(l) => {
                    return Ok(ParseResult::PartialArgument(
                        PartialCommandResult {
                            command: c,
                            previous_arguments: vec![],
                            last_argument: LastArgument::Switch(l.string.clone()),
                            last_argument_name,
                        }));
                }
                _ => {
                    return argument_error("Invalid argument name");
                }
            }
        } else {
            if arg.location().contains(cursor) {
                match arg.as_ref() {
                    Node::Label(l) => {
                        return Ok(ParseResult::PartialArgument(
                            PartialCommandResult {
                                command: c,
                                previous_arguments: vec![],
                                last_argument: LastArgument::Field(vec![l.string.clone()]),
                                last_argument_name,
                            }));
                    }

                    Node::GetAttr(_, _) => {
                        return Ok(ParseResult::PartialArgument(
                            PartialCommandResult {
                                command: c,
                                previous_arguments: vec![],
                                last_argument: LastArgument::Field(simple_attr(arg.as_ref())?),
                                last_argument_name,
                            }));
                    }

                    Node::Path(_, _) => {
                        return Ok(ParseResult::PartialArgument(
                            PartialCommandResult {
                                command: c,
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
            } else {
                return Ok(ParseResult::PartialArgument(
                    PartialCommandResult {
                        command: c,
                        previous_arguments: vec![],
                        last_argument: LastArgument::Unknown,
                        last_argument_name,
                    }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::ast::Location;

    #[test]
    fn find_command_in_substitution_test() {
        let ast = crate::lang::parser::ast("a (b)").unwrap();
        let cmd = find_command_in_job_list(ast, 4).unwrap();
        assert_eq!(cmd.location, Location::new(3, 4))
    }

    #[test]
    fn find_command_in_closure_test() {
        let ast = crate::lang::parser::ast("a {b}").unwrap();
        let cmd = find_command_in_job_list(ast, 4).unwrap();
        assert_eq!(cmd.location, Location::new(3, 4))
    }

    #[test]
    fn find_command_in_complicated_mess_test() {
        let ast = crate::lang::parser::ast("a | b {c:d (e f=g) h=(i j)}").unwrap();
        let cmd = find_command_in_job_list(ast, 25).unwrap();
        assert_eq!(cmd.location, Location::new(22, 25))
    }

    #[test]
    fn find_command_in_operator() {
        let ast = crate::lang::parser::ast("ps | where {^cpu == (max_)}").unwrap();
        let cmd = find_command_in_job_list(ast, 25).unwrap();
        assert_eq!(cmd.location, Location::new(21, 25))
    }
}
