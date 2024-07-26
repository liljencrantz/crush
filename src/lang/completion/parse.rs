use std::cmp::min;
use crate::lang::ast::{node::Node, CommandNode, JobListNode, JobNode};
use crate::lang::errors::{error, CrushResult, mandate, argument_error_legacy, to_crush_error};
use crate::lang::value::{ValueType, Value};
use crate::lang::command::{Command, ArgumentDescription};
use crate::lang::state::scope::Scope;
use std::ops::Deref;
use regex::Regex;
use crate::util::glob::Glob;
use crate::lang::parser::Parser;
use crate::util::escape::unescape;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};

pub enum CompletionCommand {
    Unknown,
    Known(Command),
}

impl Clone for CompletionCommand {
    fn clone(&self) -> Self {
        match self {
            CompletionCommand::Unknown => CompletionCommand::Unknown,
            CompletionCommand::Known(c) => CompletionCommand::Known(c.clone()),
        }
    }
}

#[derive(Clone)]
pub enum LastArgument {
    Unknown,
    Label(String),
    Field(String),
    Member(Value, String),
    File(String, bool),
    QuotedString(String),
    Switch(String),
}

#[derive(Clone)]
pub enum PreviousArgumentValue {
    Value(Value),
    ValueType(ValueType),
}

#[derive(Clone)]
pub struct PreviousArgument {
    pub name: Option<String>,
    pub value: PreviousArgumentValue,
}

#[derive(Clone)]
pub struct PartialCommandResult {
    pub command: CompletionCommand,
    pub previous_arguments: Vec<PreviousArgument>,
    pub last_argument_name: Option<String>,
    pub last_argument: LastArgument,
}

impl PartialCommandResult {
    pub fn last_argument_description(&self) -> Option<&ArgumentDescription> {
        if let CompletionCommand::Known(cmd) = &self.command {
            if let Some(name) = &self.last_argument_name {
                for arg in cmd.arguments() {
                    if &arg.name == name {
                        return Some(arg);
                    }
                }
                None
            } else {
                if false && cmd.arguments().len() == 1 {
                    Some(&cmd.arguments()[0])
                } else {
                    let mut previous_named = HashSet::new();
                    let mut previous_unnamed = 0usize;

                    for arg in &self.previous_arguments {
                        match &arg.name {
                            Some(name) => {
                                previous_named.insert(name.clone());
                            }
                            None => previous_unnamed += 1,
                        }
                    }

                    let mut unnamed_used = 0usize;
                    for arg in cmd.arguments() {
                        if arg.unnamed {
                            return Some(arg);
                        }
                        if previous_named.contains(&arg.name) {
                            continue;
                        } else {
                            unnamed_used += 1;
                        }
                        if previous_unnamed < unnamed_used {
                            return Some(arg);
                        }
                    }

                    None
                }
            }
        } else {
            None
        }
    }

    pub fn last_argument_type(&self) -> ValueType {
        match self.last_argument_description() {
            None => ValueType::Any,
            Some(d) => d.value_type.clone(),
        }
    }
}

#[derive(Clone)]
pub enum ParseResult {
    Nothing,
    PartialLabel(String),
    PartialField(String),
    PartialMember(Value, String),
    PartialFile(String, bool),
    PartialQuotedString(String),
    PartialArgument(PartialCommandResult),
}

impl Display for ParseResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseResult::Nothing => f.write_str("nothing"),
            ParseResult::PartialLabel(l) => {
                f.write_str("label ")?;
                f.write_str(l)
            }
            ParseResult::PartialField(l) => {
                f.write_str("field ")?;
                f.write_str(l)
            }
            ParseResult::PartialMember(_p, m) => {
                f.write_str("member ")?;
                f.write_str(m)
            }
            ParseResult::PartialFile(p, _q) => {
                f.write_str("file ")?;
                f.write_str(p)
            }
            ParseResult::PartialQuotedString(s) => {
                f.write_str("string ")?;
                f.write_str(s)
            }
            ParseResult::PartialArgument(_a) => {
                f.write_str("command")
            }
        }
    }
}

