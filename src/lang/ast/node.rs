use super::location::Location;
use super::node::TextLiteralStyle::{Quoted, Unquoted};
use super::parameter_node::ParameterNode;
use super::tracked_string::TrackedString;
use super::{CommandNode, JobListNode, JobNode, NodeContext, expand_user, propose_name};
use crate::lang::argument::{ArgumentDefinition, SwitchStyle};
use crate::lang::command::{Command, ParameterDefinition};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::{CrushResult, compile_error};
use crate::lang::job::Job;
use crate::lang::value::{Value, ValueDefinition};
use crate::util::escape::{unescape, unescape_file};
use crate::util::glob::Glob;
use regex::Regex;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Clone, Debug, Copy)]
pub enum TextLiteralStyle {
    Quoted,
    Unquoted,
}

/**
A type representing a node in the abstract syntax tree that is the output of parsing a Crush script.
 */
#[derive(Clone, Debug)]
pub enum Node {
    Assignment {
        target: Box<Node>,
        style: SwitchStyle,
        operation: String,
        value: Box<Node>,
    },
    Unary(TrackedString, Box<Node>),
    Glob(TrackedString),
    Identifier(TrackedString),
    Regex(TrackedString),
    String(TrackedString, TextLiteralStyle),
    File(TrackedString, TextLiteralStyle),
    Integer(TrackedString),
    Float(TrackedString),
    GetItem(Box<Node>, Box<Node>),
    GetAttr(Box<Node>, TrackedString),
    Substitution(JobListNode),
    Closure(Option<Vec<ParameterNode>>, JobListNode, Location),
}

impl Node {
    pub fn val(l: Location) -> Node {
        Node::GetAttr(
            Box::from(Node::GetAttr(Node::global(l), TrackedString::new("io", l))),
            TrackedString::new("val", l),
        )
    }

