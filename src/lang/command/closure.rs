use crate::lang::argument::{Argument, ArgumentDefinition, ArgumentType, SwitchStyle};
use crate::lang::command::{ArgumentDescription, BoundCommand, Command, CrushCommand, OutputType, Parameter};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::data::dict::Dict;
use crate::lang::errors::{argument_error, argument_error_legacy, CrushResult, error, mandate};
use crate::lang::state::contexts::{CommandContext, CompileContext, JobContext};
use crate::lang::help::Help;
use crate::lang::job::Job;
use crate::lang::data::list::List;
use crate::lang::state::scope::{Scope, ScopeType};
use crate::lang::serialization::model;
use crate::lang::serialization::model::closure::Name;
use crate::lang::serialization::model::{element, Element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::pipe::{black_hole, empty_channel};
use crate::lang::value::{Value, ValueDefinition, ValueType};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::ast::location::Location;

pub struct Closure {
    name: Option<TrackedString>,
    job_definitions: Vec<Job>,
    signature: Option<Vec<Parameter>>,
    env: Scope,
    short_help: String,
    long_help: String,
    arguments: Vec<ArgumentDescription>,
}

impl CrushCommand for Closure {
    fn eval(&self, context: CommandContext) -> CrushResult<()> {
        let job_definitions = self.job_definitions.clone();
        let parent_env = self.env.clone();

        let scope_type = match self.signature {
            None => ScopeType::Block,
            Some(_) => ScopeType::Closure,
        };

        let env = parent_env.create_child(&context.scope, scope_type);

        let mut cc = CompileContext::from(&context.clone().with_output(black_hole())).with_scope(&env);
        if let Some(this) = context.this {
            env.redeclare("this", this)?;
        }
        Closure::push_arguments_to_env(&self.signature, context.arguments, &mut cc)?;

        if env.is_stopped() {
            return Ok(());
        }

        for (idx, job_definition) in job_definitions.iter().enumerate() {
            let first = idx == 0;
            let input = if first {
                context.input.clone()
            } else {
                empty_channel()
            };

            let job = job_definition.eval(JobContext::new(
                input,
                black_hole(),
                env.clone(),
                context.global_state.clone(),
            ))?;
            let local_printer = context.global_state.printer().clone();
            let local_threads = context.global_state.threads().clone();
            job.map(|id| local_threads.join_one(id, &local_printer));

            if env.is_stopped() {
                return env.send_return_value(&context.output);
            }
        }
        context.output.empty()
    }

    fn might_block(&self, _arg: &[ArgumentDefinition], _context: &mut CompileContext) -> bool {
        true
    }

    fn name(&self) -> &str {
        "closure"
    }

    fn help(&self) -> &dyn Help {
        self
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        ClosureSerializer::new(elements, state).closure(self)
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

    fn arguments(&self) -> &Vec<ArgumentDescription> {
        &self.arguments
    }
}

struct ClosureSerializer<'a> {
    elements: &'a mut Vec<Element>,
    state: &'a mut SerializationState,
}

impl<'a> ClosureSerializer<'a> {
    fn new(
        elements: &'a mut Vec<Element>,
        state: &'a mut SerializationState,
    ) -> ClosureSerializer<'a> {
        ClosureSerializer { elements, state }
    }

    fn closure(&mut self, closure: &Closure) -> CrushResult<usize> {
        let mut serialized: model::Closure = model::Closure::default();
        serialized.name = Some(match &closure.name {
            None => model::closure::Name::HasName(false),
            Some(name) => model::closure::Name::NameValue(
                name.serialize(self.elements, self.state)? as u64,
            ),
        });

        serialized.short_help = closure.short_help.clone();
        serialized.long_help = closure.long_help.clone();

        for j in &closure.job_definitions {
            serialized.job_definitions.push(self.job(j)?)
        }

        serialized.signature = self.signature(&closure.signature)?;

        serialized.env = closure.env.serialize(self.elements, self.state)? as u64;

        let idx = self.elements.len();
        self.elements.push(model::Element {
            element: Some(model::element::Element::Closure(serialized)),
        });
        Ok(idx)
    }

    fn signature(
        &mut self,
        signature: &Option<Vec<Parameter>>,
    ) -> CrushResult<Option<model::closure::Signature>> {
        Ok(Some(if let Some(s) = signature {
            model::closure::Signature::SignatureValue(self.signature2(s)?)
        } else {
            model::closure::Signature::HasSignature(false)
        }))
    }

    fn signature_definition(
        &mut self,
        signature: &Option<Vec<Parameter>>,
    ) -> CrushResult<Option<model::closure_definition::Signature>> {
        Ok(Some(if let Some(s) = signature {
            model::closure_definition::Signature::SignatureValue(self.signature2(s)?)
        } else {
            model::closure_definition::Signature::HasSignature(false)
        }))
    }

    fn signature2(&mut self, signature: &[Parameter]) -> CrushResult<model::Signature> {
        Ok(model::Signature {
            parameter: signature
                .iter()
                .map(|p| self.parameter(p))
                .collect::<CrushResult<Vec<_>>>()?,
        })
    }

    fn parameter(&mut self, param: &Parameter) -> CrushResult<model::Parameter> {
        Ok(model::Parameter {
            parameter: Some(match param {
                Parameter::Named(n) =>
                    model::parameter::Parameter::Named(n.serialize(self.elements, self.state)? as u64),
                Parameter::Parameter(n, t, d) => {
                    model::parameter::Parameter::Normal(model::NormalParameter {
                        name: n.serialize(self.elements, self.state)? as u64,

                        r#type: Some(self.value_definition(t)?),

                        default: Some(match d {
                            None => model::normal_parameter::Default::HasDefault(false),
                            Some(dv) =>
                                model::normal_parameter::Default::DefaultValue(
                                    self.value_definition(dv)?,
                                ),
                        }),
                    })
                }
                Parameter::Unnamed(n) =>
                    model::parameter::Parameter::Unnamed(n.serialize(self.elements, self.state)? as u64),
            }),
        })
    }

    fn job(&mut self, job: &Job) -> CrushResult<model::Job> {
        let mut s: model::Job = model::Job::default();
        for c in job.commands() {
            s.commands.push(self.command(c)?);
        }
        Ok(s)
    }

    fn command(&mut self, cmd: &CommandInvocation) -> CrushResult<model::CommandInvocation> {
        let mut s: model::CommandInvocation = model::CommandInvocation::default();
        s.command = Some(self.value_definition(cmd.command())?);
        s.arguments = cmd
            .arguments()
            .iter()
            .map(|a| self.argument(a))
            .collect::<CrushResult<Vec<_>>>()?;
        Ok(s)
    }

    fn argument(&mut self, a: &ArgumentDefinition) -> CrushResult<model::ArgumentDefinition> {
        Ok(model::ArgumentDefinition {
            value: Some(self.value_definition(&a.value)?),
            argument_type: Some(self.argument_type(&a.argument_type)?),
            switch_style: a.switch_style.into(),
            start: a.location.start as u64,
            end: a.location.end as u64,
        })
    }

    fn argument_type(
        &mut self,
        a: &ArgumentType,
    ) -> CrushResult<model::argument_definition::ArgumentType> {
        Ok(match a {
            ArgumentType::Some(s) =>
                model::argument_definition::ArgumentType::Some(s.serialize(self.elements, self.state)? as u64),
            ArgumentType::None => model::argument_definition::ArgumentType::None(false),
            ArgumentType::ArgumentList => {
                model::argument_definition::ArgumentType::ArgumentList(false)
            }
            ArgumentType::ArgumentDict => {
                model::argument_definition::ArgumentType::ArgumentDict(false)
            }
        })
    }

    fn value_definition(&mut self, v: &ValueDefinition) -> CrushResult<model::ValueDefinition> {
        Ok(model::ValueDefinition {
            value_definition: Some(match v {
                ValueDefinition::Value(v, location) =>
                    model::value_definition::ValueDefinition::Value(
                        model::Value {
                            value: v.serialize(self.elements, self.state)? as u64,
                            start: location.start as u64,
                            end: location.end as u64,
                        },
                    ),

                ValueDefinition::ClosureDefinition(
                    name, parameters, jobs, location) => {
                    model::value_definition::ValueDefinition::ClosureDefinition(
                        model::ClosureDefinition {
                            job_definitions: jobs
                                .iter()
                                .map(|j| self.job(j))
                                .collect::<CrushResult<Vec<_>>>()?,
                            name: Some(match name {
                                None => model::closure_definition::Name::HasName(false),
                                Some(name) => model::closure_definition::Name::NameValue(
                                    name.serialize(self.elements, self.state)? as u64,
                                ),
                            }),
                            signature: self.signature_definition(parameters)?,
                            start: location.start as u64,
                            end: location.end as u64,
                        },
                    )
                }

                ValueDefinition::JobDefinition(j) => {
                    model::value_definition::ValueDefinition::Job(self.job(j)?)
                }

                ValueDefinition::Identifier(l) => {
                    model::value_definition::ValueDefinition::Label(
                        l.serialize(self.elements, self.state)? as u64)
                }

                ValueDefinition::GetAttr(parent, element) => {
                    model::value_definition::ValueDefinition::GetAttr(Box::from(model::Attr {
                        parent: Some(Box::from(self.value_definition(parent)?)),
                        element: element.serialize(self.elements, self.state)? as u64,
                    }))
                }
            }),
        })
    }
}

