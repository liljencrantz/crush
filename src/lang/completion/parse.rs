use crate::lang::ast::lexer::LanguageMode;
use crate::lang::ast::node::TextLiteralStyle;
use crate::lang::ast::{CommandNode, JobListNode, JobNode, node::Node};
use crate::lang::command::{Command, Parameter};
use crate::lang::errors::{CrushResult, argument_error_legacy, error};
use crate::lang::parser::Parser;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::util::escape::unescape;
use crate::util::glob::Glob;
use regex::Regex;
use std::cmp::min;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

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
    File(String, TextLiteralStyle),
    QuotedString(String),
    Switch(String),
    Glob(String),
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
    pub fn last_argument_description(&self) -> Option<&Parameter> {
        if let CompletionCommand::Known(cmd) = &self.command {
            if let Some(name) = &self.last_argument_name {
                for arg in cmd.completion_data() {
                    if &arg.name == name {
                        return Some(arg);
                    }
                }
                None
            } else {
                if false && cmd.completion_data().len() == 1 {
                    Some(&cmd.completion_data()[0])
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
                    for arg in cmd.completion_data() {
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
    PartialFile(String, TextLiteralStyle),
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
            ParseResult::PartialArgument(_a) => f.write_str("command"),
        }
    }
}

fn find_command_in_expression<'input>(
    exp: &Node,
    cursor: usize,
) -> CrushResult<Option<CommandNode>> {
    match exp {
        Node::Assignment { value, .. } => find_command_in_expression(value, cursor),

        Node::Substitution(jl) => {
            for j in &jl.jobs {
                if j.location.contains(cursor) {
                    return Ok(Some(find_command_in_job(j.clone(), cursor)?));
                }
            }
            Ok(None)
        }

        Node::Closure(_, joblist) => {
            if joblist.location.contains(cursor) {
                Ok(Some(find_command_in_job_list(joblist.clone(), cursor)?))
            } else {
                Ok(None)
            }
        }

        _ => Ok(None),
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
    Ok(job.commands.last().ok_or("Nothing to complete")?.clone())
}

fn find_command_in_job_list(ast: JobListNode, cursor: usize) -> CrushResult<CommandNode> {
    for job in &ast.jobs {
        if job.location.contains(cursor) {
            return find_command_in_job(job.clone(), cursor);
        }
    }
    Ok(ast
        .jobs
        .last()
        .and_then(|j| j.commands.last().map(|c| c.clone()))
        .ok_or("Nothing to complete")?)
}

fn fetch_value(node: &Node, scope: &Scope, is_command: bool) -> CrushResult<Option<Value>> {
    match node {
        Node::Identifier(l) => scope.get(&l.string),

        Node::String(s, TextLiteralStyle::Quoted) => Ok(Some(Value::from(s))),

        Node::String(l, TextLiteralStyle::Unquoted) => {
            if is_command {
                scope.get(&l.string)
            } else {
                Ok(None)
            }
        }

        Node::GetAttr(n, l) => match fetch_value(n, scope, is_command)? {
            Some(parent) => parent.field(&l.string),
            None => Ok(None),
        },

        Node::Integer(s) => Ok(Some(Value::Integer(
            s.string.replace("_", "").parse::<i128>()?,
        ))),

        Node::Float(s) => Ok(Some(Value::Float(
            s.string.replace("_", "").parse::<f64>()?,
        ))),

        Node::Glob(f) => Ok(Some(Value::Glob(Glob::new(&f.string)))),

        Node::Regex(r) => Ok(Some(Value::Regex(r.string.clone(), Regex::new(&r.string)?))),

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
        Node::Assignment {
            target,
            operation,
            value,
            ..
        } => match (target.as_ref(), operation.as_str()) {
            (Node::String(name, TextLiteralStyle::Unquoted), "=") => {
                let inner = parse_previous_argument(value.as_ref());
                return PreviousArgument {
                    name: Some(name.string.clone()),
                    value: inner.value,
                };
            }
            _ => {}
        },

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
    let ast = parser.ast(
        &parser.close_command(&line[0..cursor])?,
        LanguageMode::Command,
    )?;

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
                    Node::Identifier(label) => {
                        Ok(ParseResult::PartialLabel(label.prefix(cursor).string))
                    }

                    Node::String(string, TextLiteralStyle::Quoted) => Ok(
                        ParseResult::PartialQuotedString(string.prefix(cursor).string),
                    ),

                    Node::String(label, TextLiteralStyle::Unquoted) => {
                        Ok(ParseResult::PartialField(label.prefix(cursor).string))
                    }

                    Node::GetAttr(parent, field) => Ok(ParseResult::PartialMember(
                        fetch_value(parent, scope, true)?.ok_or("Unknown value")?,
                        field.prefix(cursor).string,
                    )),

                    Node::File(path, quoted) => Ok(ParseResult::PartialFile(
                        match quoted {
                            TextLiteralStyle::Quoted => unescape(&path.string)?,
                            TextLiteralStyle::Unquoted => path.string.clone(),
                        },
                        *quoted,
                    )),

                    Node::GetItem(_, _) => {
                        panic!("AAA");
                    }

                    _ => error(format!(
                        "Can't extract command to complete. Unknown node type {}",
                        cmd.type_name()
                    )),
                }
            } else {
                Ok(ParseResult::PartialArgument(PartialCommandResult {
                    command: parse_command_node(cmd, scope)?,
                    previous_arguments: vec![],
                    last_argument: LastArgument::Unknown,
                    last_argument_name: None,
                }))
            }
        }
        _ => {
            let c = parse_command_node(&cmd.expressions[0], scope)?;

            let previous_arguments = cmd.expressions[1..(cmd.expressions.len() - 1)]
                .iter()
                .map(|arg| parse_previous_argument(arg))
                .collect::<Vec<_>>();
            let (arg, last_argument_name, argument_complete) =
                if let Node::Assignment { target, value, .. } = cmd.expressions.last().unwrap() {
                    if target.location().contains(cursor) {
                        (Box::from(target.prefix(cursor)?), None, true)
                    } else {
                        if let Node::Identifier(name) = target.as_ref() {
                            (value.clone(), Some(name.string.clone()), false)
                        } else {
                            (value.clone(), None, false)
                        }
                    }
                } else {
                    (
                        Box::from(cmd.expressions.last().unwrap().clone()),
                        None,
                        false,
                    )
                };

            if argument_complete {
                match arg.deref() {
                    Node::String(l, TextLiteralStyle::Unquoted) => {
                        let substring =
                            &l.string[0..min(l.string.len(), cursor - l.location.start)];
                        Ok(ParseResult::PartialArgument(PartialCommandResult {
                            command: c,
                            previous_arguments,
                            last_argument: LastArgument::Switch(substring.to_string()),
                            last_argument_name,
                        }))
                    }

                    _ => {
                        argument_error_legacy(format!("Invalid argument name {}", arg.type_name()))
                    }
                }
            } else {
                if arg.location().contains(cursor) {
                    match arg.as_ref() {
                        Node::Identifier(l) => {
                            Ok(ParseResult::PartialArgument(PartialCommandResult {
                                command: c,
                                previous_arguments,
                                last_argument: LastArgument::Label(l.string.clone()),
                                last_argument_name,
                            }))
                        }

                        Node::String(l, TextLiteralStyle::Unquoted) => {
                            Ok(ParseResult::PartialArgument(PartialCommandResult {
                                command: c,
                                previous_arguments,
                                last_argument: LastArgument::Field(l.string.clone()),
                                last_argument_name,
                            }))
                        }

                        Node::GetAttr(parent, field) => {
                            Ok(ParseResult::PartialArgument(PartialCommandResult {
                                command: c,
                                previous_arguments,
                                last_argument: LastArgument::Member(
                                    fetch_value(parent, scope, false)?.ok_or("unknown value")?,
                                    field.prefix(cursor).string,
                                ),
                                last_argument_name,
                            }))
                        }

                        Node::File(path, quoted) => {
                            Ok(ParseResult::PartialArgument(PartialCommandResult {
                                command: c,
                                previous_arguments,
                                last_argument: LastArgument::File(
                                    match quoted {
                                        TextLiteralStyle::Quoted => unescape(&path.string)?,
                                        TextLiteralStyle::Unquoted => path.string.clone(),
                                    },
                                    *quoted,
                                ),
                                last_argument_name,
                            }))
                        }

                        Node::String(s, TextLiteralStyle::Quoted) => {
                            Ok(ParseResult::PartialArgument(PartialCommandResult {
                                command: c,
                                previous_arguments,
                                last_argument: LastArgument::QuotedString(unescape(&s.string)?),
                                last_argument_name,
                            }))
                        }

                        Node::Glob(s) => Ok(ParseResult::PartialArgument(PartialCommandResult {
                            command: c,
                            previous_arguments,
                            last_argument: LastArgument::Glob(s.string.clone()),
                            last_argument_name,
                        })),

                        _ => error(format!(
                            "Can't extract argument to complete. Node type {}.",
                            arg.type_name()
                        )),
                    }
                } else {
                    Ok(ParseResult::PartialArgument(PartialCommandResult {
                        command: c,
                        previous_arguments,
                        last_argument: LastArgument::Unknown,
                        last_argument_name,
                    }))
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
