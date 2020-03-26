use crate::lang::job::Job;
use crate::lang::errors::{CrushResult, error, to_crush_error};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::argument::ArgumentDefinition;
use crate::lang::value::{ValueDefinition, Value};
use std::ops::Deref;
use crate::lang::command::{CrushCommand, Parameter};
use crate::util::glob::Glob;
use lazy_static::lazy_static;
use crate::lib::comp;
use crate::lib::cond;
use crate::lib::var;
use crate::lib::types;
use regex::Regex;

lazy_static! {
    pub static ref LT: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(comp::lt, false)};
    pub static ref LTE: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(comp::lte, false)};
    pub static ref GT: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(comp::gt, false)};
    pub static ref GTE: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(comp::gte, false)};
    pub static ref EQ: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(comp::eq, false)};
    pub static ref NEQ: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(comp::neq, false)};
    pub static ref NOT: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(comp::not, false)};

    pub static ref AND: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(cond::and, false)};
    pub static ref OR: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(cond::or, false)};

    pub static ref LET: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(var::r#let, false)};
    pub static ref SET: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(var::set, false)};

    pub static ref AS: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(types::r#as, false)};
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
    Assignment(Box<Node>, Box<Node>),
    Declaration(Box<Node>, Box<Node>),
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
    Integer(i128),
    Float(f64),
    GetItem(Box<Node>, Box<Node>),
    GetAttr(Box<Node>, Box<str>),
    Path(Box<Node>, Box<str>),
    Substitution(JobNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode),
}

