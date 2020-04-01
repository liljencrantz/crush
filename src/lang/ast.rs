use crate::lang::job::Job;
use crate::lang::errors::{CrushResult, error, to_crush_error};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::argument::ArgumentDefinition;
use crate::lang::value::{ValueDefinition, Value, ValueType};
use std::ops::Deref;
use crate::lang::command::{CrushCommand, Parameter};
use crate::util::glob::Glob;
use lazy_static::lazy_static;
use crate::lib::comp;
use crate::lib::cond;
use crate::lib::var;
use crate::lib::types;
use regex::Regex;
use std::path::Path;

lazy_static! {
    pub static ref LT: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(comp::lt, false)};
    pub static ref LTE: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(comp::lte, false)};
    pub static ref GT: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(comp::gt, false)};
    pub static ref GTE: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(comp::gte, false)};
    pub static ref EQ: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(comp::eq, false)};
    pub static ref NEQ: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(comp::neq, false)};
    pub static ref NOT: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(comp::not, false)};

    pub static ref AND: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(cond::and, false)};
    pub static ref OR: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(cond::or, false)};

    pub static ref LET: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(var::r#let, false)};
    pub static ref SET: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(var::set, false)};

    pub static ref AS: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(types::r#as, false)};
    pub static ref TYPEOF: Box<dyn CrushCommand +  Send + Sync> = {CrushCommand::command_undocumented(types::r#typeof, false)};
}

pub struct JobListNode {
    pub jobs: Vec<JobNode>,
}

impl JobListNode {
    pub fn generate(&self) -> CrushResult<Vec<Job>> {
        self.jobs.iter().map(|j| j.generate()).collect()
    }
}

pub struct JobNode {
    pub commands: Vec<CommandNode>,
}

impl JobNode {
    pub fn generate(&self) -> CrushResult<Job> {
        Ok(Job::new(self.commands.iter().map(|c| c.generate()).collect::<CrushResult<Vec<CommandInvocation>>>()?))
    }
}

pub struct CommandNode {
    pub expressions: Vec<Node>,
}

impl CommandNode {
    pub fn generate(&self) -> CrushResult<CommandInvocation> {
        if let Some(c) = self.expressions[0].generate_standalone()? {
            if self.expressions.len() == 1 {
                Ok(c)
            } else {
                error("Stray arguments")
            }
        } else {
            let cmd = self.expressions[0].generate_argument()?;
            let arguments = self.expressions[1..].iter()
                .map(|e| e.generate_argument())
                .collect::<CrushResult<Vec<ArgumentDefinition>>>()?;
            Ok(CommandInvocation::new(cmd.unnamed_value()?, arguments))
        }
    }
}


pub enum Node {
    Assignment(Box<Node>, Box<str>, Box<Node>),
    LogicalOperation(Box<Node>, Box<str>, Box<Node>),
    Comparison(Box<Node>, Box<str>, Box<Node>),
    Replace(Box<Node>, Box<str>, Box<Node>, Box<Node>),
    Term(Box<Node>, Box<str>, Box<Node>),
    Factor(Box<Node>, Box<str>, Box<Node>),
    Unary(Box<str>, Box<Node>),
    Cast(Box<Node>, Box<Node>),
    Glob(Box<str>),
    Label(Box<str>),
    Regex(Box<str>),
    Field(Box<str>),
    String(Box<str>),
    File(Box<Path>),
    Integer(i128),
    Float(f64),
    GetItem(Box<Node>, Box<Node>),
    GetAttr(Box<Node>, Box<str>),
    Path(Box<Node>, Box<str>),
    Substitution(JobNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode),
}

fn propose_name(name: &str, v: ValueDefinition) -> ValueDefinition {
    match v {
        ValueDefinition::ClosureDefinition(_, p, j) =>
            ValueDefinition::ClosureDefinition(Some(Box::from(name)), p, j),
        o => o
    }
}

impl Node {
    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(
            match self {
                Node::Assignment(target, op, value) =>
                    match op.deref() {
                        "=" =>
                            return match target.as_ref() {
                                Node::Label(t) => Ok(ArgumentDefinition::named(t.deref(), propose_name( &t, value.generate_argument()?.unnamed_value()?))),
                                _ => error("Invalid left side in named argument"),
                            },
                        _ =>
                            return error("Invalid assignment operator"),
                    }

                Node::LogicalOperation(_, _, _) | Node::Comparison(_, _, _) | Node::Replace(_, _, _, _) |
                Node::GetItem(_, _) | Node::Term(_, _, _) | Node::Factor(_, _, _) =>
                    ValueDefinition::JobDefinition(
                        Job::new(vec![self.generate_standalone()?.unwrap()])
                    ),
                Node::Unary(op, r) =>
                    match op.deref() {
                        "neg" | "not" | "typeof" =>
                            ValueDefinition::JobDefinition(
                                Job::new(vec![self.generate_standalone()?.unwrap()])
                            ),
                        "@" =>
                            return Ok(ArgumentDefinition::list(r.generate_argument()?.unnamed_value()?)),
                        "@@" =>
                            return Ok(ArgumentDefinition::dict(r.generate_argument()?.unnamed_value()?)),
                        _ => return error("Unknown operator"),
                    },
                Node::Cast(value, target_type) =>
                    ValueDefinition::JobDefinition(
                        Job::new(vec![CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(AS.as_ref().clone())),
                            vec![value.generate_argument()?, target_type.generate_argument()?])
                        ])),
                Node::Label(l) => ValueDefinition::Label(l.clone()),
                Node::Regex(l) => ValueDefinition::Value(Value::Regex(l.clone(), to_crush_error(Regex::new(l.clone().as_ref()))?)),
                Node::String(t) => ValueDefinition::Value(Value::String(unescape(t).into_boxed_str())),
                Node::Integer(i) => ValueDefinition::Value(Value::Integer(i.clone())),
                Node::Float(f) => ValueDefinition::Value(Value::Float(f.clone())),
                Node::GetAttr(node, label) => {
                    let parent = node.generate_argument()?;
                    match parent.unnamed_value()? {
                        ValueDefinition::Value(Value::Field(mut f)) => {
                            f.push(label.clone());
                            ValueDefinition::Value(Value::Field(f))
                        }
                        value => ValueDefinition::GetAttr(Box::new(value), label.clone())
                    }
                }
                Node::Path(node, label) =>
                    ValueDefinition::Path(Box::new(node.generate_argument()?.unnamed_value()?), label.clone()),
                Node::Field(f) => ValueDefinition::Value(Value::Field(vec![f[1..].to_string().into_boxed_str()])),
                Node::Substitution(s) => ValueDefinition::JobDefinition(s.generate()?),
                Node::Closure(s, c) => {
                    let param = s.as_ref().map(|v| v.iter()
                        .map(|p| p.generate())
                        .collect::<CrushResult<Vec<Parameter>>>());
                    let p = match param {
                        None => None,
                        Some(Ok(p)) => Some(p),
                        Some(Err(e)) => return Err(e),
                    };
                    ValueDefinition::ClosureDefinition(None, p, c.generate()?)
                }
                Node::Glob(g) => ValueDefinition::Value(Value::Glob(Glob::new(&g))),
                Node::File(f) => ValueDefinition::Value(Value::File(f.clone())),
            }))
    }

    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            Node::Assignment(target, op, value) => {
                match op.deref() {
                    "=" => {
                        match target.as_ref() {
                            Node::Label(t) =>
                                Node::function_invocation(
                                    SET.as_ref().clone(),
                                    vec![ArgumentDefinition::named(t, propose_name(&t, value.generate_argument()?.unnamed_value()?))]),

                            Node::GetItem(container, key) =>
                                container.method_invocation("__setitem__", vec![
                                    ArgumentDefinition::unnamed(key.generate_argument()?.unnamed_value()?),
                                    ArgumentDefinition::unnamed(value.generate_argument()?.unnamed_value()?)]),

                            Node::GetAttr(container, attr) =>
                                container.method_invocation("__setattr__", vec![
                                    ArgumentDefinition::unnamed(ValueDefinition::Value(Value::String(attr.to_string().into_boxed_str()))),
                                    ArgumentDefinition::unnamed(value.generate_argument()?.unnamed_value()?),
                                ]),

                            _ => error("Invalid left side in assignment"),
                        }
                    }
                    ":=" => {
                        match target.as_ref() {
                            Node::Label(t) =>
                                Node::function_invocation(
                                    LET.as_ref().clone(),
                                    vec![ArgumentDefinition::named(t, propose_name(&t,value.generate_argument()?.unnamed_value()?))]),
                            _ => error("Invalid left side in declaration"),
                        }
                    }
                    _ => error("Unknown assignment operator"),
                }
            }

            Node::LogicalOperation(l, op, r) => {
                let cmd = match op.as_ref() {
                    "and" => AND.as_ref(),
                    "or" => OR.as_ref(),
                    _ => return error("Unknown operator")
                };
                Node::function_invocation(cmd.clone(), vec![l.generate_argument()?, r.generate_argument()?])
            }
            Node::Comparison(l, op, r) => {
                let cmd = match op.as_ref() {
                    "<" => LT.as_ref(),
                    "<=" => LTE.as_ref(),
                    ">" => GT.as_ref(),
                    ">=" => GTE.as_ref(),
                    "==" => EQ.as_ref(),
                    "!=" => NEQ.as_ref(),
                    "=~" =>
                        return l.method_invocation("match", vec![r.generate_argument()?]),
                    "!~" =>
                        return l.method_invocation("not_match", vec![r.generate_argument()?]),
                    _ => return error("Unknown operator"),
                };
                Node::function_invocation(cmd.clone(), vec![l.generate_argument()?, r.generate_argument()?])
            }
            Node::Replace(r, op, t1, t2) => {
                let method = match op.as_ref() {
                    "~" => "replace",
                    "~~" => "replace_all",
                    _ => return error("Unknown operator")
                };
                r.method_invocation(method, vec![t1.generate_argument()?, t2.generate_argument()?])
            }
            Node::Term(l, op, r) => {
                let method = match op.as_ref() {
                    "+" => "__add__",
                    "-" => "__sub__",
                    _ => return error("Unknown operator"),
                };
                l.method_invocation(method, vec![r.generate_argument()?])
            }
            Node::Factor(l, op, r) => {
                let method = match op.as_ref() {
                    "*" => "__mul__",
                    "//" => "__div__",
                    _ => return error("Unknown operator"),
                };
                l.method_invocation(method, vec![r.generate_argument()?])
            }
            Node::GetItem(val, key) => {
                val.method_invocation("__getitem__", vec![key.generate_argument()?])
            }
            Node::Unary(op, r) =>
                match op.deref() {
                    "neg" => r.method_invocation("__neg__", vec![]),
                    "not" =>
                        Node::function_invocation(NOT.as_ref().clone(), vec![r.generate_argument()?]),
                    "typeof" =>
                        Node::function_invocation(TYPEOF.as_ref().clone(), vec![r.generate_argument()?]),
                    "@" | "@@" => Ok(None),
                    _ => return error("Unknown operator"),
                },

            Node::Cast(_, _) | Node::Glob(_) | Node::Label(_) | Node::Regex(_) | Node::Field(_) | Node::String(_) |
            Node::Integer(_) | Node::Float(_) | Node::GetAttr(_, _) | Node::Path(_, _) | Node::Substitution(_) |
            Node::Closure(_, _) | Node::File(_) => Ok(None),
        }
    }

    fn function_invocation(function: Box<dyn CrushCommand + Send + Sync>, arguments: Vec<ArgumentDefinition>) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(
            CommandInvocation::new(
                ValueDefinition::Value(Value::Command(function)),
                arguments)))
    }

    fn method_invocation(&self, name: &str, arguments: Vec<ArgumentDefinition>) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(
            CommandInvocation::new(
                ValueDefinition::GetAttr(Box::from(self.generate_argument()?.unnamed_value()?), name.to_string().into_boxed_str()),
                arguments)
        ))
    }

    pub fn parse_label(s: &str) -> Box<Node> {
        if s.contains('%') || s.contains('?') {
            Box::from(Node::Glob(Box::from(s)))
        } else {
            if s.contains('/') {
                if s.starts_with('/') {
                    Box::from(Node::File(Box::from(Path::new(s))))
                } else {
                    let parts = s.split('/').collect::<Vec<&str>>();
                    let mut res = Node::Label(Box::from(parts[0]));
                    for part in &parts[1..] {
                        res = Node::Path(Box::from(res), Box::from(part.clone()))
                    }
                    Box::from(res)
                }
            } else {
                Box::from(Node::Label(Box::from(s)))
            }
        }
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
    Parameter(Box<str>, Option<Box<Node>>, Option<Node>),
    Named(Box<str>),
    Unnamed(Box<str>),
}

impl ParameterNode {
    pub fn generate(&self) -> CrushResult<Parameter> {
        match self {
            ParameterNode::Parameter(name, value_type, default) =>
                Ok(
                    Parameter::Parameter(
                        name.clone(),
                        value_type.as_ref().map(|t| t.generate_argument()?.unnamed_value()).unwrap_or(
                            Ok(ValueDefinition::Value(Value::Type(ValueType::Any)))
                        )?,
                        default.as_ref()
                            .map(|d| d.generate_argument()).transpose()?
                            .map(|a| a.unnamed_value()).transpose()?,
                    )
                ),
            ParameterNode::Named(s) => Ok(Parameter::Named(s.clone())),
            ParameterNode::Unnamed(s) => Ok(Parameter::Unnamed(s.clone())),
        }
    }
}
