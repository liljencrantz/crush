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

pub struct JobListNode {
    pub jobs: Vec<JobNode>,
}

impl JobListNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<Vec<Job>> {
        self.jobs.iter().map(|j| j.generate(env)).collect()
    }
}

pub struct JobNode {
    pub commands: Vec<CommandNode>,
}

impl JobNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<Job> {
        Ok(Job::new(
            self.commands
                .iter()
                .map(|c| c.generate(env))
                .collect::<CrushResult<Vec<CommandInvocation>>>()?,
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

pub enum Node {
    Assignment(Box<Node>, String, Box<Node>),
    LogicalOperation(Box<Node>, String, Box<Node>),
    Comparison(Box<Node>, String, Box<Node>),
    Term(Box<Node>, String, Box<Node>),
    Factor(Box<Node>, String, Box<Node>),
    Unary(String, Box<Node>),
    Glob(String),
    Label(String),
    Regex(String),
    Field(String),
    String(String),
    File(PathBuf),
    Integer(i128),
    Float(f64),
    GetItem(Box<Node>, Box<Node>),
    GetAttr(Box<Node>, String),
    Path(Box<Node>, String),
    Substitution(JobNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode),
}

fn propose_name(name: &str, v: ValueDefinition) -> ValueDefinition {
    match v {
        ValueDefinition::ClosureDefinition(_, p, j) =>
            ValueDefinition::ClosureDefinition(Some(name.to_string()), p, j),
        _ => v,
    }
}

impl Node {
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

            Node::LogicalOperation(_, _, _)
            | Node::Comparison(_, _, _)
            | Node::GetItem(_, _)
            | Node::Term(_, _, _)
            | Node::Factor(_, _, _) => ValueDefinition::JobDefinition(Job::new(vec![self
                .generate_standalone(env)?
                .unwrap()])),
            Node::Unary(op, r) => match op.deref() {
                "neg" | "not" | "typeof" => ValueDefinition::JobDefinition(Job::new(vec![self
                    .generate_standalone(env)?
                    .unwrap()])),
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
            Node::Regex(l) => ValueDefinition::Value(Value::Regex(
                l.clone(),
                to_crush_error(Regex::new(l.clone().as_ref()))?,
            )),
            Node::String(t) => ValueDefinition::Value(Value::string(unescape(t).as_str())),
            Node::Integer(i) => ValueDefinition::Value(Value::Integer(*i)),
            Node::Float(f) => ValueDefinition::Value(Value::Float(*f)),
            Node::GetAttr(node, label) => {
                let parent = node.generate_argument(env)?;
                match parent.unnamed_value()? {
                    ValueDefinition::Value(Value::Field(mut f)) => {
                        f.push(label.clone());
                        ValueDefinition::Value(Value::Field(f))
                    }
                    value => ValueDefinition::GetAttr(Box::new(value), label.clone()),
                }
            }
            Node::Path(node, label) => ValueDefinition::Path(
                Box::new(node.generate_argument(env)?.unnamed_value()?),
                label.clone(),
            ),
            Node::Field(f) => ValueDefinition::Value(Value::Field(vec![f[1..].to_string()])),
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
                ValueDefinition::ClosureDefinition(None, p, c.generate(env)?)
            }
            Node::Glob(g) => ValueDefinition::Value(Value::Glob(Glob::new(&g))),
            Node::File(f) => ValueDefinition::Value(Value::File(f.clone())),
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
                    vec![ArgumentDefinition::named(
                        t,
                        propose_name(&t, value.generate_argument(env)?.unnamed_value()?),
                    )],
                ),

                Node::GetItem(container, key) => container.method_invocation(
                    "__setitem__",
                    vec![
                        ArgumentDefinition::unnamed(key.generate_argument(env)?.unnamed_value()?),
                        ArgumentDefinition::unnamed(value.generate_argument(env)?.unnamed_value()?),
                    ],
                    env,
                ),

                Node::GetAttr(container, attr) => container.method_invocation(
                    "__setattr__",
                    vec![
                        ArgumentDefinition::unnamed(ValueDefinition::Value(Value::string(
                            &attr.to_string(),
                        ))),
                        ArgumentDefinition::unnamed(value.generate_argument(env)?.unnamed_value()?),
                    ],
                    env,
                ),

                _ => error("Invalid left side in assignment"),
            },
            ":=" => match target.as_ref() {
                Node::Label(t) => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "var", "let"])?,
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
                let cmd = env.global_static_cmd(match op.as_ref() {
                    "and" => vec!["global", "cond", "and"],
                    "or" => vec!["global", "cond", "or"],
                    _ => return error("Unknown operator"),
                })?;
                Node::function_invocation(
                    cmd,
                    vec![l.generate_argument(env)?, r.generate_argument(env)?],
                )
            }

            Node::Comparison(l, op, r) => {
                let cmd = env.global_static_cmd(match op.as_ref() {
                    "<" => vec!["global", "comp", "lt"],
                    "<=" => vec!["global", "comp", "lte"],
                    ">" => vec!["global", "comp", "gt"],
                    ">=" => vec!["global", "comp", "gte"],
                    "==" => vec!["global", "comp", "eq"],
                    "!=" => vec!["global", "comp", "neq"],
                    "=~" => {
                        return r.method_invocation("match", vec![l.generate_argument(env)?], env);
                    }
                    "!~" => {
                        return r.method_invocation(
                            "not_match",
                            vec![l.generate_argument(env)?],
                            env,
                        );
                    }
                    _ => return error("Unknown operator"),
                })?;
                Node::function_invocation(
                    cmd.copy(),
                    vec![l.generate_argument(env)?, r.generate_argument(env)?],
                )
            }

            Node::Term(l, op, r) => {
                let method = match op.as_ref() {
                    "+" => "__add__",
                    "-" => "__sub__",
                    _ => return error("Unknown operator"),
                };
                l.method_invocation(method, vec![r.generate_argument(env)?], env)
            }

            Node::Factor(l, op, r) => {
                let method = match op.as_ref() {
                    "*" => "__mul__",
                    "//" => "__div__",
                    _ => return error("Unknown operator"),
                };
                l.method_invocation(method, vec![r.generate_argument(env)?], env)
            }

            Node::GetItem(val, key) => {
                val.method_invocation("__getitem__", vec![key.generate_argument(env)?], env)
            }

            Node::Unary(op, r) => match op.deref() {
                "neg" => r.method_invocation("__neg__", vec![], env),
                "not" => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "comp", "not"])?,
                    vec![r.generate_argument(env)?],
                ),
                "typeof" => Node::function_invocation(
                    env.global_static_cmd(vec!["global", "types", "typeof"])?,
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
            | Node::Integer(_)
            | Node::Float(_)
            | Node::GetAttr(_, _)
            | Node::Path(_, _)
            | Node::Substitution(_)
            | Node::Closure(_, _)
            | Node::File(_) => Ok(None),
        }
    }

    fn function_invocation(
        function: Command,
        arguments: Vec<ArgumentDefinition>,
    ) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(CommandInvocation::new(
            ValueDefinition::Value(Value::Command(function)),
            arguments,
        )))
    }

    fn method_invocation(
        &self,
        name: &str,
        arguments: Vec<ArgumentDefinition>,
        env: &Scope,
    ) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(CommandInvocation::new(
            ValueDefinition::GetAttr(
                Box::from(self.generate_argument(env)?.unnamed_value()?),
                name.to_string(),
            ),
            arguments,
        )))
    }

    pub fn parse_label(s: &str) -> Box<Node> {
        if s.contains('%') || s.contains('?') {
            Box::from(Node::Glob(s.to_string()))
        } else if s.starts_with('~') {
            expand_user_path(s)
        } else if s.contains('/') {
            if s.starts_with('/') {
                Box::from(Node::File(PathBuf::from(s)))
            } else {
                let parts = s.split('/').collect::<Vec<&str>>();
                Box::from(path(&parts))
            }
        } else {
            Box::from(Node::Label(s.to_string()))
        }
    }
}