impl Node {
    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(
            match self {
                Node::Assignment(target, value) =>
                    return match target.as_ref() {
                        Node::Label(t) => Ok(ArgumentDefinition::named(t.deref(), value.generate_argument()?.unnamed_value()?)),
                        _ => error("Invalid left side in named argument"),
                    },

                Node::Declaration(target, value) =>
                    return error("Variable declarations not supported as arguments"),

                Node::LogicalOperation(_, _, _) | Node::Comparison(_, _, _) | Node::Replace(_, _, _, _) |
                Node::Term(_, _, _) | Node::Factor(_, _, _) =>
                    ValueDefinition::JobDefinition(
                        Job::new(vec![self.generate_standalone()?.unwrap()])
                    ),
                Node::Unary(op, r) =>
                    match op.deref() {
                        "neg" =>
                            ValueDefinition::JobDefinition(
                                Job::new(vec![
                                    CommandInvocation::new(
                                        ValueDefinition::GetAttr(Box::from(r.generate_argument()?.unnamed_value()?), "__neg__".to_string().into_boxed_str()),
                                        vec![])
                                ]))
                        ,
                        "not" =>
                            ValueDefinition::JobDefinition(
                                Job::new(vec![CommandInvocation::new(
                                    ValueDefinition::Value(Value::Command(NOT.as_ref().clone())),
                                    vec![r.generate_argument()?])
                                ])),
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
                Node::GetItem(node, field) =>
                    ValueDefinition::GetItem(
                        Box::new(node.generate_argument()?.unnamed_value()?),
                        Box::new(field.generate_argument()?.unnamed_value()?)),
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
                    ValueDefinition::ClosureDefinition(p, c.generate()?)
                }
                Node::Glob(g) => ValueDefinition::Value(Value::Glob(Glob::new(&g))),
            }))
    }

    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            Node::Assignment(target, value) => {
                match target.as_ref() {
                    Node::Label(t) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(SET.as_ref().clone())),
                            vec![ArgumentDefinition::named(t, value.generate_argument()?.unnamed_value()?)])
                    )),
                    Node::GetItem(container, key) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::GetAttr(
                                Box::from(container.generate_argument()?.unnamed_value()?),
                                Box::from("__setitem__")),
                            vec![
                                ArgumentDefinition::unnamed(key.generate_argument()?.unnamed_value()?),
                                ArgumentDefinition::unnamed(value.generate_argument()?.unnamed_value()?),
                            ]))),
                    Node::GetAttr(container, attr) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::GetAttr(
                                Box::from(container.generate_argument()?.unnamed_value()?),
                                Box::from("__setattr__")),
                            vec![
                                ArgumentDefinition::unnamed(ValueDefinition::Value(Value::String(attr.to_string().into_boxed_str()))),
                                ArgumentDefinition::unnamed(value.generate_argument()?.unnamed_value()?),
                            ]))),

                    _ => error("Invalid left side in assignment"),
                }
            }
            Node::Declaration(target, value) => {
                match target.as_ref() {
                    Node::Label(t) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(LET.as_ref().clone())),
                            vec![ArgumentDefinition::named(t, value.generate_argument()?.unnamed_value()?)])
                    )),
                    _ => error("Invalid left side in declaration"),
                }
            }
            Node::LogicalOperation(l, op, r) => {
                let cmd = match op.as_ref() {
                    "and" => AND.as_ref(),
                    "or" => OR.as_ref(),
                    _ => return error("Unknown operator")
                };
                Ok(Some(CommandInvocation::new(
                    ValueDefinition::Value(Value::Command(cmd.clone())),
                    vec![l.generate_argument()?, r.generate_argument()?])))
            }
            Node::Comparison(l, op, r) => {
                match op.as_ref() {
                    "<" =>
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(LT.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        )),
                    "<=" =>
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(LTE.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        )),
                    ">" =>
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(GT.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        )),
                    ">=" =>
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(GTE.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        )),
                    "==" =>
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(EQ.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        )),
                    "!=" =>
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(NEQ.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        )),
                    "=~" =>
                        Ok(Some(
                            CommandInvocation::new(
                                ValueDefinition::GetAttr(Box::from(l.generate_argument()?.unnamed_value()?), "match".to_string().into_boxed_str()),
                                vec![r.generate_argument()?])
                        )),
                    "!~" =>
                        Ok(Some(
                            CommandInvocation::new(
                                ValueDefinition::GetAttr(Box::from(l.generate_argument()?.unnamed_value()?), "not_match".to_string().into_boxed_str()),
                                vec![r.generate_argument()?])
                        )),
                    _ => error("Unknown operator"),
                }
            }
            Node::Replace(r, op, t1, t2) => {
                let cmd = match op.as_ref() {
                    "~" => "replace",
                    "~~" => "replace_all",
                    _ => return error("Unknown operator")
                };

                Ok(Some(
                    CommandInvocation::new(
                        ValueDefinition::GetAttr(Box::from(r.generate_argument()?.unnamed_value()?), cmd.to_string().into_boxed_str()),
                        vec![t1.generate_argument()?, t2.generate_argument()?])
                ))
            }
            Node::Term(l, op, r) => {
                let method = match op.as_ref() {
                    "+" => "__add__",
                    "-" => "__sub__",
                    _ => return error("Unknown operator"),
                };
                Ok(Some(
                    CommandInvocation::new(
                        ValueDefinition::GetAttr(Box::from(l.generate_argument()?.unnamed_value()?), method.to_string().into_boxed_str()),
                        vec![r.generate_argument()?])
                ))
            }
            Node::Factor(l, op, r) => {
                let method = match op.as_ref() {
                    "*" => "__mul__",
                    "//" => "__div__",
                    _ => return error("Unknown operator"),
                };
                Ok(Some(
                    CommandInvocation::new(
                        ValueDefinition::GetAttr(Box::from(l.generate_argument()?.unnamed_value()?), method.to_string().into_boxed_str()),
                        vec![r.generate_argument()?])
                ))
            }

            Node::Unary(op, r) =>
                match op.deref() {
                    "neg" =>
                        Ok(Some(
                                CommandInvocation::new(
                                    ValueDefinition::GetAttr(Box::from(r.generate_argument()?.unnamed_value()?), "__neg__".to_string().into_boxed_str()),
                                    vec![]))),
                    "not" =>
                        Ok(Some(
                            CommandInvocation::new(
                                ValueDefinition::Value(Value::Command(NOT.as_ref().clone())),
                                vec![r.generate_argument()?]))),
                    "@" | "@@" =>
                        Ok(None),
                    _ => return error("Unknown operator"),
                },

            Node::Cast(_, _) | Node::Glob(_) | Node::Label(_) | Node::Regex(_) | Node::Field(_) | Node::String(_) |
            Node::Integer(_) | Node::Float(_) | Node::GetItem(_, _) | Node::GetAttr(_, _) | Node::Path(_, _) | Node::Substitution(_) |
            Node::Closure(_, _) => Ok(None),
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
    Parameter(Box<str>, ValueDefinition, Option<Node>),
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
                        value_type.clone(),
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