fn find_command_in_expression<'input>(exp: &Node, cursor: usize) -> CrushResult<Option<CommandNode>> {
    match exp {
        Node::Assignment(_, _, _, b) => {
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
    mandate(
        ast.jobs.last()
            .and_then(|j| j.commands.last()
                .map(|c| c.clone())), "Nothing to complete")
}

fn fetch_value(node: &Node, scope: &Scope, is_command: bool) -> CrushResult<Option<Value>> {
    match node {
        Node::Identifier(l) => scope.get(&l.string),

        Node::Symbol(l) =>
            if is_command {
                scope.get(&l.string)
            } else {
                Ok(None)
            },

        Node::GetAttr(n, l) =>
            match fetch_value(n, scope, is_command)? {
                Some(parent) => parent.field(&l.string),
                None => Ok(None),
            },

        Node::String(s) => Ok(Some(Value::from(s))),

        Node::Integer(s) => Ok(Some(Value::Integer(to_crush_error(
            s.string.replace("_", "").parse::<i128>()
        )?))),

        Node::Float(s) => Ok(Some(Value::Float(to_crush_error(
            s.string.replace("_", "").parse::<f64>()
        )?))),

        Node::Glob(f) =>
            Ok(Some(Value::Glob(Glob::new(&f.string)))),

        Node::Regex(r) =>
            Ok(Some(Value::Regex(
                r.string.clone(),
                to_crush_error(Regex::new(&r.string))?))),

        _ => Ok(None),
    }
}

fn parse_command_node(node: &Node, scope: &Scope) -> CrushResult<CompletionCommand> {
    match fetch_value(node, scope, true)? {
        Some(Value::Command(command)) => Ok(CompletionCommand::Known(command)),
        _ => Ok(CompletionCommand::Unknown),
    }
}

fn parse_previous_argument(arg: &Node) -> PreviousArgument {
    match arg {
        Node::Assignment(key, _, op, value) => {
            match (key.as_ref(), op.as_str()) {
                (Node::Symbol(name), "=") => {
                    let inner = parse_previous_argument(value.as_ref());
                    return PreviousArgument {
                        name: Some(name.string.clone()),
                        value: inner.value,
                    };
                }
                _ => {}
            }
        }

        _ => {}
    }
    PreviousArgument {
        name: None,
        value: PreviousArgumentValue::ValueType(ValueType::Any),
    }
}

pub fn parse(
    line: &str,
    cursor: usize,
    scope: &Scope,
    parser: &Parser,
) -> CrushResult<ParseResult> {
    let ast = parser.ast(&parser.close_command(&line[0..cursor])?)?;

    if ast.jobs.len() == 0 {
        return Ok(ParseResult::Nothing);
    }

    let cmd = find_command_in_job_list(ast, cursor)?;

    match cmd.expressions.len() {
        0 => Ok(ParseResult::Nothing),
        1 => {
            let cmd = &cmd.expressions[0];
            if cmd.location().contains(cursor) {
                match cmd {
                    Node::Identifier(label) =>
                        Ok(ParseResult::PartialLabel(
                            label.prefix(cursor).string)),

                    Node::Symbol(label) =>
                        Ok(ParseResult::PartialField(
                            label.prefix(cursor).string)),

                    Node::GetAttr(parent, field) =>
                        Ok(ParseResult::PartialMember(
                            mandate(fetch_value(parent, scope, true)?, "Unknown value")?,
                            field.prefix(cursor).string)),

                    Node::File(path, quoted) =>
                        Ok(ParseResult::PartialFile(
                            if *quoted { unescape(&path.string)? } else { path.string.clone() },
                            *quoted)),

                    Node::String(string) =>
                        Ok(ParseResult::PartialQuotedString(string.prefix(cursor).string)),

                    Node::GetItem(_, _) => { panic!("AAA"); }

                    _ => error(format!("Can't extract command to complete. Unknown node type {}", cmd.type_name())),
                }
            } else {
                Ok(ParseResult::PartialArgument(
                    PartialCommandResult {
                        command: parse_command_node(cmd, scope)?,
                        previous_arguments: vec![],
                        last_argument: LastArgument::Unknown,
                        last_argument_name: None,
                    }
                ))
            }
        }
        _ => {
            let c = parse_command_node(&cmd.expressions[0], scope)?;

            let previous_arguments = cmd.expressions[1..(cmd.expressions.len() - 1)]
                .iter()
                .map(|arg| parse_previous_argument(arg))
                .collect::<Vec<_>>();
            let (arg, last_argument_name, argument_complete) =
                if let Node::Assignment(name, _, _op, value) = cmd.expressions.last().unwrap() {
                    if name.location().contains(cursor) {
                        (Box::from(name.prefix(cursor)?), None, true)
                    } else {
                        if let Node::Identifier(name) = name.as_ref() {
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
                    Node::Symbol(l) => {
                        let substring = &l.string[0..min(l.string.len(), cursor - l.location.start)];
                        Ok(ParseResult::PartialArgument(
                            PartialCommandResult {
                                command: c,
                                previous_arguments,
                                last_argument: LastArgument::Switch(substring.to_string()),
                                last_argument_name,
                            }
                        ))
                    }

                    _ => argument_error_legacy(format!("Invalid argument name {}", arg.type_name()))
                }
            } else {
                if arg.location().contains(cursor) {
                    match arg.as_ref() {
                        Node::Identifier(l) =>
                            Ok(ParseResult::PartialArgument(
                                PartialCommandResult {
                                    command: c,
                                    previous_arguments,
                                    last_argument: LastArgument::Label(l.string.clone()),
                                    last_argument_name,
                                }
                            )),

                        Node::Symbol(l) =>
                            Ok(ParseResult::PartialArgument(
                                PartialCommandResult {
                                    command: c,
                                    previous_arguments,
                                    last_argument: LastArgument::Field(l.string.clone()),
                                    last_argument_name,
                                }
                            )),

                        Node::GetAttr(parent, field) =>
                            Ok(ParseResult::PartialArgument(
                                PartialCommandResult {
                                    command: c,
                                    previous_arguments,
                                    last_argument: LastArgument::Member(
                                        mandate(fetch_value(parent, scope, false)?, "unknown value")?,
                                        field.prefix(cursor).string),
                                    last_argument_name,
                                })),

                        Node::File(path, quoted) =>
                            Ok(ParseResult::PartialArgument(
                                PartialCommandResult {
                                    command: c,
                                    previous_arguments,
                                    last_argument: LastArgument::File(
                                        if *quoted { unescape(&path.string)? } else { path.string.clone() },
                                        *quoted),
                                    last_argument_name,
                                }
                            )),

                        Node::String(s) =>
                            Ok(ParseResult::PartialArgument(
                                PartialCommandResult {
                                    command: c,
                                    previous_arguments,
                                    last_argument: LastArgument::QuotedString(unescape(&s.string)?),
                                    last_argument_name,
                                }
                            )),

                        _ => error("Can't extract argument to complete"),
                    }
                } else {
                    Ok(ParseResult::PartialArgument(
                        PartialCommandResult {
                            command: c,
                            previous_arguments,
                            last_argument: LastArgument::Unknown,
                            last_argument_name,
                        }
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::ast::location::Location;
    use crate::lang::parser::lalrparser;

    fn ast(s: &str) -> CrushResult<JobListNode> {
        panic!()
//        to_crush_error(lalrparser::JobListParser::new().parse(s))
    }

    #[test]
    fn find_command_in_substitution_test() {
        let ast = ast("a (b)").unwrap();
        let cmd = find_command_in_job_list(ast, 4).unwrap();
        assert_eq!(cmd.location, Location::new(3, 4))
    }

    #[test]
    fn find_command_in_closure_test() {
        let ast = ast("a {b}").unwrap();
        let cmd = find_command_in_job_list(ast, 4).unwrap();
        assert_eq!(cmd.location, Location::new(3, 4))
    }

    #[test]
    fn find_command_in_complicated_mess_test() {
        let ast = ast("a | b {c:d (e f=g) h=(i j)}").unwrap();
        let cmd = find_command_in_job_list(ast, 25).unwrap();
        assert_eq!(cmd.location, Location::new(22, 25))
    }

    #[test]
    fn find_command_in_operator() {
        let ast = ast("ps | where {cpu == (max_)}").unwrap();
        let cmd = find_command_in_job_list(ast, 24).unwrap();
        assert_eq!(cmd.location, Location::new(20, 24))
    }
}
