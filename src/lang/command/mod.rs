mod closure;

use crate::lang::any_str::AnyStr;
use crate::lang::argument::ArgumentDefinition;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::completion::Completion;
use crate::lang::completion::parse::PartialCommandResult;
use crate::lang::errors::{CrushResult, error};
use crate::lang::help::Help;
use crate::lang::job::Job;
use crate::lang::serialization::model;
use crate::lang::serialization::model::{Element, element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::state::contexts::{CommandContext, CompileContext};
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueDefinition, ValueType};
use closure::Closure;
use ordered_map::OrderedMap;
use std::fmt::{Display, Formatter, Write};
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

#[derive(Clone)]
pub struct ArgumentDescription {
    pub name: String,
    pub value_type: ValueType,
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

pub trait CrushCommand: Help + Display {
    fn eval(&self, context: CommandContext) -> CrushResult<()>;
    fn might_block(&self, arguments: &[ArgumentDefinition], context: &mut CompileContext) -> bool;
    fn name(&self) -> &str;
    fn help(&self) -> &dyn Help;
    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize>;
    fn bind_helper(&self, wrapped: &Command, this: Value) -> Command;
    fn output_type<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType>;
    fn arguments(&self) -> &[ArgumentDescription];
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
        arguments: impl Into<Vec<ArgumentDescription>>,
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
        arguments: impl Into<Vec<ArgumentDescription>>,
    ) {
        self.insert(
            path[path.len() - 1].to_string(),
            <dyn CrushCommand>::command(
                call, can_block, &path, signature, short_help, long_help, output, arguments,
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
    arguments: Vec<ArgumentDescription>,
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
    arguments: Vec<ArgumentDescription>,
}

impl dyn CrushCommand {
    pub fn closure(
        name: Option<TrackedString>,
        signature: Option<Vec<Parameter>>,
        job_definitions: Vec<Job>,
        env: &Scope,
        arguments: Vec<ArgumentDescription>,
    ) -> Command {
        Arc::from(Closure::new(
            name,
            signature,
            job_definitions,
            env.clone(),
            arguments,
        ))
    }

    pub fn command(
        call: fn(context: CommandContext) -> CrushResult<()>,
        can_block: bool,
        full_name: impl IntoIterator<Item = impl AsRef<str>>,
        signature: impl Into<AnyStr>,
        short_help: impl Into<AnyStr>,
        long_help: Option<impl Into<AnyStr>>,
        output: OutputType,
        arguments: impl Into<Vec<ArgumentDescription>>,
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
            arguments: arguments.into(),
        })
    }

    pub fn condition(
        call: fn(context: CommandContext) -> CrushResult<()>,
        full_name: Vec<String>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
        arguments: Vec<ArgumentDescription>,
    ) -> Command {
        Arc::from(ConditionCommand {
            call,
            full_name,
            signature,
            short_help,
            long_help,
            arguments,
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

impl CrushCommand for SimpleCommand {
    fn eval(&self, context: CommandContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn might_block(&self, _arg: &[ArgumentDefinition], _context: &mut CompileContext) -> bool {
        self.can_block
    }

    fn name(&self) -> &str {
        &self.full_name[self.full_name.len() - 1]
    }

    fn help(&self) -> &dyn Help {
        self
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

    fn arguments(&self) -> &[ArgumentDescription] {
        &self.arguments
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

impl std::cmp::Eq for SimpleCommand {}

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
        c(context)?;
        Ok(())
    }

    fn name(&self) -> &str {
        "conditional command"
    }

    fn might_block(&self, arguments: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        arguments.iter().any(|arg| arg.value.can_block(context))
    }

    fn help(&self) -> &dyn Help {
        self
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

    fn arguments(&self) -> &[ArgumentDescription] {
        &self.arguments
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
pub enum Parameter {
    Parameter(
        TrackedString,
        ValueDefinition,
        Option<ValueDefinition>,
        Option<TrackedString>,
    ),
    Named(TrackedString, Option<TrackedString>),
    Unnamed(TrackedString, Option<TrackedString>),
    Meta(TrackedString, TrackedString),
}

impl Display for Parameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Parameter::Parameter(name, value_type, default, _doc) => {
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
            Parameter::Named(n, _doc) => {
                f.write_str("@@")?;
                n.fmt(f)?;
                Ok(())
            }
            Parameter::Unnamed(n, _doc) => {
                f.write_str("@")?;
                n.fmt(f)?;
                Ok(())
            }
            Parameter::Meta(_key, _value) => Ok(()),
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
        self.command.eval(context)
    }

    fn might_block(&self, arguments: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        self.command.might_block(arguments, context)
    }

    fn name(&self) -> &str {
        self.command.name()
    }

    fn help(&self) -> &dyn Help {
        self.command.help()
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

    fn arguments(&self) -> &[ArgumentDescription] {
        self.command.arguments()
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
