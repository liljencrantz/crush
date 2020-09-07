use crate::lang::argument::ArgumentDefinition;
use crate::lang::command::{Command, Parameter};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::{error, to_crush_error, CrushResult};
use crate::lang::job::Job;
use crate::lang::data::scope::Scope;
use crate::lang::value::{Value, ValueDefinition, ValueType};
use crate::util::glob::Glob;
use regex::Regex;
use std::ops::Deref;
use std::path::PathBuf;
use std::fmt::{Display, Formatter};
use std::cmp::{min, max};

pub struct JobListNode {
    pub jobs: Vec<JobNode>,
    pub location: Location,
}

impl JobListNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<Vec<Job>> {
        self.jobs.iter().map(|j| j.generate(env)).collect()
    }
}

pub struct JobNode {
    pub commands: Vec<CommandNode>,
    pub location: Location,
}

impl JobNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<Job> {
        Ok(Job::new(
            self.commands
                .iter()
                .map(|c| c.generate(env))
                .collect::<CrushResult<Vec<CommandInvocation>>>()?,
            self.location,
        ))
    }
}

pub struct CommandNode {
    pub expressions: Vec<Node>,
}

impl CommandNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<CommandInvocation> {
        if let Some(c) = self.expressions[0].generate_standalone(env)? {
            if self.expressions.len() == 1 {
                Ok(c)
            } else {
                error("Stray arguments")
            }
        } else {
            let cmd = self.expressions[0].generate_argument(env)?;
            let arguments = self.expressions[1..]
                .iter()
                .map(|e| e.generate_argument(env))
                .collect::<CrushResult<Vec<ArgumentDefinition>>>()?;
            Ok(CommandInvocation::new(cmd.unnamed_value()?, arguments))
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrackedString {
    pub string: String,
    pub location: Location,
}

impl TrackedString {
    pub fn from(string: &str, location: Location) -> TrackedString {
        TrackedString {
            string: string.to_string(),
            location,
        }
    }

    pub fn literal(start: usize, string: &str, end: usize) -> TrackedString {
        TrackedString {
            string: string.to_string(),
            location: Location::new(start, end),
        }
    }
}

impl Display for TrackedString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.string)
    }
}

#[derive(Clone, Debug, Copy)]
pub struct Location {
    pub start: usize,
    pub end: usize,
}

impl Location {
    pub fn empty() -> Location {
        Location { start: 0, end: 0 }
    }

    pub fn new(start: usize, end: usize) -> Location {
        Location { start, end }
    }

    pub fn union(&self, other: &Location) -> Location {
        Location {
            start: min(self.start, other.start),
            end: max(self.end, other.end),
        }
    }
}

pub enum Node {
    Assignment(Box<Node>, String, Box<Node>),
    LogicalOperation(Box<Node>, TrackedString, Box<Node>),
    Comparison(Box<Node>, TrackedString, Box<Node>),
    Term(Box<Node>, TrackedString, Box<Node>),
    Factor(Box<Node>, TrackedString, Box<Node>),
    Unary(TrackedString, Box<Node>),
    Glob(TrackedString),
    Label(TrackedString),
    Regex(TrackedString),
    Field(TrackedString),
    String(TrackedString),
    File(PathBuf, Location),
    Integer(i128, Location),
    Float(f64, Location),
    GetItem(Box<Node>, Box<Node>),
    GetAttr(Box<Node>, TrackedString),
    Path(Box<Node>, TrackedString),
    Substitution(JobNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode),
}

fn propose_name(name: &TrackedString, v: ValueDefinition) -> ValueDefinition {
    match v {
        ValueDefinition::ClosureDefinition(_, p, j, l) =>
            ValueDefinition::ClosureDefinition(Some(name.clone()), p, j, l),
        _ => v,
    }
}

impl Node {
    pub fn location(&self) -> Location {
        use Node::*;

        match self {
            Glob(s) |
            Label(s) |
            Field(s) |
            String(s) |
            Regex(s) =>
                s.location,

            Assignment(a, _, b) |
            LogicalOperation(a, _, b) |
            Comparison(a, _, b) |
            Term(a, _, b) |
            Factor(a, _, b) =>
                a.location().union(&b.location()),

            Unary(s, a) =>
                s.location.union(&a.location()),

            File(_, l) |
            Integer(_, l) |
            Float(_, l) => *l,

            GetItem(a, b) => a.location().union(&b.location()),
            GetAttr(p, n) |
            Path(p, n) => p.location().union(&n.location),
            Substitution(j) => j.location,
            Closure(p, j) => {
                // Fixme: Can't tab complete or error report on parameters because they're not currently tracked
                j.location
            }
        }
    }

