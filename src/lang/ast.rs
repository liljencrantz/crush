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
    pub expressions: Vec<AssignmentNode>,
}

impl CommandNode {
    pub fn generate(&self) -> CrushResult<CommandInvocation> {
        let s = self.expressions[0].generate_standalone()?;
        if let Some(c) = s {
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


pub enum AssignmentNode {
    Assignment(ItemNode, Box<AssignmentNode>),
    Declaration(ItemNode, Box<AssignmentNode>),
    Logical(LogicalNode),
}

impl AssignmentNode {
    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            AssignmentNode::Assignment(target, value) =>
                match target {
                    ItemNode::Label(t) => Ok(ArgumentDefinition::named(t.deref(), value.generate_argument()?.unnamed_value()?)),
                    _ => error("Invalid left side in named argument"),
                },
            AssignmentNode::Declaration(target, value) =>
                error("Variable declarations not supported as arguments"),
            AssignmentNode::Logical(l) => l.generate_argument(),
        }
    }

    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            AssignmentNode::Logical(e) => e.generate_standalone(),
            AssignmentNode::Assignment(target, value) => {
                match target {
                    ItemNode::Label(t) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(SET.as_ref().clone())),
                            vec![ArgumentDefinition::named(t, value.generate_argument()?.unnamed_value()?)])
                    )),
                    ItemNode::GetItem(container, key) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::GetAttr(
                                Box::from(container.generate_argument()?.unnamed_value()?),
                                Box::from("__setitem__")),
                            vec![
                                ArgumentDefinition::unnamed(key.generate_argument()?.unnamed_value()?),
                                ArgumentDefinition::unnamed(value.generate_argument()?.unnamed_value()?),
                            ]))),
                    ItemNode::GetAttr(container, attr) => Ok(Some(
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
            AssignmentNode::Declaration(target, value) => {
                match target {
                    ItemNode::Label(t) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(LET.as_ref().clone())),
                            vec![ArgumentDefinition::named(t, value.generate_argument()?.unnamed_value()?)])
                    )),
                    _ => error("Invalid left side in assignment"),
                    ItemNode::Integer(_) => error("Invalid left side in assignment"),
                    ItemNode::Float(_) => error("Invalid left side in assignment"),
                    ItemNode::GetItem(_, _) => error("Invalid left side in assignment"),
                    ItemNode::Path(_, _) => error("Invalid left side in assignment"),
                }
            }
        }
    }
}

pub enum LogicalNode {
    LogicalOperation(Box<LogicalNode>, Box<str>, ComparisonNode),
    Comparison(ComparisonNode),
}

impl LogicalNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            LogicalNode::LogicalOperation(l, op, r) => {
                let cmd = match op.as_ref() {
                    "and" => AND.as_ref(),
                    "or" => OR.as_ref(),
                    _ => return error("Unknown operator")
                };
                Ok(Some(CommandInvocation::new(
                    ValueDefinition::Value(Value::Command(cmd.clone())),
                    vec![l.generate_argument()?, r.generate_argument()?])))
            }
            LogicalNode::Comparison(c) => c.generate_standalone(),
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            LogicalNode::LogicalOperation(_, _, _) =>
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                ))),
            LogicalNode::Comparison(c) =>
                c.generate_argument(),
        }
    }
}

pub enum ComparisonNode {
    Comparison(Box<ComparisonNode>, Box<str>, TermNode),
    Replace(Box<ComparisonNode>, Box<str>, TermNode, TermNode),
    Term(TermNode),
}

impl ComparisonNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            ComparisonNode::Comparison(l, op, r) => {
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
            ComparisonNode::Replace(r, op, t1, t2) => {
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

            ComparisonNode::Term(t) =>
                t.generate_standalone(),
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            ComparisonNode::Comparison(_, _, _) | ComparisonNode::Replace(_, _, _, _) =>
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                ))),

            ComparisonNode::Term(t) => t.generate_argument(),
        }
    }
}

pub enum TermNode {
    Term(Box<TermNode>, Box<str>, FactorNode),
    Factor(FactorNode),
}

impl TermNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            TermNode::Term(l, op, r) => {
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
            TermNode::Factor(f) => f.generate_standalone(),
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            TermNode::Term(_, _, _) =>
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                ))),
            TermNode::Factor(f) => f.generate_argument(),
        }
    }
}

pub enum FactorNode {
    Factor(Box<FactorNode>, Box<str>, UnaryNode),
    Unary(UnaryNode),
}

impl FactorNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            FactorNode::Factor(l, op, r) => {
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
            FactorNode::Unary(u) => u.generate_standalone(),
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            FactorNode::Factor(_, _, _) =>
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                ))),
            FactorNode::Unary(u) =>
                u.generate_argument(),
        }
    }
}

pub enum UnaryNode {
    Unary(Box<str>, Box<UnaryNode>),
    Item(ItemNode),
}

impl UnaryNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        Ok(None)
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            UnaryNode::Unary(op, r) =>
                match op.deref() {
                    "!" =>
                        Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                            Job::new(vec![CommandInvocation::new(
                                ValueDefinition::Value(Value::Command(NOT.as_ref().clone())),
                                vec![r.generate_argument()?])
                            ])))),
                    _ => error("Unknown operator")
                },
            UnaryNode::Item(i) => i.generate_argument(),
        }
    }
}

pub enum ItemNode {
    Glob(Box<str>),
    Label(Box<str>),
    Regex(Box<str>),
    Field(Box<str>),
    String(Box<str>),
    Integer(i128),
    Float(f64),
    GetItem(Box<ItemNode>, Box<AssignmentNode>),
    GetAttr(Box<ItemNode>, Box<str>),
    Path(Box<ItemNode>, Box<str>),
    Substitution(JobNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode),
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

impl ItemNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        Ok(None)
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(match self {
            ItemNode::Label(l) => ValueDefinition::Label(l.clone()),
            ItemNode::Regex(l) => ValueDefinition::Value(Value::Regex(l.clone(), to_crush_error(Regex::new(l.clone().as_ref()))?)),
            ItemNode::String(t) => ValueDefinition::Value(Value::String(unescape(t).into_boxed_str())),
            ItemNode::Integer(i) => ValueDefinition::Value(Value::Integer(i.clone())),
            ItemNode::Float(f) => ValueDefinition::Value(Value::Float(f.clone())),
            ItemNode::GetItem(node, field) =>
                ValueDefinition::GetItem(
                    Box::new(node.generate_argument()?.unnamed_value()?),
                    Box::new(field.generate_argument()?.unnamed_value()?)),
            ItemNode::GetAttr(node, label) => {
                let parent = node.generate_argument()?;
                match parent.unnamed_value()? {
                    ValueDefinition::Value(Value::Field(mut f)) => {
                        f.push(label.clone());
                        ValueDefinition::Value(Value::Field(f))
                    }
                    value => ValueDefinition::GetAttr(Box::new(value), label.clone())
                }
            }
            ItemNode::Path(node, label) =>
                ValueDefinition::Path(Box::new(node.generate_argument()?.unnamed_value()?), label.clone()),
            ItemNode::Field(f) => ValueDefinition::Value(Value::Field(vec![f[1..].to_string().into_boxed_str()])),
            ItemNode::Substitution(s) => ValueDefinition::JobDefinition(s.generate()?),
            ItemNode::Closure(s, c) => {
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
            ItemNode::Glob(g) => ValueDefinition::Value(Value::Glob(Glob::new(&g))),
        }))
    }
}

pub enum ParameterNode {
    Parameter(Box<str>, ValueDefinition, Option<LogicalNode>),
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
