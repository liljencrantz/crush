/*
job_list := | non_empty_job_list

non_empty_job_list := non_empty_job_list Separator job | job

job := expression | job Pipe expression

expression := assignment_expression | expression assignment_expression | '[' job_list ']' | '(' job ')'

assignment_expression := label assignment_op assignment_expression | expression1 '[' job ']' '=' job | logical_expression;

logical_expression := logical_expression logical_op comparison_expression | comparsion_expression

comparison_expression := comparison_expression comparison_op term | term

term := term add_op factor | factor

factor := factor mul_op unary_expression

unary_expression := unary_op item | item

item := text | label | item [ job ] | item '/' label | integer | glob | float

*/


use crate::lang::job::Job;
use crate::lang::errors::{CrushResult, error, argument_error};
use crate::lang::call_definition::CallDefinition;
use crate::lang::argument::ArgumentDefinition;
use crate::lang::value::{ValueDefinition, Value};
use std::ops::Deref;

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
    pub expressions: Vec<ExpressionNode>,
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
            Ok(CallDefinition::new(cmd.value, arguments))
        }
    }
}

#[derive(Debug)]
pub enum ExpressionNode {
    Assignment(AssignmentNode),
    //    ListLiteral(JobListNode),
    Substitution(JobNode),
    Closure(JobListNode),
}

impl ExpressionNode {
    pub fn generate_standalone(&self) -> CrushResult<Option<CallDefinition>> {
        match self {
            ExpressionNode::Assignment(a) => a.generate_standalone(),
            ExpressionNode::Substitution(_) => Ok(None),
            ExpressionNode::Closure(_) => Ok(None),
        }
    }

    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            ExpressionNode::Assignment(a) => {
                a.generate_argument()
            }
            ExpressionNode::Substitution(s) =>
                Ok(ArgumentDefinition::unnamed(
                    ValueDefinition::JobDefinition(
                        s.generate()?
                    )
                )),
            ExpressionNode::Closure(c) =>
                Ok(ArgumentDefinition::unnamed(
                    ValueDefinition::ClosureDefinition(
                        c.generate()?
                    )
                )),
        }
    }
}

#[derive(Debug)]
pub enum AssignmentNode {
    Assignment(ItemNode, Box<ExpressionNode>),
    Logical(LogicalNode),
}

impl AssignmentNode {
    pub fn generate_argument(&self) -> CrushResult<ArgumentDefinition> {
        match self {
            AssignmentNode::Assignment(target, value) => {
                match target {
                    ItemNode::Label(t) => Ok(ArgumentDefinition::named(t.deref(), value.generate_argument()?.value)),
                    ItemNode::Text(_) => error("Invalid left side in assignment"),
                    ItemNode::Integer(_) => error("Invalid left side in assignment"),
                    ItemNode::Get(_, _) => error("Invalid left side in assignment"),
                    ItemNode::Path(_, _) => error("Invalid left side in assignment"),
                }
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
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("var"))), Box::from("set")),
                            vec![ArgumentDefinition::named(t, value.generate_argument()?.value)])
                    )),
                    ItemNode::Text(_) => error("Invalid left side in assignment"),
                    ItemNode::Integer(_) => error("Invalid left side in assignment"),
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
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("cond"))), Box::from("and")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "||" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("cond"))), Box::from("or")),
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
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("comp"))), Box::from("lt")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "<=" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("comp"))), Box::from("lte")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    ">" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("comp"))), Box::from("gt")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    ">=" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("comp"))), Box::from("gte")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "==" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("comp"))), Box::from("eq")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "!=" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("comp"))), Box::from("neq")),
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
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("math"))), Box::from("add")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "-" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("math"))), Box::from("sub")),
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
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("math"))), Box::from("mul")),
                            vec![l.generate_argument()?, r.generate_argument()?])
                        ))
                    }
                    "//" => {
                        Ok(Some(CallDefinition::new(
                            ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("math"))), Box::from("div")),
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
                                ValueDefinition::Path(Box::new(ValueDefinition::Lookup(Box::from("comp"))), Box::from("not")),
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
    Text(Box<str>),
    Integer(i128),
    Get(Box<ItemNode>, Box<JobNode>),
    Path(Box<ItemNode>, Box<str>),
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
            ItemNode::Label(l) => ValueDefinition::Lookup(l.clone()),
            ItemNode::Text(t) => ValueDefinition::Value(Value::Text(unescape(t).into_boxed_str())),
            ItemNode::Integer(i) => ValueDefinition::Value(Value::Integer(i.clone())),
            ItemNode::Get(node, field) =>
                ValueDefinition::Get(
                    Box::new(node.generate_argument()?.value),
                    Box::new(ValueDefinition::JobDefinition(field.generate()?))),
            ItemNode::Path(node, label) => ValueDefinition::Path(Box::new(node.generate_argument()?.value), label.clone()),
        }))
    }
    /*
        pub fn path(&self) -> Option<Vec<Box<str>>> {
            match self {
                ItemNode::Label(l) => Some(vec![l.clone()]),
                ItemNode::Text(t) => None,
                ItemNode::Integer(i) => None,
                ItemNode::Get(node, field) => None,
                ItemNode::Path(node, label) => {
                    v = node.path()?;
                    v.push(label);
                    Some(v)
                },
            }
        }
        */
}