struct ClosureDeserializer<'a> {
    elements: &'a [Element],
    state: &'a mut DeserializationState,
}

impl<'a> ClosureDeserializer<'a> {
    fn new(
        elements: &'a [Element],
        state: &'a mut DeserializationState,
    ) -> ClosureDeserializer<'a> {
        ClosureDeserializer { elements, state }
    }

    pub fn closure(&mut self, id: usize) -> CrushResult<Command> {
        match self.elements[id].element.as_ref().unwrap() {
            element::Element::Closure(s) => {
                let env = Scope::deserialize(s.env as usize, self.elements, self.state)?;
                Ok(Arc::from(Closure {
                    name: match s.name {
                        None | Some(Name::HasName(_)) => None,
                        Some(Name::NameValue(idx)) => Some(TrackedString::deserialize(
                            idx as usize,
                            self.elements,
                            self.state,
                        )?),
                    },
                    job_definitions: s
                        .job_definitions
                        .iter()
                        .map(|j| self.job(j))
                        .collect::<CrushResult<Vec<_>>>()?,
                    signature: match &s.signature {
                        None | Some(model::closure::Signature::HasSignature(_)) => None,
                        Some(model::closure::Signature::SignatureValue(sig)) => {
                            self.signature(sig)?
                        }
                    },
                    env,
                    short_help: s.short_help.clone(),
                    long_help: s.long_help.clone(),
                    arguments: vec![],
                }))
            }
            _ => error("Expected a closure"),
        }
    }

    fn signature(&mut self, signature: &model::Signature) -> CrushResult<Option<Vec<Parameter>>> {
        Ok(Some(
            signature
                .parameter
                .iter()
                .map(|p| self.parameter(p))
                .collect::<CrushResult<Vec<_>>>()?,
        ))
    }

    fn parameter(&mut self, parameter: &model::Parameter) -> CrushResult<Parameter> {
        match &parameter.parameter {
            None => error("Missing parameter"),
            Some(model::parameter::Parameter::Normal(param)) => Ok(Parameter::Parameter(
                TrackedString::deserialize(param.name as usize, self.elements, self.state)?,
                self.value_definition(mandate(param.r#type.as_ref(), "Invalid parameter")?)?,
                match &param.default {
                    None | Some(model::normal_parameter::Default::HasDefault(_)) => None,
                    Some(model::normal_parameter::Default::DefaultValue(def)) => {
                        Some(self.value_definition(def)?)
                    }
                },
            )),
            Some(model::parameter::Parameter::Named(param)) => Ok(Parameter::Named(
                TrackedString::deserialize(*param as usize, self.elements, self.state)?)),
            Some(model::parameter::Parameter::Unnamed(param)) => {
                Ok(Parameter::Unnamed(
                    TrackedString::deserialize(*param as usize, self.elements, self.state)?
                ))
            }
        }
    }

    fn job(&mut self, s: &model::Job) -> CrushResult<Job> {
        Ok(Job::new(
            s.commands
                .iter()
                .map(|c| self.command(c))
                .collect::<CrushResult<Vec<_>>>()?,
            Location::new(s.start as usize, s.end as usize),
        ))
    }

    fn command(&mut self, s: &model::CommandInvocation) -> CrushResult<CommandInvocation> {
        if let Some(command) = &s.command {
            Ok(CommandInvocation::new(
                self.value_definition(command)?,
                s.arguments
                    .iter()
                    .map(|a| self.argument(a))
                    .collect::<CrushResult<Vec<_>>>()?,
            ))
        } else {
            error("Invalid job")
        }
    }

    fn argument(&mut self, s: &model::ArgumentDefinition) -> CrushResult<ArgumentDefinition> {
        Ok(ArgumentDefinition {
            value: self.value_definition(mandate(s.value.as_ref(), "Missing argument value")?)?,
            argument_type: match mandate(s.argument_type.as_ref(), "Missing argument type")? {
                model::argument_definition::ArgumentType::Some(s) => ArgumentType::Some(TrackedString::deserialize(*s as usize, self.elements, self.state)?),
                model::argument_definition::ArgumentType::None(_) => ArgumentType::None,
                model::argument_definition::ArgumentType::ArgumentList(_) => {
                    ArgumentType::ArgumentList
                }
                model::argument_definition::ArgumentType::ArgumentDict(_) => {
                    ArgumentType::ArgumentDict
                }
            },
            switch_style: SwitchStyle::try_from(s.switch_style)?,
            location: Location::new(s.start as usize, s.end as usize),
        })
    }

    fn value_definition(&mut self, s: &model::ValueDefinition) -> CrushResult<ValueDefinition> {
        Ok(
            match mandate(s.value_definition.as_ref(), "Invalid value definition")? {
                model::value_definition::ValueDefinition::Value(val) => ValueDefinition::Value(
                    Value::deserialize(val.value as usize, self.elements, self.state)?,
                    Location { start: val.start as usize, end: val.end as usize },
                ),
                model::value_definition::ValueDefinition::ClosureDefinition(c) => {
                    ValueDefinition::ClosureDefinition(
                        match c.name {
                            None | Some(model::closure_definition::Name::HasName(_)) => None,
                            Some(model::closure_definition::Name::NameValue(id)) =>
                                Some(TrackedString::deserialize(id as usize, self.elements, self.state)?),
                        },
                        match &c.signature {
                            None | Some(model::closure_definition::Signature::HasSignature(_)) =>
                                None,
                            Some(model::closure_definition::Signature::SignatureValue(sig)) =>
                                self.signature(sig)?,
                        },
                        c.job_definitions
                            .iter()
                            .map(|j| self.job(j))
                            .collect::<CrushResult<Vec<_>>>()?,
                        Location::new(c.start as usize, c.end as usize),
                    )
                }
                model::value_definition::ValueDefinition::Job(j) => {
                    ValueDefinition::JobDefinition(
                        Job::new(
                            j.commands
                                .iter()
                                .map(|c| self.command(c))
                                .collect::<CrushResult<Vec<_>>>()?,
                            Location::new(j.start as usize, j.end as usize),
                        ))
                }
                model::value_definition::ValueDefinition::Label(s) => {
                    ValueDefinition::Identifier(TrackedString::deserialize(*s as usize, self.elements, self.state)?)
                }
                model::value_definition::ValueDefinition::GetAttr(a) => ValueDefinition::GetAttr(
                    Box::from(self.value_definition(mandate(
                        a.parent.as_ref(),
                        "Invalid value definition",
                    )?)?),
                    TrackedString::deserialize(a.element as usize, self.elements, self.state)?,
                ),
            },
        )
    }
}

impl Help for Closure {
    fn signature(&self) -> String {
        format!(
            "{} {}",
            self.name.as_ref().map(|s| s.string.clone()).as_deref().unwrap_or("<unnamed>"),
            self.signature
                .as_ref()
                .map(|s| s
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(" "))
                .unwrap_or_else(|| "".to_string()),
        )
    }

    fn short_help(&self) -> String {
        self.short_help.clone()
    }

    fn long_help(&self) -> Option<String> {
        Some(self.long_help.clone())
    }
}

/** Extracts the help message from a closure definition */
fn extract_help(jobs: &mut Vec<Job>) -> String {
    if jobs.is_empty() {
        return "".to_string();
    }

    let j = &jobs[0];
    match j.extract_help_message() {
        Some(help) => {
            if jobs.len() > 1 {
                jobs.remove(0);
            }
            help
        }
        _ => "".to_string(),
    }
}

impl Closure {
    pub fn new(
        name: Option<TrackedString>,
        signature: Option<Vec<Parameter>>,
        mut job_definitions: Vec<Job>,
        env: Scope,
        arguments: Vec<ArgumentDescription>,
    ) -> Closure {
        let short_help = extract_help(&mut job_definitions);
        let long_help = extract_help(&mut job_definitions);

        Closure {
            name,
            job_definitions,
            signature,
            env,
            short_help,
            long_help,
            arguments,
        }
    }

    fn push_arguments_to_env_with_signature(
        signature: &Vec<Parameter>,
        mut arguments: Vec<Argument>,
        context: &mut CompileContext,
    ) -> CrushResult<()> {
        let mut named = HashMap::new();
        let mut unnamed = Vec::new();
        for arg in arguments.drain(..) {
            match arg.argument_type {
                Some(name) => {
                    named.insert(name.clone(), arg.value);
                }
                None => unnamed.push(arg.value),
            };
        }
        let mut unnamed_name = None;
        let mut named_name = None;

        for param in signature {
            match param {
                Parameter::Parameter(name, value_type, default) => {
                    if let Value::Type(value_type) = value_type.eval_and_bind(context)? {
                        if named.contains_key(&name.string) {
                            let value = named.remove(&name.string).unwrap();
                            if !value_type.is(&value) {
                                return argument_error(
                                    format!(
                                        "Wrong parameter type {}, expected {}",
                                        value.value_type(), value_type),
                                    name.location);
                            }
                            context.env.redeclare(&name.string, value)?;
                        } else if !unnamed.is_empty() {
                            context.env.redeclare(&name.string, unnamed.remove(0))?;
                        } else if let Some(default) = default {
                            let env = context.env.clone();
                            env.redeclare(&name.string, default.eval_and_bind(context)?)?;
                        } else {
                            return argument_error(
                                format!(
                                    "Missing variable {}. Options are {}!!!",
                                    name.string,
                                    named.keys().map(|a| { a.to_string() }).collect::<Vec<String>>().join(", ")),
                                name.location);
                        }
                    } else {
                        return argument_error_legacy("Not a type");
                    }
                }
                Parameter::Named(name) => {
                    if named_name.is_some() {
                        return argument_error_legacy("Multiple named argument maps specified");
                    }
                    named_name = Some(name);
                }
                Parameter::Unnamed(name) => {
                    if unnamed_name.is_some() {
                        return argument_error_legacy("Multiple named argument maps specified");
                    }
                    unnamed_name = Some(name);
                }
            }
        }

        if let Some(unnamed_name) = unnamed_name {
            context.env.redeclare(
                unnamed_name.string.as_ref(),
                List::new(ValueType::Any, unnamed).into(),
            )?;
        } else if !unnamed.is_empty() {
            return argument_error_legacy("No target for unnamed arguments");
        }

        if let Some(named_name) = named_name {
            let d = Dict::new(ValueType::String, ValueType::Any)?;
            for (k, v) in named {
                d.insert(Value::from(k), v)?;
            }
            context.env.redeclare(named_name.string.as_ref(), d.into())?;
        } else if !named.is_empty() {
            return argument_error_legacy("No target for extra named arguments");
        }
        Ok(())
    }

    fn push_arguments_to_env(
        signature: &Option<Vec<Parameter>>,
        mut arguments: Vec<Argument>,
        context: &mut CompileContext,
    ) -> CrushResult<()> {
        if let Some(signature) = signature {
            Self::push_arguments_to_env_with_signature(signature, arguments, context)
        } else {
            for arg in arguments.drain(..) {
                match arg.argument_type {
                    Some(name) => {
                        context.env.redeclare(name.as_ref(), arg.value)?;
                    }
                    None => {
                        return argument_error_legacy("No target for unnamed arguments");
                    }
                }
            }
            Ok(())
        }
    }

    pub fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Command> {
        ClosureDeserializer::new(elements, state).closure(id)
    }
}

impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for j in &self.job_definitions {
            if first {
                first = false;
            } else {
                f.write_str("; ")?;
            }
            j.fmt(f)?;
        }
        Ok(())
    }
}
