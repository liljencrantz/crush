use crate::CrushResult;
use crate::lang::ast::Node;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::command::Parameter;
use crate::lang::value::{Value, ValueDefinition, ValueType};
use crate::state::scope::Scope;

#[derive(Clone, Debug)]
pub enum ParameterNode {
    Parameter(TrackedString, Option<Box<Node>>, Option<Node>, Option<TrackedString>),
    Named(TrackedString, Option<TrackedString>),
    Unnamed(TrackedString, Option<TrackedString>),
    Meta(TrackedString, TrackedString),
}

impl ParameterNode {

    pub fn parameter(is: impl Into<TrackedString>, parameter_type: Option<Box<Node>>, default: Option<Node>, doc: Option<impl Into<TrackedString>>) -> ParameterNode {
        let s = is.into();
        if s.string.starts_with("$") {
            ParameterNode::Parameter(s.slice_to_end(1), parameter_type, default, doc.map(|t| t.into()))
        } else {
            ParameterNode::Parameter(s, parameter_type, default, doc.map(|t| t.into()))
        }
    }

    pub fn meta(key: impl Into<TrackedString>, value: impl Into<TrackedString>) -> ParameterNode {
        ParameterNode::Meta(key.into(), value.into())
    }

    pub fn unnamed(is: impl Into<TrackedString>, doc: Option<impl Into<TrackedString>>) -> ParameterNode {
        ParameterNode::Unnamed(is.into(), doc.map(|t| t.into()))
    }
    pub fn named(is: impl Into<TrackedString>, doc: Option<impl Into<TrackedString>>) -> ParameterNode {
        ParameterNode::Named(is.into(), doc.map(|t| t.into()))
    }
    pub fn generate(&self, env: &Scope) -> CrushResult<Parameter> {
        match self {
            ParameterNode::Parameter(name, value_type, default, doc) => Ok(Parameter::Parameter(
                name.clone(),
                value_type
                    .as_ref()
                    .map(|t| t.compile_argument(env)?.unnamed_value())
                    .unwrap_or(Ok(ValueDefinition::Value(Value::Type(ValueType::Any), name.location)))?,
                default
                    .as_ref()
                    .map(|d| d.compile_argument(env))
                    .transpose()?
                    .map(|a| a.unnamed_value())
                    .transpose()?,
                doc.clone(),
            )),
            ParameterNode::Named(s, doc) => Ok(Parameter::Named(s.clone(), doc.clone())),
            ParameterNode::Unnamed(s, doc) => Ok(Parameter::Unnamed(s.clone(), doc.clone())),
            ParameterNode::Meta(k, v) => Ok(Parameter::Meta(k.clone(), v.clone())),
        }
    }
}
