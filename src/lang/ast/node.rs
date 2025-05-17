use std::ops::Deref;
use std::path::PathBuf;
use regex::Regex;
use crate::lang::argument::{ArgumentDefinition, SwitchStyle};
use crate::lang::ast::{CommandNode, expand_user, JobListNode, JobNode, propose_name};
use crate::lang::ast::location::Location;
use crate::lang::ast::parameter_node::ParameterNode;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::command::{Command, Parameter};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::{CrushResult, error};
use crate::lang::job::Job;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueDefinition};
use crate::util::escape::unescape;
use crate::util::glob::Glob;

/**
A type representing a node in the abstract syntax tree that is the output of parsing a Crush script.
 */
#[derive(Clone, Debug)]
pub enum Node {
    Assignment(Box<Node>, SwitchStyle, String, Box<Node>),
    Unary(TrackedString, Box<Node>),
    Glob(TrackedString),
    Identifier(TrackedString),
    Regex(TrackedString),
    // true if filename is quoted
    String(TrackedString, bool),
    // true if filename is quoted
    File(TrackedString, bool),
    Integer(TrackedString),
    Float(TrackedString),
    GetItem(Box<Node>, Box<Node>),
    GetAttr(Box<Node>, TrackedString),
    Substitution(JobNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode),
}

impl Node {
    pub fn val(l: Location) -> Node {
        Node::GetAttr(
            Box::from(Node::GetAttr(
                Node::global(l),
                TrackedString::new("io", l))),
            TrackedString::new("val", l))
    }

    pub fn list_literal(node: JobListNode) -> Box<Node> {
        let mut cmd = vec![Self::get_attr(&["global", "types", "list", "of"], node.location)];
        for it in node.jobs {
            cmd.push(Node::Substitution(it))
        }

        Box::from(Node::Substitution(JobNode {
            commands: vec![CommandNode {
                expressions: cmd,
                location: node.location,
            }],
            location: node.location,
        }))
    }

    fn id(s: &str, l: Location) -> Box<Node> {
        Box::from(Node::Identifier(TrackedString::new(s, l)))
    }

    fn global(l: Location) -> Box<Node> {
        Node::id("global", l)
    }

    pub fn expression_to_command(self) -> CommandNode {
        let l = self.location();
        match self {
            Node::Substitution(n) if n.commands.len() == 1 => {
                n.commands[0].clone()
            }
            _ => {
                CommandNode {
                    expressions: vec![Node::val(self.location()), self],
                    location: l,
                }
            }
        }
    }

    pub fn expression_to_job(self) -> JobNode {
        let location = self.location();
        match self {
            Node::Substitution(s) => s,
            Node::Assignment(..) => {
                JobNode {
                    commands: vec![CommandNode { expressions: vec![self], location }],
                    location,
                }
            }
            _ => {
                let expressions = vec![Node::val(location), self];
                JobNode {
                    commands: vec![CommandNode { expressions, location }],
                    location,
                }
            }
        }
    }

    pub fn prefix(&self, pos: usize) -> CrushResult<Node> {
        match self {
            Node::Identifier(s) => Ok(Node::Identifier(s.prefix(pos))),
            _ => Ok(self.clone()),
        }
    }

    pub fn location(&self) -> Location {
        use Node::*;

        match self {
            Glob(s) | Identifier(s) |
            String(s, _) | Integer(s) | Float(s) |
            Regex(s) | File(s, _) =>
                s.location,

            Assignment(a, _, _, b) =>
                a.location().union(b.location()),

            Unary(s, a) =>
                s.location.union(a.location()),

            GetItem(a, b) => a.location().union(b.location()),
            GetAttr(p, n) => p.location().union(n.location),
            Substitution(j) => j.location,
            Closure(_, j) => {
                // Fixme: Can't tab complete or error report on parameters because they're not currently tracked
                j.location
            }
        }
    }

