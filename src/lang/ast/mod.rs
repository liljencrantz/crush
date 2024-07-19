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
use parameter_node::ParameterNode;
use tracked_string::TrackedString;
use crate::util::escape::unescape;
use crate::util::user_map::get_user;
use crate::util::user_map::get_current_username;

pub mod location;
pub mod tracked_string;
pub mod parameter_node;
pub mod lexer;

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
    pub fn to_command(self) -> CommandNode {
        let l = self.location();
        match self {
            Node::Substitution(n) if n.commands.len() == 1 => {
                n.commands[0].clone()
            }
            _ => CommandNode {
                expressions: vec![self],
                location: l,
            }
        }
    }
}

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
        if let Some(c) = self.expressions[0].compile_as_command(env)? {
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

    pub fn compile(&self, env: &Scope, is_command: bool) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(match self {
            Node::Assignment(target, style, op, value) => match op.deref() {
                "=" => {
                    return match target.as_ref() {
                        Node::Symbol(t) | Node::Identifier(t) => Ok(ArgumentDefinition::named_with_style(
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
                    .compile_as_command(env)?
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
                ValueDefinition::GetAttr(Box::new(node.compile(env, is_command)?.unnamed_value()?), identifier.clone()),

            Node::Symbol(f) =>
                if is_command {
                    ValueDefinition::Identifier(f.clone())
                } else {
                    ValueDefinition::Value(Value::from(f), f.location)
                },
            Node::Substitution(s) => ValueDefinition::JobDefinition(s.compile(env)?),
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
                ValueDefinition::ClosureDefinition(None, p, c.compile(env)?, c.location)
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

    pub fn compile_as_command(&self, env: &Scope) -> CrushResult<Option<CommandInvocation>> {
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
                Box::from(self.compile(env, as_command)?.unnamed_value()?),
                name.clone(),
            ),
            arguments,
        )))
    }

    pub fn parse_symbol_or_glob(is: impl Into<TrackedString>) -> Box<Node> {
        let s = is.into();
        let path = expand_user(s.string.clone()).unwrap_or_else(|_| { s.string.clone() });
        let ts = TrackedString::new(&path, s.location);
        if path.contains('*') || path.contains('?') {
            Box::from(Node::Glob(ts))
        } else if s.string.contains('/') || s.string.contains('.') {
            Box::from(Node::File(ts, false))
        } else {
            Box::from(Node::Symbol(ts))
        }
    }

    pub fn identifier(is: impl Into<TrackedString>) -> Box<Node> {
        let s = is.into();
        if s.string.starts_with("$") {
            Box::from(Node::Identifier(s.slice_to_end(1)))
        } else {
            Box::from(Node::Identifier(s))
        }
    }

    pub fn parse_file_or_glob(is: impl Into<TrackedString>) -> Box<Node> {
        let s = is.into();
        let path = expand_user(s.string.clone()).unwrap_or_else(|_| { s.string.clone() });
        let ts = TrackedString::new(&path, s.location);
        if ts.string.contains('*') || ts.string.contains('?') {
            Box::from(Node::Glob(ts.clone()))
        } else {
            Box::from(Node::File(ts.clone(), false))
        }
    }

    pub fn file(is: impl Into<TrackedString>, quoted: bool) -> Box<Node> {
        Box::from(Node::File(is.into(), quoted))
    }

    pub fn string(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::String(is.into()))
    }

    pub fn integer(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Integer(is.into()))
    }

    pub fn float(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Float(is.into()))
    }

    pub fn regex(is: impl Into<TrackedString>) -> Box<Node> {
        let ts = is.into();
        let s = ts.string;
        Box::from(Node::Regex(TrackedString::new(&s[3..s.len() - 1], ts.location)))
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Token<'input> {
    LogicalOperator(&'input str, Location),
    UnaryOperator(&'input str, Location),
    ComparisonOperator(&'input str, Location),
    Bang(Location),
    Plus(Location),
    Minus(Location),
    Star(Location),
    Slash(Location),
    QuotedString(&'input str, Location),
    StringOrGlob(&'input str, Location),
    Identifier(&'input str, Location),
    Flag(&'input str, Location),
    QuotedFile(&'input str, Location),
    FileOrGlob(&'input str, Location),
    Regex(&'input str, Location),
    Integer(&'input str, Location),
    Float(&'input str, Location),
    MemberOperator(Location),
    Equals(Location),
    Declare(Location),
    Separator(&'input str, Location),
    SubStart(Location),
    SubEnd(Location),
    JobStart(Location),
    JobEnd(Location),
    GetItemStart(Location),
    GetItemEnd(Location),
    Pipe(Location),
    Unnamed(Location),
    Named(Location),
    ExprModeStart(Location),
}

impl Token<'_> {
    pub fn location(&self) -> Location {
        match self {
            Token::LogicalOperator(_, l) |
            Token::UnaryOperator(_, l) |
            Token::ComparisonOperator(_, l) |
            Token::QuotedString(_, l) |
            Token::StringOrGlob(_, l) |
            Token::Identifier(_, l) |
            Token::Flag(_, l) |
            Token::QuotedFile(_, l) |
            Token::FileOrGlob(_, l) |
            Token::Regex(_, l) |
            Token::Integer(_, l) |
            Token::Float(_, l) |
            Token::MemberOperator(l) |
            Token::Equals(l) |
            Token::Declare(l) |
            Token::Separator(_, l) |
            Token::SubStart(l) |
            Token::SubEnd(l) |
            Token::JobStart(l) |
            Token::JobEnd(l) |
            Token::GetItemStart(l) |
            Token::GetItemEnd(l) |
            Token::Pipe(l) |
            Token::Unnamed(l) |
            Token::Named(l) |
            Token::Bang(l) |
            Token::Plus(l) |
            Token::Minus(l) |
            Token::Star(l) |
            Token::Slash(l) |
            Token::ExprModeStart(l) => *l,
        }
    }

    pub fn as_string(&self) -> &str {
        match self {
            Token::LogicalOperator(s, _) |
            Token::UnaryOperator(s, _) |
            Token::ComparisonOperator(s, _) |
            Token::QuotedString(s, _) |
            Token::StringOrGlob(s, _) |
            Token::Identifier(s, _) |
            Token::Flag(s, _) |
            Token::QuotedFile(s, _) |
            Token::FileOrGlob(s, _) |
            Token::Regex(s, _) |
            Token::Integer(s, _) |
            Token::Separator(s, _) |
            Token::Float(s, _) => s,
            Token::MemberOperator(_) => ":",
            Token::Equals(_) => "=",
            Token::Declare(_) => ":=",
            Token::SubStart(_) => "(",
            Token::SubEnd(_) => "_",
            Token::JobStart(_) => "{",
            Token::JobEnd(_) => "}",
            Token::GetItemStart(_) => "[",
            Token::GetItemEnd(_) => "]",
            Token::Pipe(_) => "|",
            Token::Unnamed(_) => "@",
            Token::Named(_) => "@@",
            Token::ExprModeStart(_) => "m(",
            Token::Bang(_) => "!",
            Token::Plus(_) => "+",
            Token::Minus(_) => "-",
            Token::Star(_) => "*",
            Token::Slash(_) => "/",
        }
    }
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&TrackedString::from(self.clone()).string)
    }
}
impl From<Token<'_>> for String {
    fn from(token: Token) -> String {
        TrackedString::from(token).string
    }
}
impl<'a> Into<Spanned<'a>> for Token<'a> {
    fn into(self) -> Spanned<'a> {
        let loc = match &self {
            Token::LogicalOperator(_, l) |
            Token::UnaryOperator(_, l) |
            Token::QuotedString(_, l) |
            Token::StringOrGlob(_, l) |
            Token::Identifier(_, l) |
            Token::Flag(_, l) |
            Token::QuotedFile(_, l) |
            Token::FileOrGlob(_, l) |
            Token::Regex(_, l) |
            Token::Integer(_, l) |
            Token::ComparisonOperator(_, l) |
            Token::Float(_, l) |
            Token::MemberOperator(l) |
            Token::Equals(l) |
            Token::Declare(l) |
            Token::Separator(_, l) |
            Token::SubStart(l) |
            Token::SubEnd(l) |
            Token::JobStart(l) |
            Token::JobEnd(l) |
            Token::GetItemStart(l) |
            Token::GetItemEnd(l) |
            Token::Pipe(l) |
            Token::Unnamed(l) |
            Token::Named(l) |
            Token::Bang(l) |
            Token::Plus(l) |
            Token::Minus(l) |
            Token::Star(l) |
            Token::Slash(l) |
            Token::ExprModeStart(l) => { l }
        };
        Ok((loc.start, self, loc.end))
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum LexicalError {
    #[default]
    MismatchedSubEnd,
    MismatchedDoubleQuote,
    MismatchedSingleQuote,
    UnexpectedCharacter(char),
    UnexpectedCharacterWithSuggestion(char, char),
    UnexpectedEOF,
    UnexpectedEOFWithSuggestion(char),
}

impl Display for LexicalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LexicalError::MismatchedSubEnd => f.write_str("Mismatched ) (ending parenthesis)"),
            LexicalError::MismatchedDoubleQuote => f.write_str("Mismatched \" (double quote)"),
            LexicalError::MismatchedSingleQuote => f.write_str("Mismatched ' (single quote)"),
            LexicalError::UnexpectedCharacter(c) => {
                f.write_str("Unexpected character ")?;
                f.write_char(*c)
            }
            LexicalError::UnexpectedCharacterWithSuggestion(actual, expected) => {
                f.write_str("Unexpected character ")?;
                f.write_char(*actual)?;
                f.write_str(", expected ")?;
                f.write_char(*expected)
            }
            LexicalError::UnexpectedEOF => f.write_str("Unexpected end of input"),
            LexicalError::UnexpectedEOFWithSuggestion(expected) => {
                f.write_str("Unexpected end of input, expected ")?;
                f.write_char(*expected)
            }
        }
    }
}

pub type Spanned<'input> = Result<(usize, Token<'input>, usize), LexicalError>;

