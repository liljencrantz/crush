use super::argument::ArgumentDefinition;
use super::command_invocation::CommandInvocation;
use super::errors::{CrushResult, error};
use super::job::Job;
use super::state::scope::Scope;
use super::value::ValueDefinition;
use crate::util::user_map::get_current_username;
use crate::util::user_map::get_user;
use location::Location;
use node::Node;
use tracked_string::TrackedString;

pub mod lexer;
pub mod location;
pub mod node;
pub mod parameter_node;
pub mod token;
pub mod tracked_string;

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

impl From<JobNode> for JobListNode {
    fn from(job: JobNode) -> JobListNode {
        JobListNode {
            location: job.location,
            jobs: vec![job],
        }
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
}

impl From<Node> for JobNode {
    fn from(node: Node) -> JobNode {
        let location = node.location();
        match node {
            Node::Substitution(mut s) if s.jobs.len() == 1 => s.jobs.remove(0),
            Node::Assignment { .. } => JobNode {
                commands: vec![CommandNode {
                    expressions: vec![node],
                    location,
                }],
                location,
            },
            _ => {
                let expressions = vec![Node::val(location), node];
                JobNode {
                    commands: vec![CommandNode {
                        expressions,
                        location,
                    }],
                    location,
                }
            }
        }
    }
}

fn operator_function(op: &[&str], op_location: Location, l: Box<Node>, r: Box<Node>) -> Box<Node> {
    let location = op_location.union(l.location()).union(r.location());
    let cmd = attr(op, op_location);
    Box::from(Node::Substitution(
        JobNode {
            commands: vec![CommandNode {
                expressions: vec![cmd, *l, *r],
                location: location,
            }],
            location: location,
        }
        .into(),
    ))
}

pub fn operator_method(op: &str, op_location: Location, l: Box<Node>, r: Box<Node>) -> Box<Node> {
    let location = op_location.union(l.location()).union(r.location());
    let cmd = Node::GetAttr(l, TrackedString::new(op, op_location));
    Box::from(Node::Substitution(
        JobNode {
            commands: vec![CommandNode {
                expressions: vec![cmd, *r],
                location: location,
            }],
            location: location,
        }
        .into(),
    ))
}

pub fn unary_operator_method(op: &str, op_location: Location, n: Box<Node>) -> Box<Node> {
    let location = op_location.union(n.location());
    let cmd = Node::GetAttr(n, TrackedString::new(op, op_location));
    Box::from(Node::Substitution(
        JobNode {
            commands: vec![CommandNode {
                expressions: vec![cmd],
                location: location,
            }],
            location: location,
        }
        .into(),
    ))
}

pub fn negate(n: Box<Node>) -> Box<Node> {
    let location = n.location();
    let cmd = attr(&["global", "comp", "not"], location);
    Box::from(Node::Substitution(
        JobNode {
            commands: vec![CommandNode {
                expressions: vec![cmd, *n],
                location,
            }],
            location,
        }
        .into(),
    ))
}

pub fn expr_operator(iop: impl Into<TrackedString>, l: Box<Node>, r: Box<Node>) -> Box<Node> {
    let op = iop.into();
    match op.string.as_str() {
        "<" => operator_function(&["global", "comp", "lt"], op.location, l, r),
        "<=" => operator_function(&["global", "comp", "lte"], op.location, l, r),
        ">" => operator_function(&["global", "comp", "gt"], op.location, l, r),
        ">=" => operator_function(&["global", "comp", "gte"], op.location, l, r),
        "==" => operator_function(&["global", "comp", "eq"], op.location, l, r),
        "!=" => operator_function(&["global", "comp", "neq"], op.location, l, r),

        "and" => operator_function(&["global", "cond", "and"], op.location, l, r),
        "or" => operator_function(&["global", "cond", "or"], op.location, l, r),

        // Note that these operators reverse the arguments because the method exists on the second argument!
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
    pub fn background_job(location: Location) -> CommandNode {
        CommandNode {
            location,
            expressions: vec![attr(&["global", "control", "bg"], location)],
        }
    }

    pub fn compile(&self, env: &Scope) -> CrushResult<CommandInvocation> {
        if let Some(c) = self.expressions[0].compile_as_special_command(env)? {
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
        ValueDefinition::ClosureDefinition {
            signature,
            jobs,
            location,
            ..
        } => ValueDefinition::ClosureDefinition {
            name: Some(name.clone()),
            signature,
            jobs,
            location,
        },
        _ => v,
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
    Ok(get_user(user)?
        .home
        .to_str()
        .ok_or("Bad home directory")?
        .to_string())
}

fn expand_user(s: &str) -> CrushResult<String> {
    if !s.starts_with('~') {
        Ok(s.to_string())
    } else {
        let parts: Vec<&str> = s[1..].splitn(2, '/').collect();
        let home = if parts[0].len() > 0 {
            home_as_string(parts[0])
        } else {
            home_as_string(&get_current_username()?)
        };
        if parts.len() == 1 {
            home
        } else {
            home.map(|home| format!("{}/{}", home, parts[1]))
        }
    }
}