    pub fn list_literal(node: JobListNode) -> Box<Node> {
        let mut cmd = vec![Self::get_attr(
            &["global", "types", "list", "of"],
            node.location,
        )];
        for it in node.jobs {
            cmd.push(Node::Substitution(it.into()))
        }

        Box::from(Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: cmd,
                    location: node.location,
                }],
                location: node.location,
            }
            .into(),
        ))
    }

    fn id(s: &str, l: Location) -> Box<Node> {
        Box::from(Node::Identifier(TrackedString::new(s, l)))
    }

    fn global(l: Location) -> Box<Node> {
        Node::id("global", l)
    }

    pub fn prefix(&self, pos: usize) -> CrushResult<Node> {
        match self {
            Node::Identifier(s) => Ok(Node::Identifier(s.prefix(pos))),
            _ => Ok(self.clone()),
        }
    }

    pub fn location(&self) -> Location {
        use Node::*;

        match self {
            Glob(s)
            | Identifier(s)
            | String(s, _)
            | Integer(s)
            | Float(s)
            | Regex(s)
            | File(s, _) => s.location,

            Assignment { target, value, .. } => target.location().union(value.location()),

            Unary(s, a) => s.location.union(a.location()),

            GetItem(a, b) => a.location().union(b.location()),
            GetAttr(p, n) => p.location().union(n.location),
            Substitution(j) => j.location,
            Closure(_, _, l) => {
                // Fixme: Can't tab complete or error report on parameters because they're not currently tracked
                *l
            }
        }
    }

    pub fn compile_command(&self, ctx: &NodeContext) -> CrushResult<ArgumentDefinition> {
        self.compile(ctx, true)
    }

    pub fn compile_argument(&self, ctx: &NodeContext) -> CrushResult<ArgumentDefinition> {
        self.compile(ctx, false)
    }

    pub fn type_name(&self) -> &str {
        match self {
            Node::Assignment { .. } => "assignment",
            Node::Unary(_, _) => "unary operator",
            Node::Glob(_) => "glob",
            Node::Identifier(_) => "identifier",
            Node::Regex(_) => "regular expression literal",
            Node::String(_, _) => "string literal",
            Node::File(_, _) => "file literal",
            Node::Integer(_) => "integer literal",
            Node::Float(_) => "floating point number literal",
            Node::GetItem(_, _) => "subscript",
            Node::GetAttr(_, _) => "member access",
            Node::Substitution(_) => "command substitution",
            Node::Closure(_, _, _) => "closure",
        }
    }

    pub fn compile(&self, ctx: &NodeContext, is_command: bool) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition::unnamed(match self {
            Node::Assignment {
                target,
                style,
                operation,
                value,
            } => match operation.deref() {
                "=" => {
                    return match target.as_ref() {
                        Node::String(t, TextLiteralStyle::Unquoted) => {
                            Ok(ArgumentDefinition::named_with_style(
                                &ctx.source.subtrackedstring(t),
                                *style,
                                propose_name(&t, value.compile_argument(ctx)?.unnamed_value()?),
                            ))
                        }
                        _ => compile_error(
                            format!(
                                "Invalid left side in named argument. Expected `string literal`, got `{}`.",
                                target.type_name()
                            ),
                            &ctx.source.substring(target.location()),
                        ),
                    };
                }
                s => {
                    return compile_error(
                        format!(
                            "Invalid assignment operator, can't use the {} operator inside a parameter list.",
                            s
                        ),
                        &ctx.source.substring(target.location()),
                    );
                }
            },

            Node::GetItem(a, o) => ValueDefinition::JobDefinition(Job::new(
                vec![self.compile_as_special_command(ctx)?.unwrap()],
                ctx.source.substring(a.location().union(o.location())),
            )),

            Node::Unary(op, r) => match op.string.as_str() {
                "@" => {
                    return Ok(ArgumentDefinition::list(
                        r.compile_argument(ctx)?.unnamed_value()?,
                    ));
                }
                "@@" => {
                    return Ok(ArgumentDefinition::dict(
                        r.compile_argument(ctx)?.unnamed_value()?,
                    ));
                }
                v => {
                    return compile_error(
                        format!("Unknown operator {}", v),
                        &ctx.source.substring(op.location()),
                    );
                }
            },
            Node::Identifier(l) => ValueDefinition::Identifier(ctx.source.subtrackedstring(l)),
            Node::Regex(l) => ValueDefinition::Value(
                Value::Regex(l.string.clone(), Regex::new(&l.string.clone())?),
                ctx.source.subtrackedstring(l),
            ),
            Node::String(t, TextLiteralStyle::Quoted) => ValueDefinition::Value(
                Value::from(unescape(&t.string)?),
                ctx.source.subtrackedstring(t),
            ),
            Node::String(f, TextLiteralStyle::Unquoted) => {
                if is_command {
                    ValueDefinition::Identifier(ctx.source.subtrackedstring(f))
                } else {
                    ValueDefinition::Value(Value::from(f), ctx.source.subtrackedstring(f))
                }
            }
            Node::Integer(s) => ValueDefinition::Value(
                Value::Integer(s.string.replace("_", "").parse::<i128>()?),
                ctx.source.subtrackedstring(s),
            ),
            Node::Float(s) => ValueDefinition::Value(
                Value::Float(s.string.replace("_", "").parse::<f64>()?),
                ctx.source.subtrackedstring(s),
            ),
            Node::GetAttr(node, identifier) => ValueDefinition::GetAttr(
                Box::new(node.compile(ctx, is_command)?.unnamed_value()?),
                ctx.source.subtrackedstring(identifier),
            ),

            Node::Substitution(s) => ValueDefinition::JobListDefinition(s.compile(ctx)?),
            Node::Closure(signature, jobs, location) => {
                let param = signature.as_ref().map(|v| {
                    v.iter()
                        .map(|p| p.generate(ctx))
                        .collect::<CrushResult<Vec<ParameterDefinition>>>()
                });
                let p = match param {
                    None => None,
                    Some(Ok(p)) => Some(p),
                    Some(Err(e)) => return Err(e),
                };
                ValueDefinition::ClosureDefinition {
                    name: None,
                    signature: p,
                    jobs: jobs.compile(ctx)?,
                    source: ctx.source.substring(*location),
                }
            }
            Node::Glob(g) => ValueDefinition::Value(
                Value::Glob(Glob::new(&g.string)),
                ctx.source.subtrackedstring(g),
            ),
            Node::File(s, quote_style) => ValueDefinition::Value(
                Value::from(match quote_style {
                    Quoted => unescape_file(&s.string)?,
                    Unquoted => PathBuf::from(&expand_user(&s.string)?),
                }),
                ctx.source.subtrackedstring(s),
            ),
        }))
    }

    fn compile_standalone_assignment(
        target: &Box<Node>,
        op: &String,
        value: &Node,
        ctx: &NodeContext,
    ) -> CrushResult<Option<CommandInvocation>> {
        match op.deref() {
            "=" => match target.as_ref() {
                Node::Identifier(t) => Node::function_invocation(
                    ctx.env.global_static_cmd(vec!["global", "var", "set"])?,
                    t.location,
                    vec![ArgumentDefinition::named(
                        &ctx.source.subtrackedstring(t),
                        propose_name(&t, value.compile_argument(ctx)?.unnamed_value()?),
                    )],
                    ctx,
                ),

                Node::GetItem(container, key) => container.method_invocation(
                    &TrackedString::new("__setitem__", key.location()),
                    vec![
                        ArgumentDefinition::unnamed(key.compile_argument(ctx)?.unnamed_value()?),
                        ArgumentDefinition::unnamed(value.compile_argument(ctx)?.unnamed_value()?),
                    ],
                    ctx,
                    true,
                ),

                Node::GetAttr(container, attr) => container.method_invocation(
                    &TrackedString::new("__setattr__", attr.location),
                    vec![
                        ArgumentDefinition::unnamed(ValueDefinition::Value(
                            Value::from(attr),
                            ctx.source.subtrackedstring(attr),
                        )),
                        ArgumentDefinition::unnamed(propose_name(
                            attr,
                            value.compile_argument(ctx)?.unnamed_value()?,
                        )),
                    ],
                    ctx,
                    true,
                ),

                n => compile_error(
                    format!(
                        "Invalid left side in assignment. Expected `identifier`, got `{}`.  Try `$foo = 1`.",
                        n.type_name()
                    ),
                    &ctx.source.substring(n.location()),
                ),
            },
            ":=" => match target.as_ref() {
                Node::Identifier(t) => Node::function_invocation(
                    ctx.env.global_static_cmd(vec!["global", "var", "let"])?,
                    t.location,
                    vec![ArgumentDefinition::named(
                        &ctx.source.subtrackedstring(t),
                        propose_name(&t, value.compile_argument(ctx)?.unnamed_value()?),
                    )],
                    ctx,
                ),
                n => compile_error(
                    format!(
                        "Invalid left side in declaration. Expected `identifier`, got `{}`. Try `$foo := 1`",
                        n.type_name()
                    ),
                    &ctx.source.substring(n.location()),
                ),
            },
            s => compile_error(
                format!("Unknown assignment operator `{}`", s),
                &ctx.source.substring(target.location()),
            ),
        }
    }

    pub fn compile_as_special_command(
        &self,
        ctx: &NodeContext,
    ) -> CrushResult<Option<CommandInvocation>> {
        match self {
            Node::Assignment {
                target,
                operation,
                value,
                ..
            } => Node::compile_standalone_assignment(target, operation, value, ctx),

            Node::GetItem(val, key) => val.method_invocation(
                &TrackedString::new("__getitem__", key.location()),
                vec![key.compile_argument(ctx)?],
                ctx,
                true,
            ),

            Node::Unary(op, _) => match op.string.as_ref() {
                "@" | "@@" => Ok(None),
                _ => compile_error("Unknown operator", &ctx.source.substring(op.location())),
            },

            Node::Glob(_)
            | Node::Identifier(_)
            | Node::Regex(_)
            | Node::String(_, _)
            | Node::Integer(_)
            | Node::Float(_)
            | Node::GetAttr(_, _)
            | Node::Substitution(_)
            | Node::Closure(_, _, _)
            | Node::File(_, _) => Ok(None),
        }
    }

    fn function_invocation(
        function: Command,
        location: Location,
        arguments: Vec<ArgumentDefinition>,
        ctx: &NodeContext,
    ) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(CommandInvocation::new(
            ValueDefinition::Value(Value::from(function), ctx.source.substring(location)),
            ctx.source.substring(location),
            arguments,
        )))
    }

    fn method_invocation(
        &self,
        name: &TrackedString,
        arguments: Vec<ArgumentDefinition>,
        ctx: &NodeContext,
        as_command: bool,
    ) -> CrushResult<Option<CommandInvocation>> {
        Ok(Some(CommandInvocation::new(
            ValueDefinition::GetAttr(
                Box::from(self.compile(ctx, as_command)?.unnamed_value()?),
                ctx.source.subtrackedstring(name),
            ),
            ctx.source.clone(),
            arguments,
        )))
    }

    pub fn identifier(is: impl Into<TrackedString>) -> Box<Node> {
        let s = is.into();
        if s.string.starts_with("$") {
            Box::from(Node::Identifier(s.slice_to_end(1)))
        } else {
            Box::from(Node::Identifier(s))
        }
    }

    pub fn file(is: impl Into<TrackedString>, quoted: TextLiteralStyle) -> Box<Node> {
        Box::from(Node::File(is.into(), quoted))
    }

    pub fn quoted_string(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::String(is.into(), Quoted))
    }

    pub fn return_expr(location: Location) -> Box<Node> {
        Self::control_expr("return", location)
    }

    pub fn break_expr(location: Location) -> Box<Node> {
        Self::control_expr("break", location)
    }

    pub fn continue_expr(location: Location) -> Box<Node> {
        Self::control_expr("continue", location)
    }

    pub fn if_expr(
        if_location: Location,
        condition: Box<Node>,
        true_body: JobListNode,
        false_body: Option<JobListNode>,
    ) -> Box<Node> {
        let location = if_location.union(true_body.location);
        let true_location = true_body.location;
        let mut expressions = vec![
            Self::get_attr(&["global", "control", "if"], if_location),
            Node::Substitution(
                JobNode {
                    commands: vec![CommandNode {
                        expressions: vec![*condition],
                        location,
                    }],
                    location,
                }
                .into(),
            ),
            Node::Closure(None, true_body, true_location),
        ];

        if let Some(x) = false_body {
            let false_location = x.location;
            expressions.push(Node::String(
                TrackedString::new("else", x.location),
                Unquoted,
            ));
            expressions.push(Node::Closure(None, x, false_location));
        }

        Box::from(Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions,
                    location,
                }],
                location,
            }
            .into(),
        ))
    }

    pub fn while_expr(
        while_location: Location,
        condition: Box<Node>,
        body: JobListNode,
    ) -> Box<Node> {
        let location = while_location.union(body.location);
        Box::from(Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![
                        Self::get_attr(&["global", "control", "while"], while_location),
                        Node::Closure(
                            None,
                            JobListNode {
                                jobs: vec![JobNode {
                                    commands: vec![CommandNode {
                                        expressions: vec![*condition],
                                        location,
                                    }],
                                    location,
                                }],
                                location,
                            },
                            location,
                        ),
                        Node::Closure(None, body, location),
                    ],
                    location,
                }],
                location,
            }
            .into(),
        ))
    }

    pub fn loop_expr(loop_location: Location, body: JobListNode) -> Box<Node> {
        let location = loop_location.union(body.location);
        Box::from(Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![
                        Self::get_attr(&["global", "control", "loop"], loop_location),
                        Node::Closure(None, body, location),
                    ],
                    location,
                }],
                location,
            }
            .into(),
        ))
    }

    pub fn for_expr(
        for_location: Location,
        id: TrackedString,
        iter: Box<Node>,
        body: JobListNode,
    ) -> Box<Node> {
        let location = for_location.union(body.location);
        Box::from(Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![
                        Self::get_attr(&["global", "control", "for"], for_location),
                        Node::Assignment {
                            target: Box::from(Node::Identifier(id)),
                            style: SwitchStyle::None,
                            operation: "=".to_string(),
                            value: iter,
                        },
                        Node::Closure(None, body, location),
                    ],
                    location,
                }],
                location,
            }
            .into(),
        ))
    }

    fn get_attr(path: &[&str], location: Location) -> Node {
        if path.len() == 1 {
            Node::Identifier(TrackedString::from((path[0], location)))
        } else {
            Node::GetAttr(
                Box::from(Self::get_attr(&path[0..(path.len() - 1)], location)),
                TrackedString::from((path[path.len() - 1], location)),
            )
        }
    }

    fn control_expr(keyword: &str, location: Location) -> Box<Node> {
        Box::from(Node::Substitution(
            JobNode {
                commands: vec![CommandNode {
                    expressions: vec![Self::get_attr(&["global", "control", keyword], location)],
                    location,
                }],
                location,
            }
            .into(),
        ))
    }

    pub fn unquoted_string(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::String(is.into(), Unquoted))
    }

    pub fn glob(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Glob(is.into()))
    }

    pub fn integer(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Integer(is.into()))
    }

    pub fn float(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Float(is.into()))
    }

    pub fn regex(is: impl Into<TrackedString>) -> Box<Node> {
        Box::from(Node::Regex(is.into()))
    }
}

