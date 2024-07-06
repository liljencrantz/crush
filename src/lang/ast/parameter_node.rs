use num_format::Locale::pa;
use crate::CrushResult;
use crate::lang::ast::Node;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::command::Parameter;
use crate::lang::value::{Value, ValueDefinition, ValueType};
use crate::state::scope::Scope;

#[derive(Clone, Debug)]
pub enum ParameterNode {
    Parameter(TrackedString, Option<Box<Node>>, Option<Node>),
    Named(TrackedString),
    Unnamed(TrackedString),
}

impl ParameterNode {

    pub fn parameter(is: impl Into<TrackedString>, parameter_type: Option<Box<Node>>, default: Option<Node>) -> ParameterNode {
        ParameterNode::Parameter(is.into().slice_to_end(1), parameter_type, default)
    }

    pub fn unnamed(is: impl Into<TrackedString>) -> ParameterNode {
        ParameterNode::Unnamed(is.into())
    }
    pub fn named(is: impl Into<TrackedString>) -> ParameterNode {
        ParameterNode::Named(is.into())
    }
    pub fn generate(&self, env: &Scope) -> CrushResult<Parameter> {
        match self {
            ParameterNode::Parameter(name, value_type, default) => Ok(Parameter::Parameter(
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
            )),
            ParameterNode::Named(s) => Ok(Parameter::Named(s.clone())),
            ParameterNode::Unnamed(s) => Ok(Parameter::Unnamed(s.clone())),
        }
    }
}
