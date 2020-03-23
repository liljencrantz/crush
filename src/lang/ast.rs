use crate::lang::job::Job;
use crate::lang::errors::{CrushResult, error};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::argument::ArgumentDefinition;
use crate::lang::value::{ValueDefinition, Value};
use std::ops::Deref;
use crate::lang::command::CrushCommand;
use crate::util::glob::Glob;
use lazy_static::lazy_static;
use crate::lib::math;
use crate::lib::comp;
use crate::lib::cond;
use crate::lib::var;

lazy_static! {
    pub static ref ADD: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(math::add, false)};
    pub static ref SUB: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(math::sub, false)};
    pub static ref MUL: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(math::mul, false)};
    pub static ref DIV: Box<dyn CrushCommand + Send + Sync> = {CrushCommand::command(math::div, false)};

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

#[derive(Debug)]
pub struct JobListNode {
    pub jobs: Vec<JobNode>,
}

impl JobListNode {
    pub fn generate(&self) -> CrushResult<Vec<Job>> {
        self.jobs.iter().map(|j| j.generate()).collect()
    }
}

#[derive(Debug)]
pub struct JobNode {
    pub commands: Vec<CommandNode>,
}

impl JobNode {
    pub fn generate(&self) -> CrushResult<Job> {
        Ok(Job::new(self.commands.iter().map(|c| c.generate()).collect::<CrushResult<Vec<CommandInvocation>>>()?))
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum AssignmentNode {
    Assignment(ItemNode, Box<AssignmentNode>),
    Declaration(ItemNode, Box<AssignmentNode>),
    Logical(LogicalNode),
}

impl AssignmentNode {
    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            AssignmentNode::Assignment(target, value) => {
                match target {
                    ItemNode::Label(t) => Ok(ArgumentDefinition::named(t.deref(), value.generate_argument()?.unnamed_value()?)),
                    ItemNode::QuotedLabel(t) => Ok(ArgumentDefinition::named(unescape(t).as_str(), value.generate_argument()?.unnamed_value()?)),
                    _ => error("Invalid left side in named argument"),
                }
            }
            AssignmentNode::Declaration(target, value) => {
                error("Variable declarations not supported as arguments")
            }
            AssignmentNode::Logical(l) => {
                l.generate_argument()
            }
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
                    ItemNode::QuotedLabel(t) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(SET.as_ref().clone())),
                            vec![ArgumentDefinition::named(unescape(t).as_str(), value.generate_argument()?.unnamed_value()?)])
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
                    ItemNode::QuotedLabel(t) => Ok(Some(
                        CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(LET.as_ref().clone())),
                            vec![ArgumentDefinition::named(unescape(t).as_str(), value.generate_argument()?.unnamed_value()?)])
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

#[derive(Debug)]
pub enum LogicalNode {
    LogicalOperation(Box<LogicalNode>, Box<str>, ComparisonNode),
    Comparison(ComparisonNode),
}

impl LogicalNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            LogicalNode::LogicalOperation(l, op, r) => {
                match op.as_ref() {
                    "&&" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(AND.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "||" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(OR.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    _ => error("Unknown operator")
                }
            }
            LogicalNode::Comparison(c) => {
                c.generate_standalone()
            }
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            LogicalNode::LogicalOperation(l, op, r) => {
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                )))
            }
            LogicalNode::Comparison(c) => {
                c.generate_argument()
            }
        }
    }
}

#[derive(Debug)]
pub enum ComparisonNode {
    Comparison(Box<ComparisonNode>, Box<str>, TermNode),
    Term(TermNode),
}

impl ComparisonNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            ComparisonNode::Comparison(l, op, r) => {
                match op.as_ref() {
                    "<" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(LT.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "<=" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(LTE.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    ">" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(GT.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    ">=" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(GTE.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "==" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(EQ.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "!=" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(NEQ.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    _ => error("Unknown operator")
                }
            }
            ComparisonNode::Term(t) => {
                t.generate_standalone()
            }
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            ComparisonNode::Comparison(l, op, r) => {
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                )))
            }
            ComparisonNode::Term(t) => {
                t.generate_argument()
            }
        }
    }
}


#[derive(Debug)]
pub enum TermNode {
    Term(Box<TermNode>, Box<str>, FactorNode),
    Factor(FactorNode),
}

impl TermNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            TermNode::Term(l, op, r) => {
                match op.as_ref() {
                    "+" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(ADD.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "-" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(SUB.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    _ => error("Unknown operator")
                }
            }
            TermNode::Factor(f) =>
                f.generate_standalone(),
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            TermNode::Term(l, op, r) => {
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                )))
            }
            TermNode::Factor(f) => {
                f.generate_argument()
            }
        }
    }
}

#[derive(Debug)]
pub enum FactorNode {
    Factor(Box<FactorNode>, Box<str>, UnaryNode),
    Unary(UnaryNode),
}

impl FactorNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CommandInvocation>> {
        match self {
            FactorNode::Factor(l, op, r) => {
                match op.as_ref() {
                    "*" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(MUL.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "//" => {
                        Ok(Some(CommandInvocation::new(
                            ValueDefinition::Value(Value::Command(DIV.as_ref().clone())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    _ => error(format!("Unknown operator {}", op).as_str())
                }
            }
            FactorNode::Unary(u) => {
                u.generate_standalone()
            }
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            FactorNode::Factor(l, op, r) => {
                Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                    Job::new(vec![self.generate_standalone()?.unwrap()])
                )))
            }
            FactorNode::Unary(u) => {
                u.generate_argument()
            }
        }
    }
}

#[derive(Debug)]
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
            UnaryNode::Unary(op, r) => {
                match op.deref() {
                    "!" => {
                        Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                            Job::new(vec![CommandInvocation::new(
                                ValueDefinition::Value(Value::Command(NOT.as_ref().clone())),
                                vec![r.generate_argument()?])
                            ]))))
                    }
                    _ => error("Unknown operator")
                }
            }
            UnaryNode::Item(i) => {
                i.generate_argument()
            }
        }
    }
}

#[derive(Debug)]
pub enum ItemNode {
    Glob(Box<str>),
    Label(Box<str>),
    Field(Box<str>),
    QuotedLabel(Box<str>),
    String(Box<str>),
    Integer(i128),
    Float(f64),
    GetItem(Box<ItemNode>, Box<AssignmentNode>),
    GetAttr(Box<ItemNode>, Box<str>),
    Path(Box<ItemNode>, Box<str>),
    Substitution(JobNode),
    Closure(JobListNode),
}

fn unescape(s: &str) -> String {
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
            ItemNode::QuotedLabel(t) => ValueDefinition::Label(unescape(t).into_boxed_str()),
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
            },
            ItemNode::Path(node, label) =>
                ValueDefinition::Path(Box::new(node.generate_argument()?.unnamed_value()?), label.clone()),
            ItemNode::Field(f) => ValueDefinition::Value(Value::Field(vec![f[1..].to_string().into_boxed_str()])),
            ItemNode::Substitution(s) =>
                    ValueDefinition::JobDefinition(s.generate()?),
            ItemNode::Closure(c) =>
                    ValueDefinition::ClosureDefinition(c.generate()?),
            ItemNode::Glob(g) => ValueDefinition::Value(Value::Glob(Glob::new(&g))),
        }))
    }
}
