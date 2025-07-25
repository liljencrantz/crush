use super::tracked_string::TrackedString;
use super::{Node, NodeContext};
use crate::CrushResult;
use crate::lang::command::ParameterDefinition;
use crate::lang::value::{Value, ValueDefinition, ValueType};

#[derive(Clone, Debug)]
pub enum ParameterNode {
    Parameter {
        name: TrackedString,
        parameter_type: Option<Box<Node>>,
        default: Option<Node>,
        documentation: Option<TrackedString>,
    },
    Named(TrackedString, Option<TrackedString>),
    Unnamed(TrackedString, Option<TrackedString>),
    Meta(TrackedString, TrackedString),
}

impl ParameterNode {
    pub fn parameter(
        is: impl Into<TrackedString>,
        parameter_type: Option<Box<Node>>,
        default: Option<Node>,
        doc: Option<impl Into<TrackedString>>,
    ) -> ParameterNode {
        let name = is.into().slice_to_end(1);
        ParameterNode::Parameter {
            name,
            parameter_type,
            default,
            documentation: doc.map(|t| t.into()),
        }
    }

    pub fn meta(key: impl Into<TrackedString>, value: impl Into<TrackedString>) -> ParameterNode {
        ParameterNode::Meta(key.into(), value.into())
    }

    pub fn unnamed(
        is: impl Into<TrackedString>,
        doc: Option<impl Into<TrackedString>>,
    ) -> ParameterNode {
        let name = is.into().slice_to_end(1);
        ParameterNode::Unnamed(name, doc.map(|t| t.into()))
    }

    pub fn named(
        is: impl Into<TrackedString>,
        doc: Option<impl Into<TrackedString>>,
    ) -> ParameterNode {
        let name = is.into().slice_to_end(1);
        ParameterNode::Named(name, doc.map(|t| t.into()))
    }

    pub fn generate(&self, ctx: &NodeContext) -> CrushResult<ParameterDefinition> {
        match self {
            ParameterNode::Parameter {
                name,
                parameter_type,
                default,
                documentation,
            } => Ok(ParameterDefinition::Normal(
                name.clone(),
                parameter_type
                    .as_ref()
                    .map(|t| t.compile_argument(ctx)?.unnamed_value())
                    .unwrap_or(Ok(ValueDefinition::Value(
                        Value::Type(ValueType::Any),
                        ctx.source.subtrackedstring(name),
                    )))?,
                default
                    .as_ref()
                    .map(|d| d.compile_argument(ctx))
                    .transpose()?
                    .map(|a| a.unnamed_value())
                    .transpose()?,
                documentation.clone(),
            )),
            ParameterNode::Named(s, doc) => Ok(ParameterDefinition::Named {
                name: s.clone(),
                description: doc.clone(),
            }),
            ParameterNode::Unnamed(s, doc) => Ok(ParameterDefinition::Unnamed {
                name: s.clone(),
                description: doc.clone(),
            }),
            ParameterNode::Meta(k, v) => Ok(ParameterDefinition::Meta(k.clone(), v.clone())),
        }
    }
}
