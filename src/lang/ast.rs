use crate::lang::job::Job;
use crate::lang::errors::{CrushResult, error, argument_error, parse_error};
use crate::lang::call_definition::CallDefinition;
use crate::lang::argument::ArgumentDefinition;
use crate::lang::value::{ValueDefinition, Value};
use std::ops::Deref;
use crate::lang::command::{SimpleCommand, CrushCommand};

static ADD: SimpleCommand = SimpleCommand { call:crate::lib::math::add, can_block:false};
static SUB: SimpleCommand = SimpleCommand { call:crate::lib::math::sub, can_block:false};
static MUL: SimpleCommand = SimpleCommand { call:crate::lib::math::mul, can_block:false};
static DIV: SimpleCommand = SimpleCommand { call:crate::lib::math::div, can_block:false};

static LT: SimpleCommand = SimpleCommand { call:crate::lib::comp::lt, can_block:true};
static LTE: SimpleCommand = SimpleCommand { call:crate::lib::comp::lte, can_block:true};
static GT: SimpleCommand = SimpleCommand { call:crate::lib::comp::gt, can_block:true};
static GTE: SimpleCommand = SimpleCommand { call:crate::lib::comp::gte, can_block:true};
static EQ: SimpleCommand = SimpleCommand { call:crate::lib::comp::eq, can_block:true};
static NEQ: SimpleCommand = SimpleCommand { call:crate::lib::comp::neq, can_block:true};
static NOT: SimpleCommand = SimpleCommand { call:crate::lib::comp::not, can_block:true};

static AND: SimpleCommand = SimpleCommand { call:crate::lib::cond::and, can_block:true};
static OR: SimpleCommand = SimpleCommand { call:crate::lib::cond::or, can_block:true};

static LET: SimpleCommand = SimpleCommand { call:crate::lib::var::r#let, can_block:false};
static SET: SimpleCommand = SimpleCommand { call:crate::lib::var::set, can_block:false};

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
        Ok(Job::new(self.commands.iter().map(|c| c.generate()).collect::<CrushResult<Vec<CallDefinition>>>()?))
    }
}

#[derive(Debug)]
pub struct CommandNode {
    pub expressions: Vec<AssignmentNode>,
}

impl CommandNode {
    pub fn generate(&self) -> CrushResult<CallDefinition> {
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
            Ok(CallDefinition::new(cmd.unnamed_value()?, arguments))
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

    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        match self {
            AssignmentNode::Logical(e) => e.generate_standalone(),
            AssignmentNode::Assignment(target, value) => {
                match target {
                    ItemNode::Label(t) => Ok(Some(
                        CallDefinition::new(
                            ValueDefinition::Value(Value::Command(SET.boxed())),
                            vec![ArgumentDefinition::named(t, value.generate_argument()?.unnamed_value()?)])
                    )),
                    ItemNode::QuotedLabel(t) => Ok(Some(
                        CallDefinition::new(
                            ValueDefinition::Value(Value::Command(SET.boxed())),
                            vec![ArgumentDefinition::named(unescape(t).as_str(), value.generate_argument()?.unnamed_value()?)])
                    )),
                    ItemNode::Get(container, key) => Ok(Some(
                        CallDefinition::new(
                        ValueDefinition::Path(
                            Box::from(container.generate_argument()?.unnamed_value()?),
                            Box::from("setitem")),
                        vec![
                            ArgumentDefinition::unnamed(key.generate_argument()?.unnamed_value()?),
                            ArgumentDefinition::unnamed(value.generate_argument()?.unnamed_value()?),
                        ]))),

                    ItemNode::Path(_, _) => unimplemented!(),
                    ItemNode::Field(_) => unimplemented!(),

                    _ => error("Invalid left side in assignment"),
                }
            }
            AssignmentNode::Declaration(target, value) => {
                match target {
                    ItemNode::Label(t) => Ok(Some(
                        CallDefinition::new(
                            ValueDefinition::Value(Value::Command(LET.boxed())),
                            vec![ArgumentDefinition::named(t, value.generate_argument()?.unnamed_value()?)])
                    )),
                    ItemNode::QuotedLabel(t) => Ok(Some(
                        CallDefinition::new(
                            ValueDefinition::Value(Value::Command(LET.boxed())),
                            vec![ArgumentDefinition::named(unescape(t).as_str(), value.generate_argument()?.unnamed_value()?)])
                    )),
                    _ => error("Invalid left side in assignment"),
                    ItemNode::Integer(_) => error("Invalid left side in assignment"),
                    ItemNode::Float(_) => error("Invalid left side in assignment"),
                    ItemNode::Get(_, _) => error("Invalid left side in assignment"),
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
    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        match self {
            LogicalNode::LogicalOperation(l, op, r) => {
                match op.as_ref() {
                    "&&" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(AND.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "||" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(OR.boxed())),
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
    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        match self {
            ComparisonNode::Comparison(l, op, r) => {
                match op.as_ref() {
                    "<" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(LT.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "<=" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(LTE.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    ">" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(GT.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    ">=" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(GTE.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "==" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(EQ.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "!=" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(NEQ.boxed())),
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
    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        match self {
            TermNode::Term(l, op, r) => {
                match op.as_ref() {
                    "+" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(ADD.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "-" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(SUB.boxed())),
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
    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        match self {
            FactorNode::Factor(l, op, r) => {
                match op.as_ref() {
                    "*" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(MUL.boxed())),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "//" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Value(Value::Command(DIV.boxed())),
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
    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        Ok(None)
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            UnaryNode::Unary(op, r) => {
                match op.deref() {
                    "!" => {
                        Ok(ArgumentDefinition::unnamed(ValueDefinition::JobDefinition(
                            Job::new(vec![CallDefinition::new(
                                ValueDefinition::Value(Value::Command(NOT.boxed())),
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
    Label(Box<str>),
    Field(Box<str>),
    QuotedLabel(Box<str>),
    String(Box<str>),
    Integer(i128),
    Float(f64),
    Get(Box<ItemNode>, Box<AssignmentNode>),
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
    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        Ok(None)
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(match self {
            ItemNode::Label(l) => ValueDefinition::Label(l.clone()),
            ItemNode::QuotedLabel(t) => ValueDefinition::Label(unescape(t).into_boxed_str()),
            ItemNode::String(t) => ValueDefinition::Value(Value::String(unescape(t).into_boxed_str())),
            ItemNode::Integer(i) => ValueDefinition::Value(Value::Integer(i.clone())),
            ItemNode::Float(f) => ValueDefinition::Value(Value::Float(f.clone())),
            ItemNode::Get(node, field) =>
                ValueDefinition::GetItem(
                    Box::new(node.generate_argument()?.unnamed_value()?),
                    Box::new(field.generate_argument()?.unnamed_value()?)),
            ItemNode::Path(node, label) => {
                let parent = node.generate_argument()?;
                match parent.unnamed_value()? {
                    ValueDefinition::Value(Value::Field(mut f)) => {
                        f.push(label.clone());
                        ValueDefinition::Value(Value::Field(f))
                    }
                    value => ValueDefinition::Path(Box::new(value), label.clone())
                }
            },
            ItemNode::Field(f) => ValueDefinition::Value(Value::Field(vec![f[1..].to_string().into_boxed_str()])),
            ItemNode::Substitution(s) =>
                    ValueDefinition::JobDefinition(s.generate()?),
            ItemNode::Closure(c) =>
                    ValueDefinition::ClosureDefinition(c.generate()?),
        }))
    }
}