fn path(parts: &[&str]) -> Node {
    let mut res = Node::Label(parts[0].to_string());
    for part in &parts[1..] {
        res = Node::Path(Box::from(res), part.to_string());
    }
    res
}

fn attr(parts: &[&str]) -> Node {
    let mut res = Node::Label(parts[0].to_string());
    for part in &parts[1..] {
        res = Node::GetAttr(Box::from(res), part.to_string());
    }
    res
}

fn simple_substitution(cmd: Vec<Node>) -> Box<Node> {
    Box::from(
        Node::Substitution(
            JobNode {
                commands: vec![
                    CommandNode {
                        expressions: cmd
                    }
                ]
            }
        )
    )
}

fn expand_user(s: &str) -> Box<Node> {
    if s.len() == 1 {
        Box::from(
            Node::GetAttr(
                simple_substitution(
                    vec![
                        attr(&vec!["global", "user", "me"])
                    ]
                ),
                "home".to_string(),
            )
        )
    } else {
        Box::from(
            Node::GetAttr(
                simple_substitution(
                    vec![
                        attr(&vec!["global", "user", "find"]),
                        Node::String(format!("\"{}\"", &s[1..]))
                    ]
                ),
                "home".to_string(),
            )
        )
    }
}

fn expand_user_path(s: &str) -> Box<Node> {
    if s.contains('/') {
        let (user, path) = s.split_at(s.find('/').unwrap());
        Box::from(
            Node::Path(
                expand_user(user),
                path[1..].to_string(),
            )
        )
    } else {
        expand_user(s)
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
    Parameter(String, Option<Box<Node>>, Option<Node>),
    Named(String),
    Unnamed(String),
}

impl ParameterNode {
    pub fn generate(&self, env: &Scope) -> CrushResult<Parameter> {
        match self {
            ParameterNode::Parameter(name, value_type, default) => Ok(Parameter::Parameter(
                name.clone(),
                value_type
                    .as_ref()
                    .map(|t| t.generate_argument(env)?.unnamed_value())
                    .unwrap_or(Ok(ValueDefinition::Value(Value::Type(ValueType::Any))))?,
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

pub enum TokenNode {
    LogicalOperator(String),
    UnaryOperator(String),
    Colon(String),
    ComparisonOperator(String),
    FactorOperator(String),
    TermOperator(String),
    QuotedString(String),
    Label(String),
    Flag(String),
    Field(String),
    QuotedLabel(String),
    Regex(String),
    Separator(String),
    Integer(String),
    Float(String),
}