    pub fn generate_argument(&self, env: &Scope) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(match self {
            Node::Assignment(target, op, value) => match op.deref() {
                "=" => {
                    return match target.as_ref() {
                        Node::Label(t) => Ok(ArgumentDefinition::named(
                            t.deref(),
                            propose_name(&t, value.generate_argument(env)?.unnamed_value()?),
                        )),
                        _ => error("Invalid left side in named argument"),
                    };
                }
                _ => return error("Invalid assignment operator"),
            },

            Node::GetItem(a, o) => ValueDefinition::JobDefinition(
                Job::new(vec![self
                    .generate_standalone(env)?
                    .unwrap()],
                         a.location().union(&o.location()),
                )),

            Node::LogicalOperation(a, o, b)
            | Node::Comparison(a, o, b)
            | Node::Term(a, o, b)
            | Node::Factor(a, o, b) => ValueDefinition::JobDefinition(
                Job::new(vec![self
                    .generate_standalone(env)?
                    .unwrap()],
                         a.location().union(&b.location()).union(&o.location),
                )),
            Node::Unary(op, r) => match op.string.as_str() {
                "neg" | "not" | "typeof" => ValueDefinition::JobDefinition(Job::new(vec![self
                    .generate_standalone(env)?
                    .unwrap()],
                op.location.union(&r.location()))),
                "@" => {
                    return Ok(ArgumentDefinition::list(
                        r.generate_argument(env)?.unnamed_value()?,
                    ));
                }
                "@@" => {
                    return Ok(ArgumentDefinition::dict(
                        r.generate_argument(env)?.unnamed_value()?,
                    ));
                }
                _ => return error("Unknown operator"),
            },
            Node::Label(l) => ValueDefinition::Label(l.clone()),
            Node::Regex(l) => ValueDefinition::Value(
                Value::Regex(
                    l.string.clone(),
                    to_crush_error(Regex::new(&l.string.clone()))?, ),
                l.location,
            ),
            Node::String(t) => ValueDefinition::Value(Value::String(unescape(&t.string)), t.location),
            Node::Integer(i, location) => ValueDefinition::Value(Value::Integer(*i), *location),
            Node::Float(f, location) => ValueDefinition::Value(Value::Float(*f), *location),
            Node::GetAttr(node, label) => {
                let parent = node.generate_argument(env)?;
                match parent.unnamed_value()? {
                    ValueDefinition::Value(Value::Field(mut f), location) => {
                        f.push(label.string.clone());
                        ValueDefinition::Value(Value::Field(f), location)
                    }
                    value => ValueDefinition::GetAttr(Box::new(value), label.clone()),
                }
            }
            Node::Path(node, label) => ValueDefinition::Path(
                Box::new(node.generate_argument(env)?.unnamed_value()?),
                label.clone(),
            ),
            Node::Field(f) => ValueDefinition::Value(Value::Field(vec![f.string[1..].to_string()]), f.location),
            Node::Substitution(s) => ValueDefinition::JobDefinition(s.generate(env)?),
            Node::Closure(s, c) => {
                let param = s.as_ref().map(|v| {
                    v.iter()
                        .map(|p| p.generate(env))
                        .collect::<CrushResult<Vec<Parameter>>>()
                });
                let p = match param {
                    None => None,
                    Some(Ok(p)) => Some(p),
                    Some(Err(e)) => return Err(e),
                };
                ValueDefinition::ClosureDefinition(None, p, c.generate(env)?, c.location)
            }
            Node::Glob(g) => ValueDefinition::Value(Value::Glob(Glob::new(&g.string)), g.location),
            Node::File(f, location) => ValueDefinition::Value(Value::File(f.clone()), *location),
        }))
    }

    fn generate_standalone_assignment(
        target: &Box<Node>,
        op: &String,
        value: &Node,
        env: &Scope,
    ) -> CrushResult<Option<CommandInvocation>> {
        match op.deref() {
            "=" => match target.as_ref() {
                Node::Label(t) => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "var", "set"])?,
                    t.location,
                    vec![ArgumentDefinition::named(
                        t,
                        propose_name(&t, value.generate_argument(env)?.unnamed_value()?),
                    )],
                ),

                Node::GetItem(container, key) => container.method_invocation(
                    &TrackedString::from("__setitem__", key.location()),
                    vec![
                        ArgumentDefinition::unnamed(key.generate_argument(env)?.unnamed_value()?),
                        ArgumentDefinition::unnamed(value.generate_argument(env)?.unnamed_value()?),
                    ],
                    env,
                ),

                Node::GetAttr(container, attr) => container.method_invocation(
                    &TrackedString::from("__setattr__", attr.location),
                    vec![
                        ArgumentDefinition::unnamed(ValueDefinition::Value(Value::String(
                            attr.string.to_string(),
                        ),
                                                                           attr.location)),
                        ArgumentDefinition::unnamed(value.generate_argument(env)?.unnamed_value()?),
                    ],
                    env,
                ),

                _ => error("Invalid left side in assignment"),
            },
            ":=" => match target.as_ref() {
                Node::Label(t) => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "var", "let"])?,
                    t.location,
                    vec![ArgumentDefinition::named(
                        t,
                        propose_name(&t, value.generate_argument(env)?.unnamed_value()?),
                    )],
                ),
                _ => error("Invalid left side in declaration"),
            },
            _ => error("Unknown assignment operator"),
        }
    }

    pub fn generate_standalone(&self, env: &Scope) -> CrushResult<Option<CommandInvocation>> {
        match self {
            Node::Assignment(target, op, value) => {
                Node::generate_standalone_assignment(target, op, value, env)
            }

            Node::LogicalOperation(l, op, r) => {
                let function = env.global_static_cmd(match op.string.as_ref() {
                    "and" => vec!["global", "cond", "and"],
                    "or" => vec!["global", "cond", "or"],
                    _ => return error("Unknown operator"),
                })?;
                Node::function_invocation(
                    function,
                    op.location,
                    vec![l.generate_argument(env)?, r.generate_argument(env)?],
                )
            }

            Node::Comparison(l, op, r) => {
                let function = env.global_static_cmd(match op.string.as_ref() {
                    "<" => vec!["global", "comp", "lt"],
                    "<=" => vec!["global", "comp", "lte"],
                    ">" => vec!["global", "comp", "gt"],
                    ">=" => vec!["global", "comp", "gte"],
                    "==" => vec!["global", "comp", "eq"],
                    "!=" => vec!["global", "comp", "neq"],
                    "=~" => {
                        return r.method_invocation(&TrackedString::from("match", op.location), vec![l.generate_argument(env)?], env);
                    }
                    "!~" => {
                        return r.method_invocation(
                            &TrackedString::from("not_match", op.location),
                            vec![l.generate_argument(env)?],
                            env,
                        );
                    }
                    _ => return error("Unknown operator"),
                })?;
                Node::function_invocation(
                    function,
                    op.location,
                    vec![l.generate_argument(env)?, r.generate_argument(env)?],
                )
            }

            Node::Term(l, op, r) => {
                let method = match op.string.as_ref() {
                    "+" => "__add__",
                    "-" => "__sub__",
                    _ => return error("Unknown operator"),
                };
                l.method_invocation(&TrackedString::from(method, op.location), vec![r.generate_argument(env)?], env)
            }

            Node::Factor(l, op, r) => {
                let method = match op.string.as_ref() {
                    "*" => "__mul__",
                    "//" => "__div__",
                    _ => return error("Unknown operator"),
                };
                l.method_invocation(&TrackedString::from(method, op.location), vec![r.generate_argument(env)?], env)
            }

            Node::GetItem(val, key) => {
                val.method_invocation(&TrackedString::from("__getitem__", key.location()), vec![key.generate_argument(env)?], env)
            }

            Node::Unary(op, r) => match op.string.as_ref() {
                "neg" => r.method_invocation(&TrackedString::from("__neg__", op.location), vec![], env),
                "not" => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "comp", "not"])?,
                    op.location,
                    vec![r.generate_argument(env)?],
                ),
                "typeof" => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "types", "typeof"])?,
                    op.location,
                    vec![r.generate_argument(env)?],
                ),
                "@" | "@@" => Ok(None),
                _ => error("Unknown operator"),
            },

            Node::Glob(_)
            | Node::Label(_)
            | Node::Regex(_)
            | Node::Field(_)
            | Node::String(_)
            | Node::Integer(_, _)
            | Node::Float(_, _)
            | Node::GetAttr(_, _)
            | Node::Path(_, _)
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
            ValueDefinition::Value(Value::Command(function), location),
            arguments,
        )))
    }

    fn method_invocation(
        &self,
        name: &TrackedString,
        arguments: Vec<ArgumentDefinition>,
        env: &Scope,
    ) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(CommandInvocation::new(
            ValueDefinition::GetAttr(
                Box::from(self.generate_argument(env)?.unnamed_value()?),
                name.clone(),
            ),
            arguments,
        )))
    }

    pub fn parse_label(s: &TrackedString) -> Box<Node> {
        if s.string.contains('%') || s.string.contains('?') {
            Box::from(Node::Glob(s.clone()))
        } else if s.string.starts_with('~') {
            expand_user_path(s)
        } else if s.string.contains('/') {
            if s.string.starts_with('/') {
                Box::from(Node::File(PathBuf::from(&s.string), s.location))
            } else {
                let parts = s.string.split('/').collect::<Vec<&str>>();
                Box::from(path(&parts, s.location))
            }
        } else {
            Box::from(Node::Label(s.clone()))
        }
    }
}

