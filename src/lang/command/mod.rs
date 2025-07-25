mod closure;

use crate::lang::any_str::AnyStr;
use crate::lang::argument::ArgumentDefinition;
use crate::lang::ast::source::Source;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::completion::Completion;
use crate::lang::completion::parse::PartialCommandResult;
use crate::lang::errors::{CrushResult, CrushResultExtra, error};
use crate::lang::help::Help;
use crate::lang::job::Job;
use crate::lang::serialization::model;
use crate::lang::serialization::model::{Element, element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::state::contexts::{CommandContext, EvalContext};
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueDefinition, ValueType};
use closure::Closure;
use ordered_map::OrderedMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

pub type Command = Arc<dyn CrushCommand + Send + Sync>;

#[derive(Clone, Debug)]
pub enum OutputType {
    Unknown,
    Known(ValueType),
    Passthrough,
}

pub trait CommandBinder {
    fn bind(&self, value: Value) -> Command;
}

impl CommandBinder for Command {
    fn bind(&self, value: Value) -> Command {
        self.bind_helper(self, value)
    }
}

impl OutputType {
    fn calculate<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType> {
        match self {
            OutputType::Unknown => None,
            OutputType::Known(t) => Some(t),
            OutputType::Passthrough => input.calculate(&OutputType::Unknown),
        }
    }

    fn format(&self) -> Option<String> {
        match self {
            OutputType::Unknown => None,
            OutputType::Known(t) => Some(format!("# Output\n    {}", t)),
            OutputType::Passthrough => {
                Some("# Output\nA stream with the same columns as the input".to_string())
            }
        }
    }
}

pub trait CrushCommand: Help + Display {
    /// Execute this command with the specified context
    fn eval(&self, context: CommandContext) -> CrushResult<()>;
    /// True if there is a chance that invoking this command will block the thread
    fn might_block(&self, arguments: &[ArgumentDefinition], context: &mut EvalContext) -> bool;
    /// The name of this command
    fn name(&self) -> &str;
    /// Write this completion into pup format
    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize>;
    /// A helper method that binds this command to be a method of the specified object
    fn bind_helper(&self, wrapped: &Command, this: Value) -> Command;
    /// The return type of this command, if known
    fn output_type<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType>;
    /// Information about the parameters that can be passed to this command, which is useful for providing completions
    fn completion_data(&self) -> &[Parameter];
    fn definition(&self) -> Option<String>;
}

pub trait TypeMap {
    fn declare(
        &mut self,
        path: Vec<&str>,
        call: fn(context: CommandContext) -> CrushResult<()>,
        can_block: bool,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
        output: OutputType,
        completion_data: impl Into<Vec<Parameter>>,
    );
}

impl TypeMap for OrderedMap<String, Command> {
    fn declare(
        &mut self,
        path: Vec<&str>,
        call: fn(CommandContext) -> CrushResult<()>,
        can_block: bool,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
        output: OutputType,
        completion_data: impl Into<Vec<Parameter>>,
    ) {
        self.insert(
            path[path.len() - 1].to_string(),
            <dyn CrushCommand>::command(
                call,
                can_block,
                &path,
                signature,
                short_help,
                long_help,
                output,
                completion_data,
            ),
        );
    }
}

struct SimpleCommand {
    call: fn(context: CommandContext) -> CrushResult<()>,
    can_block: bool,
    full_name: Vec<String>,
    signature: AnyStr,
    short_help: AnyStr,
    long_help: Option<AnyStr>,
    output: OutputType,
    completion_data: Vec<Parameter>,
}

/**
A command that can block iff any of its arguments can block, e.g. `and` or `or`.
*/
struct ConditionCommand {
    call: fn(context: CommandContext) -> CrushResult<()>,
    full_name: Vec<String>,
    signature: &'static str,
    short_help: &'static str,
    long_help: Option<&'static str>,
    completion_data: Vec<Parameter>,
}

impl dyn CrushCommand {
    pub fn closure_command(
        name: Option<Source>,
        signature: Vec<ParameterDefinition>,
        job_definitions: Vec<Job>,
        env: &Scope,
        state: &GlobalState,
        source: Source,
    ) -> CrushResult<Command> {
        Ok(Arc::from(Closure::command(
            name,
            signature,
            job_definitions,
            env,
            state,
            source,
        )?))
    }

    pub fn closure_block(job_definitions: Vec<Job>, env: &Scope, source: Source) -> Command {
        Arc::from(Closure::block(job_definitions, env, source))
    }

