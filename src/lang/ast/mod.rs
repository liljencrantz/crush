use std::fmt::{Display, Formatter, Write};
use crate::lang::argument::{ArgumentDefinition, SwitchStyle};
use crate::lang::command::{Command, Parameter};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::{CrushResult, error, mandate, to_crush_error};
use crate::lang::job::Job;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueDefinition};
use crate::util::glob::Glob;
use regex::Regex;
use std::ops::Deref;
use std::path::PathBuf;
use location::Location;
use tracked_string::TrackedString;
use crate::util::escape::unescape;
use crate::util::user_map::get_user;
use crate::util::user_map::get_current_username;
use node::Node;

pub mod location;
pub mod tracked_string;
pub mod parameter_node;
pub mod lexer;
pub mod token;
pub mod node;

#[derive(Clone, Debug)]
pub struct JobListNode {
    pub jobs: Vec<JobNode>,
    pub location: Location,
}

impl JobListNode {
    pub fn compile(&self, env: &Scope) -> CrushResult<Vec<Job>> {
        self.jobs.iter().map(|j| j.compile(env)).collect()
    }
}

#[derive(Clone, Debug)]
pub struct JobNode {
    pub commands: Vec<CommandNode>,
    pub location: Location,
}

impl JobNode {
    pub fn compile(&self, env: &Scope) -> CrushResult<Job> {
        Ok(Job::new(
            self.commands
                .iter()
                .map(|c| c.compile(env))
                .collect::<CrushResult<Vec<CommandInvocation>>>()?,
            self.location,
        ))
    }

    pub fn to_node(mut self) -> Box<Node> {
        if self.commands.len() == 1 {
            if self.commands[0].expressions.len() == 1 {
                return Box::from(self.commands[0].expressions.remove(0));
            }
        }
        Box::from(Node::Substitution(self))
    }
}

fn operator_function(op: &[&str], op_location: Location, l: Box<Node>, r: Box<Node>) -> Box<Node> {
    let location = op_location.union(l.location()).union(r.location());
    let cmd = attr(op, op_location);
    Box::from(
        Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![
                        cmd, *l, *r,
                    ],
                    location: location,
                }],
                location: location,
            }
        )
    )
}

pub fn operator_method(op: &str, op_location: Location, l: Box<Node>, r: Box<Node>) -> Box<Node> {
    let location = op_location.union(l.location()).union(r.location());
    let cmd = Node::GetAttr(l, TrackedString::new(op, op_location));
    Box::from(
        Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![
                        cmd, *r,
                    ],
                    location: location,
                }],
                location: location,
            }
        )
    )
}

pub fn unary_operator_method(op: &str, op_location: Location, n: Box<Node>) -> Box<Node> {
    let location = op_location.union(n.location());
    let cmd = Node::GetAttr(n, TrackedString::new(op, op_location));
    Box::from(
        Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![
                        cmd,
                    ],
                    location: location,
                }],
                location: location,
            }
        )
    )
}

pub fn operator(iop: impl Into<TrackedString>, l: Box<Node>, r: Box<Node>) -> Box<Node> {
    let op = iop.into();
    match op.string.as_str() {
        "<" => operator_function(&["global", "comp", "lt"], op.location, l, r),
        "<=" => operator_function(&["global", "comp", "lte"], op.location, l, r),
        ">" => operator_function(&["global", "comp", "gt"], op.location, l, r),
        ">=" => operator_function(&["global", "comp", "gte"], op.location, l, r),
        "==" => operator_function(&["global", "comp", "eq"], op.location, l, r),
        "!=" => operator_function(&["global", "comp", "neq"], op.location, l, r),

        "and" => operator_function(&["global", "cond", "__and__"], op.location, l, r),
        "or" => operator_function(&["global", "cond", "__or__"], op.location, l, r),

        "+" => operator_method("__add__", op.location, l, r),
        "-" => operator_method("__sub__", op.location, l, r),

        "*" => operator_method("__mul__", op.location, l, r),
        "//" => operator_method("__div__", op.location, l, r),

        // Note that these operators reverse the arguemnts because the method wxists on the second argument!
        "=~" => operator_method("match", op.location, r, l),
        "!~" => operator_method("not_match", op.location, r, l),

        _ => panic!("Unknown operator {}", &op.string),
    }
}


#[derive(Clone, Debug)]
pub struct CommandNode {
    pub expressions: Vec<Node>,
    pub location: Location,
}

impl CommandNode {
    pub fn compile(&self, env: &Scope) -> CrushResult<CommandInvocation> {
        if let Some(c) = self.expressions[0].compile_as_special_command(env)? {
            if self.expressions.len() == 1 {
                Ok(c)
            } else {
                error("Stray arguments")
            }
        } else {
            let cmd = self.expressions[0].compile_command(env)?;
            let arguments = self.expressions[1..]
                .iter()
                .map(|e| e.compile_argument(env))
                .collect::<CrushResult<Vec<ArgumentDefinition>>>()?;
            Ok(CommandInvocation::new(cmd.unnamed_value()?, arguments))
        }
    }
}

fn propose_name(name: &TrackedString, v: ValueDefinition) -> ValueDefinition {
    match v {
        ValueDefinition::ClosureDefinition(_, p, j, l) =>
            ValueDefinition::ClosureDefinition(Some(name.clone()), p, j, l),
        _ => v,
    }
}

fn attr(parts: &[&str], location: Location) -> Node {
    let mut res = Node::Identifier(TrackedString::new(parts[0], location));
    for part in &parts[1..] {
        res = Node::GetAttr(Box::from(res), TrackedString::new(part, location));
    }
    res
}

fn home_as_string(user: &str) -> CrushResult<String> {
    mandate(get_user(user)?.home.to_str(), "Bad home directory").map(|s| { s.to_string() })
}

fn expand_user(s: String) -> CrushResult<String> {
    if !s.starts_with('~') {
        Ok(s)
    } else {
        let parts: Vec<&str> = s[1..].splitn(2, '/').collect();
        let home = if parts[0].len() > 0 { home_as_string(parts[0]) } else { home_as_string(&get_current_username()?) };
        if parts.len() == 1 {
            home
        } else {
            home.map(|home| { format!("{}/{}", home, parts[1]) })
        }
    }
}


