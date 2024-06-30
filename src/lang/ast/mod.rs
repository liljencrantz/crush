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
use parameter_node::ParameterNode;
use tracked_string::TrackedString;
use crate::util::escape::unescape;
use crate::util::user_map::get_user;
use crate::util::user_map::get_current_username;

pub mod location;
pub mod tracked_string;
pub mod parameter_node;

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
    Symbol(TrackedString),
    String(TrackedString),
    File(TrackedString, bool),
    // true if filename is quoted
    Integer(TrackedString),
    Float(TrackedString),
    GetItem(Box<Node>, Box<Node>),
    GetAttr(Box<Node>, TrackedString),
    Substitution(JobNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode),
}

#[derive(Clone, Debug)]
pub struct JobListNode {
    pub jobs: Vec<JobNode>,
    pub location: Location,
}

impl JobListNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<Vec<Job>> {
        self.jobs.iter().map(|j| j.generate(env)).collect()
    }
}

#[derive(Clone, Debug)]
pub struct JobNode {
    pub commands: Vec<CommandNode>,
    pub location: Location,
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

fn operator_method(op: &str, op_location: Location, l: Box<Node>, r: Box<Node>) -> Box<Node> {
    let location = op_location.union(l.location()).union(r.location());
    let cmd = Node::GetAttr(l, TrackedString::from(op, op_location));
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

fn unary_operator_function(op: &[&str], op_location: Location, n: Box<Node>) -> Box<Node> {
    let location = op_location.union(n.location());
    let cmd = attr(op, op_location);
    Box::from(
        Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![
                        cmd, *n,
                    ],
                    location: location,
                }],
                location: location,
            }
        )
    )
}