    pub fn command(
        call: fn(context: CommandContext) -> CrushResult<()>,
        can_block: bool,
        full_name: impl IntoIterator<Item = impl AsRef<str>>,
        signature: impl Into<AnyStr>,
        short_help: impl Into<AnyStr>,
        long_help: Option<impl Into<AnyStr>>,
        output: OutputType,
        completion_data: impl Into<Vec<Parameter>>,
    ) -> Command {
        Arc::from(SimpleCommand {
            call,
            can_block,
            full_name: full_name
                .into_iter()
                .map(|a| a.as_ref().to_string())
                .collect(),
            signature: signature.into(),
            short_help: short_help.into(),
            long_help: long_help.map(|h| h.into()),
            output,
            completion_data: completion_data.into(),
        })
    }

    pub fn condition(
        call: fn(context: CommandContext) -> CrushResult<()>,
        full_name: Vec<String>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
        completion_data: Vec<Parameter>,
    ) -> Command {
        Arc::from(ConditionCommand {
            call,
            full_name,
            signature,
            short_help,
            long_help,
            completion_data,
        })
    }

    pub fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Command> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::Command(_) => {
                let strings = Vec::deserialize(id, elements, state)?;

                let val = state
                    .env
                    .get_absolute_path(strings.iter().map(|e| e.clone()).collect())?;
                match val {
                    Value::Command(c) => Ok(c),
                    _ => error("Expected a command"),
                }
            }
            element::Element::BoundCommand(bound_command) => {
                let this = Value::deserialize(bound_command.this as usize, elements, state)?;
                let command = <dyn CrushCommand>::deserialize(
                    bound_command.command as usize,
                    elements,
                    state,
                )?;
                Ok(command.bind(this))
            }
            element::Element::Closure(_) => Closure::deserialize(id, elements, state),
            _ => error("Expected a command"),
        }
    }
}

impl Display for SimpleCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

struct Foo<'a> {
    path: &'a Vec<String>,
}

impl Into<String> for Foo<'_> {
    fn into(self) -> String {
        self.path.join(":")
    }
}

impl CrushCommand for SimpleCommand {
    fn eval(&self, context: CommandContext) -> CrushResult<()> {
        let c = self.call;
        let source = context.source.clone();
        c(context)
            .with_command(Foo {
                path: &self.full_name,
            })
            .with_source_fallback(&source)
    }

    fn might_block(&self, _arg: &[ArgumentDefinition], _context: &mut EvalContext) -> bool {
        self.can_block
    }

    fn name(&self) -> &str {
        &self.full_name[self.full_name.len() - 1]
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let strings_idx = self.full_name.serialize(elements, state)?;
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Command(strings_idx as u64)),
        });
        Ok(idx)
    }

    fn bind_helper(&self, wrapped: &Command, this: Value) -> Command {
        Arc::from(BoundCommand {
            command: wrapped.clone(),
            this,
        })
    }

    fn output_type<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType> {
        self.output.calculate(input)
    }

    fn completion_data(&self) -> &[Parameter] {
        &self.completion_data
    }

    fn definition(&self) -> Option<String> {
        None
    }
}

impl Help for SimpleCommand {
    fn signature(&self) -> String {
        self.signature.to_string()
    }

    fn short_help(&self) -> String {
        self.short_help.to_string()
    }

    fn long_help(&self) -> Option<String> {
        let output = self.output.format();
        let long_cat = self.long_help.as_ref().map(|s| s.to_string());
        match (output, long_cat) {
            (Some(o), Some(l)) => Some(format!("{}\n\n{}", o, l)),
            (Some(o), None) => Some(o),
            (None, Some(o)) => Some(o),
            (None, None) => None,
        }
    }
}

impl std::cmp::PartialEq for SimpleCommand {
    fn eq(&self, _other: &SimpleCommand) -> bool {
        false
    }
}

impl Eq for SimpleCommand {}

impl std::fmt::Debug for SimpleCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command")
    }
}

impl Display for ConditionCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl CrushCommand for ConditionCommand {
    fn eval(&self, context: CommandContext) -> CrushResult<()> {
        let c = self.call;
        let source = context.source.clone();
        c(context)
            .with_command(Foo {
                path: &self.full_name,
            })
            .with_source_fallback(&source)
    }

    fn name(&self) -> &str {
        "conditional command"
    }

    fn might_block(&self, arguments: &[ArgumentDefinition], context: &mut EvalContext) -> bool {
        arguments.iter().any(|arg| arg.value.can_block(context))
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let strings_idx = self.full_name.serialize(elements, state)?;
        elements.push(Element {
            element: Some(element::Element::Command(strings_idx as u64)),
        });
        Ok(elements.len() - 1)
    }

    fn bind_helper(&self, wrapped: &Command, this: Value) -> Command {
        Arc::from(BoundCommand {
            command: wrapped.clone(),
            this,
        })
    }

    fn output_type(&self, _input: &OutputType) -> Option<&ValueType> {
        None
    }