    pub fn compile_command(&self, env: &Scope) -> CrushResult<ArgumentDefinition> {
        self.compile(env, true)
    }

    pub fn compile_argument(&self, env: &Scope) -> CrushResult<ArgumentDefinition> {
        self.compile(env, false)
    }

    pub fn type_name(&self) -> &str {
        match self {
            Node::Assignment(_, _, _, _) => "assignment",
            Node::Unary(_, _) => "unary operator",
            Node::Glob(_) => "glob",
            Node::Identifier(_) => "identifier",
            Node::Regex(_) => "regular expression literal",
            Node::String(_, _) => "string literal",
            Node::File(_, _) => "file literal",
            Node::Integer(_) => "integer literal",
            Node::Float(_) => "floating point number literal",
            Node::GetItem(_, _) => "subscript",
            Node::GetAttr(_, _) => "member access",
            Node::Substitution(_) => "command substitution",
            Node::Closure(_, _) => "closure",
        }
    }

    pub fn compile(&self, env: &Scope, is_command: bool) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(match self {
            Node::Assignment(target, style, op, value) => match op.deref() {
                "=" => {
                    return match target.as_ref() {
                        Node::String(t, false) | Node::Identifier(t) => Ok(ArgumentDefinition::named_with_style(
                            t,
                            *style,
                            propose_name(&t, value.compile_argument(env)?.unnamed_value()?),
                        )),
                        _ => error(format!("Invalid left side in named argument. Expected a string or identifier, got a {}", target.type_name())),
                    };
                }
                _ => return error("Invalid assignment operator"),
            },

            Node::GetItem(a, o) => ValueDefinition::JobDefinition(
                Job::new(vec![self
                    .compile_as_special_command(env)?
                    .unwrap()],
                         a.location().union(o.location()),
                )),

            Node::Unary(op, r) => match op.string.as_str() {
                "@" => {
                    return Ok(ArgumentDefinition::list(
                        r.compile_argument(env)?.unnamed_value()?,
                    ));
                }
                "@@" => {
                    return Ok(ArgumentDefinition::dict(
                        r.compile_argument(env)?.unnamed_value()?,
                    ));
                }
                _ => return error("Unknown operator"),
            },
            Node::Identifier(l) => ValueDefinition::Identifier(l.clone()),
            Node::Regex(l) => ValueDefinition::Value(
                Value::Regex(
                    l.string.clone(),
                    Regex::new(&l.string.clone())?, ),
                l.location,
            ),
            Node::String(t, true) => ValueDefinition::Value(Value::from(unescape(&t.string)?), t.location),
            Node::String(f, false) =>
                if is_command {
                    ValueDefinition::Identifier(f.clone())
                } else {
                    ValueDefinition::Value(Value::from(f), f.location)
                },
            Node::Integer(s) =>
                ValueDefinition::Value(
                    Value::Integer(s.string.replace("_", "").parse::<i128>()?),
                    s.location),
            Node::Float(s) =>
                ValueDefinition::Value(
                    Value::Float(s.string.replace("_", "").parse::<f64>()?),
                    s.location),
            Node::GetAttr(node, identifier) =>
                ValueDefinition::GetAttr(Box::new(node.compile(env, is_command)?.unnamed_value()?), identifier.clone()),

            Node::Substitution(s) => ValueDefinition::JobDefinition(s.compile(env)?),
            Node::Closure(signature, jobs) => {
                let param = signature.as_ref().map(|v| {
                    v.iter()
                        .map(|p| p.generate(env))
                        .collect::<CrushResult<Vec<Parameter>>>()
                });
                let p = match param {
                    None => None,
                    Some(Ok(p)) => Some(p),
                    Some(Err(e)) => return Err(e),
                };
                ValueDefinition::ClosureDefinition(None, p, jobs.compile(env)?, jobs.location)
            }
            Node::Glob(g) => ValueDefinition::Value(Value::Glob(Glob::new(&g.string)), g.location),
            Node::File(s, quoted) => ValueDefinition::Value(
                Value::from(
                    if *quoted { PathBuf::from(&unescape(&s.string)?) } else { PathBuf::from(&expand_user(&s.string)?) }
                ),
                s.location,
            ),
        }))
    }

    fn compile_standalone_assignment(
        target: &Box<Node>,
        op: &String,
        value: &Node,
        env: &Scope,
    ) -> CrushResult<Option<CommandInvocation>> {
        match op.deref() {
            "=" => match target.as_ref() {
                Node::Identifier(t) => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "var", "set"])?,
                    t.location,
                    vec![ArgumentDefinition::named(
                        t,
                        propose_name(&t, value.compile_argument(env)?.unnamed_value()?),
                    )],
                ),

                Node::GetItem(container, key) => container.method_invocation(
                    &TrackedString::new("__setitem__", key.location()),
                    vec![
                        ArgumentDefinition::unnamed(key.compile_argument(env)?.unnamed_value()?),
                        ArgumentDefinition::unnamed(value.compile_argument(env)?.unnamed_value()?),
                    ],
                    env,
                    true,
                ),

                Node::GetAttr(container, attr) => container.method_invocation(
                    &TrackedString::new("__setattr__", attr.location),
                    vec![
                        ArgumentDefinition::unnamed(ValueDefinition::Value(Value::from(attr),
                                                                           attr.location)),
                        ArgumentDefinition::unnamed(value.compile_argument(env)?.unnamed_value()?),
                    ],
                    env,
                    true,
                ),

                _ => error("Invalid left side in assignment"),
            },
            ":=" => match target.as_ref() {
                Node::Identifier(t) => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "var", "let"])?,
                    t.location,
                    vec![ArgumentDefinition::named(
                        t,
                        propose_name(&t, value.compile_argument(env)?.unnamed_value()?),
                    )],
                ),
                _ => error("Invalid left side in declaration"),
            },
            _ => error("Unknown assignment operator"),
        }
    }

    pub fn compile_as_special_command(&self, env: &Scope) -> CrushResult<Option<CommandInvocation>> {
        match self {
            Node::Assignment(target, _style, op, value) => {
                Node::compile_standalone_assignment(target, op, value, env)
            }

            Node::GetItem(val, key) => {
                val.method_invocation(&TrackedString::new("__getitem__", key.location()), vec![key.compile_argument(env)?], env, true)
            }

            Node::Unary(op, _) => match op.string.as_ref() {
                "@" | "@@" => Ok(None),
                _ => error("Unknown operator"),
            },

            Node::Glob(_)
            | Node::Identifier(_)
            | Node::Regex(_)
            | Node::String(_, _)
            | Node::Integer(_)
            | Node::Float(_)
            | Node::GetAttr(_, _)
            | Node::Substitution(_)
            | Node::Closure(_, _)
            | Node::File(_, _) => Ok(None),
        }
    }

    fn function_invocation(
        function: Command,
        location: Location,
        arguments: Vec<ArgumentDefinition>,
    ) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(CommandInvocation::new(
            ValueDefinition::Value(Value::from(function), location),
            arguments,
        )))
    }

    fn method_invocation(
        &self,
        name: &TrackedString,
        arguments: Vec<ArgumentDefinition>,
        env: &Scope,
        as_command: bool,
    ) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(CommandInvocation::new(
            ValueDefinition::GetAttr(
                Box::from(self.compile(env, as_command)?.unnamed_value()?),
                name.clone(),
            ),
            arguments,
        )))
    }

    pub fn identifier(is: impl Into<TrackedString>) -> Box<Node> {
        let s = is.into();
        if s.string.starts_with("$") {
            Box::from(Node::Identifier(s.slice_to_end(1)))
        } else {
            Box::from(Node::Identifier(s))
        }
    }

    pub fn file(is: impl Into<TrackedString>, quoted: bool) -> Box<Node> {
        Box::from(Node::File(is.into(), quoted))
    }

    pub fn quoted_string(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::String(is.into(), true))
    }

    pub fn return_expr(location: Location) -> Box<Node> {
        Self::control_expr("return", location)
    }

    pub fn break_expr(location: Location) -> Box<Node> {
        Self::control_expr("break", location)
    }

    pub fn continue_expr(location: Location) -> Box<Node> {
        Self::control_expr("continue", location)
    }

    pub fn if_expr(if_location: Location, condition: Box<Node>, true_body: JobListNode, false_body: Option<JobListNode>) -> Box<Node> {
        let location = if_location.union(true_body.location);
        let mut expressions = vec![
            Self::get_attr(&["global", "control", "if"], if_location),
            Node::Substitution(
                JobNode {
                    commands: vec![
                        CommandNode {
                            expressions: vec![*condition],
                            location,
                        }],
                    location,
                }
            ),
            Node::Closure(None, true_body),
        ];

        for x in false_body {
            expressions.push(Node::Closure(None, x));
        }

        Box::from(Node::Substitution(JobNode {
            commands: vec![CommandNode {
                expressions,
                location,
            }],
            location,
        }))
    }

    pub fn while_expr(while_location: Location, condition: Box<Node>, body: JobListNode) -> Box<Node> {
        let location = while_location.union(body.location);
        Box::from(Node::Substitution(JobNode {
            commands: vec![CommandNode {
                expressions: vec![
                    Self::get_attr(&["global", "control", "while"], while_location),
                    Node::Closure(
                        None,
                        JobListNode {
                            jobs: vec![
                                JobNode {
                                    commands: vec![
                                        CommandNode {
                                            expressions: vec![*condition],
                                            location,
                                        }],
                                    location,
                                }
                            ],
                            location,
                        },
                    ),
                    Node::Closure(None, body),
                ],
                location,
            }],
            location,
        }))
    }

    pub fn loop_expr(loop_location: Location, body: JobListNode) -> Box<Node> {
        let location = loop_location.union(body.location);
        Box::from(Node::Substitution(JobNode {
            commands: vec![CommandNode {
                expressions: vec![
                    Self::get_attr(&["global", "control", "loop"], loop_location),
                    Node::Closure(None, body),
                ],
                location,
            }],
            location,
        }))
    }

    pub fn for_expr(for_location: Location, id: TrackedString, iter: Box<Node>, body: JobListNode) -> Box<Node> {
        let location = for_location.union(body.location);
        Box::from(Node::Substitution(JobNode {
            commands: vec![CommandNode {
                expressions: vec![
                    Self::get_attr(&["global", "control", "for"], for_location),
                    Node::Assignment(Box::from(Node::Identifier(id)), SwitchStyle::None, "=".to_string(), iter),
                    Node::Closure(None, body),
                ],
                location,
            }],
            location,
        }))
    }

    fn get_attr(path: &[&str], location: Location) -> Node {
        if path.len() == 1 {
            Node::Identifier(TrackedString::from((path[0], location)))
        } else {
            Node::GetAttr(
                Box::from(Self::get_attr(&path[0..(path.len() - 1)], location)),
                TrackedString::from((path[path.len() - 1], location)))
        }
    }

    fn control_expr(keyword: &str, location: Location) -> Box<Node> {
        Box::from(Node::Substitution(JobNode {
            commands: vec![CommandNode {
                expressions: vec![
                    Self::get_attr(&["global", "control", keyword], location)
                ],
                location,
            }],
            location,
        }))
    }

    pub fn unquoted_string(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::String(is.into(), false))
    }

    pub fn glob(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Glob(is.into()))
    }

    pub fn integer(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Integer(is.into()))
    }

    pub fn float(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Float(is.into()))
    }

    pub fn regex(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Regex(is.into()))
    }
}