impl From<JobNode> for Box<Node> {
    fn from(mut list: JobNode) -> Box<Node> {
        if list.commands.len() == 1 {
            if list.commands[0].expressions.len() == 1 {
                return Box::from(list.commands[0].expressions.remove(0));
            }
        }
        Box::from(Node::Substitution(list.into()))
    }
}

impl From<Box<Node>> for JobNode {
    fn from(node: Box<Node>) -> JobNode {
        JobNode::from(CommandNode::from(*node))
    }
}

impl From<CommandNode> for JobNode {
    fn from(value: CommandNode) -> Self {
        JobNode {
            location: value.location,
            commands: vec![value],
        }
    }
}

impl From<Node> for CommandNode {
    fn from(value: Node) -> Self {
        let l = value.location();
        match value {
            Node::Substitution(n) if n.jobs.len() == 1 && n.jobs[0].commands.len() == 1 => {
                n.jobs[0].commands[0].clone()
            }
            Node::Assignment { .. } => CommandNode {
                expressions: vec![value],
                location: l,
            },
            _ => CommandNode {
                expressions: vec![Node::val(value.location()), value],
                location: l,
            },
        }
    }
}

impl From<Box<Node>> for JobListNode {
    fn from(node: Box<Node>) -> JobListNode {
        match *node {
            Node::Substitution(job) => job,
            _ => JobListNode::from(JobNode::from(node)),
        }
    }
}