    fn completion_data(&self) -> &[Parameter] {
        &self.completion_data
    }

    fn definition(&self) -> Option<String> {
        None
    }
}

impl Help for ConditionCommand {
    fn signature(&self) -> String {
        self.signature.to_string()
    }

    fn short_help(&self) -> String {
        self.short_help.to_string()
    }

    fn long_help(&self) -> Option<String> {
        self.long_help.map(|s| s.to_string())
    }
}

impl PartialEq for ConditionCommand {
    fn eq(&self, _other: &ConditionCommand) -> bool {
        false
    }
}

impl Eq for ConditionCommand {}

#[derive(Clone)]
pub struct Parameter {
    pub name: String,
    pub value_type: ValueType,
    pub default: Option<Value>,
    pub allowed: Option<Vec<Value>>,
    pub description: Option<String>,
    pub complete: Option<
        fn(
            cmd: &PartialCommandResult,
            cursor: usize,
            scope: &Scope,
            res: &mut Vec<Completion>,
        ) -> CrushResult<()>,
    >,
    pub named: bool,
    pub unnamed: bool,
}

impl Display for Parameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (self.named, self.unnamed) {
            (false, false) => {
                f.write_str("$")?;
                self.name.fmt(f)?;
                if self.value_type != ValueType::Any {
                    if self.value_type.is_parametrized() {
                        f.write_str(": $(")?;
                        self.value_type.fmt(f)?;
                        f.write_str(")")?;
                    } else {
                        f.write_str(": $")?;
                        self.value_type.fmt(f)?;
                    }
                }
                if let Some(default) = &self.default {
                    f.write_str(" = ")?;
                    default.fmt(f)?;
                }
                Ok(())
            }
            (true, false) => {
                f.write_str("@@")?;
                self.name.fmt(f)?;
                Ok(())
            }
            (false, true) => {
                f.write_str("@")?;
                self.name.fmt(f)?;
                Ok(())
            }
            (true, true) => Ok(()), // This is an error, but per the API docs, formatting should be considered an infallible operation, so we do nothing.
        }
    }
}

#[derive(Clone)]
pub enum ParameterDefinition {
    Normal(
        TrackedString,
        ValueDefinition,
        Option<ValueDefinition>,
        Option<TrackedString>,
    ),
    Named {
        name: TrackedString,
        description: Option<TrackedString>,
    },
    Unnamed {
        name: TrackedString,
        description: Option<TrackedString>,
    },
    Meta(TrackedString, TrackedString),
}

impl Display for ParameterDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterDefinition::Normal(name, value_type, default, _doc) => {
                f.write_str("$")?;
                name.fmt(f)?;
                f.write_str(": $")?;
                value_type.fmt(f)?;
                if let Some(default) = default {
                    f.write_str(" = ")?;
                    default.fmt(f)?;
                }
                Ok(())
            }
            ParameterDefinition::Named { name, .. } => {
                f.write_str("@@")?;
                name.fmt(f)?;
                Ok(())
            }
            ParameterDefinition::Unnamed { name, .. } => {
                f.write_str("@")?;
                name.fmt(f)?;
                Ok(())
            }
            ParameterDefinition::Meta(_key, _value) => Ok(()),
        }
    }
}

pub struct BoundCommand {
    command: Command,
    this: Value,
}

impl Display for BoundCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl CrushCommand for BoundCommand {
    fn eval(&self, mut context: CommandContext) -> CrushResult<()> {
        context.this = Some(self.this.clone());
        let source = context.source.clone();
        self.command
            .eval(context)
            .with_command(self.name())
            .with_source_fallback(&source)
    }

    fn might_block(&self, arguments: &[ArgumentDefinition], context: &mut EvalContext) -> bool {
        self.command.might_block(arguments, context)
    }

    fn name(&self) -> &str {
        self.command.name()
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let this = self.this.serialize(elements, state)? as u64;
        let command = self.command.serialize(elements, state)? as u64;
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::BoundCommand(model::BoundCommand {
                this,
                command,
            })),
        });
        Ok(idx)
    }

    fn bind_helper(&self, _: &Command, this: Value) -> Command {
        Arc::from(BoundCommand {
            command: self.command.clone(),
            this: this.clone(),
        })
    }

    fn output_type<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType> {
        self.command.output_type(input)
    }

    fn completion_data(&self) -> &[Parameter] {
        self.command.completion_data()
    }

    fn definition(&self) -> Option<String> {
        self.command.definition()
    }
}

impl Help for BoundCommand {
    fn signature(&self) -> String {
        self.command.signature()
    }

    fn short_help(&self) -> String {
        self.command.short_help()
    }

    fn long_help(&self) -> Option<String> {
        self.command.long_help()
    }
}