fn unary_operator_method(op: &str, op_location: Location, n: Box<Node>) -> Box<Node> {
    let location = op_location.union(n.location());
    let cmd = Node::GetAttr(n, TrackedString::from(op, op_location));
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

pub fn operator(op: TrackedString, l: Box<Node>, r: Box<Node>) -> Box<Node> {
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

pub fn unary_operator(op: TrackedString, n: Box<Node>) -> Box<Node> {
    match op.string.as_str() {
        "typeof" => unary_operator_function(&["global", "types", "__typeof__"], op.location, n),
        "neg" => unary_operator_method("__neg__", op.location, n),
        "not" => unary_operator_function(&["global", "comp", "__not__"], op.location, n),

        _ => panic!("Unknown operator {}", &op.string),
    }
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

#[derive(Clone, Debug)]
pub struct CommandNode {
    pub expressions: Vec<Node>,
    pub location: Location,
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
            let cmd = self.expressions[0].generate_command(env)?;
            let arguments = self.expressions[1..]
                .iter()
                .map(|e| e.generate_argument(env))
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

impl Node {
    pub fn prefix(&self, pos: usize) -> CrushResult<Node> {
        match self {
            Node::Identifier(s) => Ok(Node::Identifier(s.prefix(pos))),
            _ => Ok(self.clone()),
        }
    }

    pub fn location(&self) -> Location {
        use Node::*;

        match self {
            Glob(s) | Identifier(s) | Symbol(s) |
            String(s) | Integer(s) | Float(s) |
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

    pub fn generate_command(&self, env: &Scope) -> CrushResult<ArgumentDefinition> {
        self.generate(env, true)
    }

    pub fn generate_argument(&self, env: &Scope) -> CrushResult<ArgumentDefinition> {
        self.generate(env, false)
    }

    pub fn type_name(&self) -> &str {
        match self {
            Node::Assignment(_, _, _, _) => "assignment",
            Node::Unary(_, _) => "unary operator",
            Node::Glob(_) => "glob",
            Node::Identifier(_) => "identifier",
            Node::Regex(_) => "regular expression literal",
            Node::Symbol(_) => "symbol",
            Node::String(_) => "quoted string literal",
            Node::File(_, _) => "file literal",
            Node::Integer(_) => "integer literal",
            Node::Float(_) => "floating point number literal",
            Node::GetItem(_, _) => "subscript",
            Node::GetAttr(_, _) => "member access",
            Node::Substitution(_) => "command substitution",
            Node::Closure(_, _) => "closure",
        }
    }

    pub fn generate(&self, env: &Scope, is_command: bool) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(match self {
            Node::Assignment(target, style, op, value) => match op.deref() {
                "=" => {
                    return match target.as_ref() {
                        Node::Symbol(t) => Ok(ArgumentDefinition::named_with_style(
                            t,
                            *style,
                            propose_name(&t, value.generate_argument(env)?.unnamed_value()?),
                        )),
                        _ => error(format!("Invalid left side in named argument. Expected a symbol, got a {}", target.type_name())),
                    };
                }
                _ => return error("Invalid assignment operator"),
            },

            Node::GetItem(a, o) => ValueDefinition::JobDefinition(
                Job::new(vec![self
                    .generate_standalone(env)?
                    .unwrap()],
                         a.location().union(o.location()),
                )),

            Node::Unary(op, r) => match op.string.as_str() {
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
            Node::Identifier(l) => ValueDefinition::Identifier(l.clone()),
            Node::Regex(l) => ValueDefinition::Value(
                Value::Regex(
                    l.string.clone(),
                    to_crush_error(Regex::new(&l.string.clone()))?, ),
                l.location,
            ),
            Node::String(t) => ValueDefinition::Value(Value::from(unescape(&t.string)?), t.location),
            Node::Integer(s) =>
                ValueDefinition::Value(
                    Value::Integer(to_crush_error(
                        s.string.replace("_", "").parse::<i128>()
                    )?),
                    s.location),
            Node::Float(s) =>
                ValueDefinition::Value(
                    Value::Float(to_crush_error(
                        s.string.replace("_", "").parse::<f64>()
                    )?),
                    s.location),
            Node::GetAttr(node, identifier) =>
                ValueDefinition::GetAttr(Box::new(node.generate(env, is_command)?.unnamed_value()?), identifier.clone()),

            Node::Symbol(f) =>
                if is_command {
                    ValueDefinition::Identifier(f.clone())
                } else {
                    ValueDefinition::Value(Value::from(f), f.location)
                },
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
            Node::File(s, quoted) => ValueDefinition::Value(
                Value::from(
                    if *quoted { PathBuf::from(&unescape(&s.string)?) } else { PathBuf::from(&s.string) }
                ),
                s.location,
            ),
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
                Node::Identifier(t) => Node::function_invocation(
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
                    true,
                ),

                Node::GetAttr(container, attr) => container.method_invocation(
                    &TrackedString::from("__setattr__", attr.location),
                    vec![
                        ArgumentDefinition::unnamed(ValueDefinition::Value(Value::from(attr),
                                                                           attr.location)),
                        ArgumentDefinition::unnamed(value.generate_argument(env)?.unnamed_value()?),
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
            Node::Assignment(target, _style, op, value) => {
                Node::generate_standalone_assignment(target, op, value, env)
            }

            Node::GetItem(val, key) => {
                val.method_invocation(&TrackedString::from("__getitem__", key.location()), vec![key.generate_argument(env)?], env, true)
            }

            Node::Unary(op, _) => match op.string.as_ref() {
                "@" | "@@" => Ok(None),
                _ => error("Unknown operator"),
            },

            Node::Glob(_)
            | Node::Identifier(_)
            | Node::Regex(_)
            | Node::Symbol(_)
            | Node::String(_)
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
                Box::from(self.generate(env, as_command)?.unnamed_value()?),
                name.clone(),
            ),
            arguments,
        )))
    }

    pub fn parse_symbol_or_glob(s: &TrackedString) -> Box<Node> {
        let path = expand_user(s.string.clone()).unwrap_or_else(|_| { s.string.clone() });
        let ts = TrackedString::from(&path, s.location);
        if path.contains('%') || path.contains('?') {
            Box::from(Node::Glob(ts))
        } else if s.string.contains('/') || s.string.contains('.') {
            Box::from(Node::File(ts, false))
        } else {
            Box::from(Node::Symbol(ts))
        }
    }

    pub fn parse_identifier(s: &TrackedString) -> Box<Node> {
        Box::from(Node::Identifier(TrackedString::from(&s.string[1..], s.location)))
    }

    pub fn parse_file_or_glob(s: &TrackedString) -> Box<Node> {
        let path = expand_user(s.string.clone()).unwrap_or_else(|_| { s.string.clone() });
        let ts = TrackedString::from(&path, s.location);
        if ts.string.contains('%') || ts.string.contains('?') {
            Box::from(Node::Glob(ts.clone()))
        } else {
            Box::from(Node::File(ts.clone(), false))
        }
    }
}

fn attr(parts: &[&str], location: Location) -> Node {
    let mut res = Node::Identifier(TrackedString::from(parts[0], location));
    for part in &parts[1..] {
        res = Node::GetAttr(Box::from(res), TrackedString::from(part, location));
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
        let home = if parts[0].len() > 0 {home_as_string(parts[0])} else {home_as_string(&get_current_username()?)};
        if parts.len() == 1 {
            home
        } else {
            home.map(|home| { format!("{}/{}", home, parts[1]) })
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
    StringOrGlob,
    Identifier,
    Flag,
    QuotedFile,
    FileOrGlob,
    Regex,
    Separator,
    Integer,
    Float,
    SubStart,
    SubEnd,
    JobStart,
    JobEnd,
    GetItemStart,
    GetItemEnd,
    Pipe,
    Unnamed,
    Named,
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