fn path(parts: &[&str], location: Location) -> Node {
    let mut res = Node::Label(TrackedString::from(parts[0], location));
    for part in &parts[1..] {
        res = Node::Path(Box::from(res), TrackedString::from(part, location));
    }
    res
}

fn attr(parts: &[&str], location: Location) -> Node {
    let mut res = Node::Label(TrackedString::from(parts[0], location));
    for part in &parts[1..] {
        res = Node::GetAttr(Box::from(res), TrackedString::from(part, location));
    }
    res
}

fn simple_substitution(cmd: Vec<Node>, location: Location) -> Box<Node> {
    Box::from(
        Node::Substitution(
            JobNode {
                commands: vec![
                    CommandNode {
                        expressions: cmd
                    }
                ],
                location,
            }
        )
    )
}

fn expand_user(s: &str, location: Location) -> Box<Node> {
    if s.len() == 1 {
        Box::from(
            Node::GetAttr(
                simple_substitution(
                    vec![
                        attr(&vec!["global", "user", "me"], location)
                    ],
                    location,
                ),
                TrackedString::from("home", location),
            )
        )
    } else {
        Box::from(
            Node::GetAttr(
                simple_substitution(
                    vec![
                        attr(&vec!["global", "user", "find"], location),
                        Node::String(TrackedString::from(&format!("\"{}\"", &s[1..]), location))
                    ],
                    location,
                ),
                TrackedString::from("home", location),
            )
        )
    }
}

