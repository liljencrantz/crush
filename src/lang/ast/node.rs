use crate::lang::argument::SwitchStyle;
use crate::lang::ast::{CommandNode, JobListNode, JobNode};
use crate::lang::ast::location::Location;
use crate::lang::ast::parameter_node::ParameterNode;
use crate::lang::ast::tracked_string::TrackedString;

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
    pub fn val(l: Location) -> Node {
        Node::GetAttr(
            Box::from(Node::GetAttr(
                Box::from(Node::Identifier(TrackedString::new("global", l))),
                TrackedString::new("io", l))),
            TrackedString::new("val", l))
    }

    pub fn expression_to_command(self) -> CommandNode {
        let l = self.location();
        match self {
            Node::Substitution(n) if n.commands.len() == 1 => {
                n.commands[0].clone()
            }
            _ => {
                CommandNode {
                    expressions: vec![Node::val(self.location()), self],
                    location: l,
                }
            }
        }
    }

    pub fn expression_to_job(self) -> JobNode {
        if let Node::Substitution(s) = self {
            s
        } else {
            let location = self.location();
            let expressions = vec![Node::val(location), self];
            JobNode {
                commands: vec![CommandNode { expressions, location }],
                location,
            }
        }
    }
}