fn expand_user_path(s: &TrackedString) -> Box<Node> {
    if s.string.contains('/') {
        let (user, path) = s.string.split_at(s.string.find('/').unwrap());
        Box::from(
            Node::Path(
                expand_user(user, s.location),
                TrackedString::from(&path[1..], s.location),
            )
        )
    } else {
        expand_user(&s.string, s.location)
    }
}

pub fn unescape(s: &str) -> String {
    let mut res = "".to_string();
    let mut was_backslash = false;
    for c in s[1..s.len() - 1].chars() {
        if was_backslash {
            match c {
                'n' => res += "\n",
                'r' => res += "\r",
                't' => res += "\t",
                _ => res += &c.to_string(),
            }
            was_backslash = false;
        } else {
            if c == '\\' {
                was_backslash = true;
            } else {
                res += &c.to_string();
            }
        }
    }
    res
}

pub enum ParameterNode {
    Parameter(TrackedString, Option<Box<Node>>, Option<Node>),
    Named(TrackedString),
    Unnamed(TrackedString),
}

impl ParameterNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<Parameter> {
        match self {
            ParameterNode::Parameter(name, value_type, default) => Ok(Parameter::Parameter(
                name.clone(),
                value_type
                    .as_ref()
                    .map(|t| t.generate_argument(env)?.unnamed_value())
                    .unwrap_or(Ok(ValueDefinition::Value(Value::Type(ValueType::Any), name.location)))?,
                default
                    .as_ref()
                    .map(|d| d.generate_argument(env))
                    .transpose()?
                    .map(|a| a.unnamed_value())
                    .transpose()?,
            )),
            ParameterNode::Named(s) => Ok(Parameter::Named(s.clone())),
            ParameterNode::Unnamed(s) => Ok(Parameter::Unnamed(s.clone())),
        }
    }
}

pub struct TokenListNode {
    pub tokens: Vec<TokenNode>,
}

impl TokenListNode {
    pub fn new() -> TokenListNode {
        TokenListNode {
            tokens: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub enum TokenType {
    LogicalOperator,
    UnaryOperator,
    Colon,
    AssignmentOperator,
    ComparisonOperator,
    FactorOperator,
    TermOperator,
    QuotedString,
    Label,
    Flag,
    Field,
    QuotedLabel,
    Regex,
    Separator,
    Integer,
    Float,
    /*
    Missing:
    |, @, @@ [] () {}
     */
}

#[derive(Clone)]
pub struct TokenNode {
    pub token_type: TokenType,
    pub start: usize,
    pub end: usize,
    pub data: String,
}

impl TokenNode {
    pub fn new(token_type: TokenType, start: usize, data: &str, end: usize) -> TokenNode {
        TokenNode {
            token_type,
            start,
            end,
            data: data.to_string(),
        }
    }

    pub fn location(&self) -> (usize, usize) {
        (self.start, self.end)
    }
}