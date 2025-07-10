use crate::lang::argument::{Argument, ArgumentDefinition, ArgumentType, SwitchStyle};
use crate::lang::ast::location::Location;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::command::{
    BoundCommand, Command, CrushCommand, OutputType, ParameterDefinition, Parameter,
};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::data::dict::Dict;
use crate::lang::data::list::List;
use crate::lang::errors::{CrushResult, argument_error, argument_error_legacy, error, serialization_error};
use crate::lang::help::Help;
use crate::lang::job::Job;
use crate::lang::pipe::{black_hole, empty_channel};
use crate::lang::serialization::model;
use crate::lang::serialization::model::{Element, element, normal_parameter_definition, Values, SignatureDefinition};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::state::contexts::{CommandContext, CompileContext, JobContext};
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::scope::{Scope, ScopeType};
use crate::lang::value::{Value, ValueDefinition, ValueType};
use crate::util::escape::unescape;
use itertools::Itertools;
use ordered_map::OrderedMap;
use std::fmt::Display;
use std::sync::Arc;

enum ClosureType {
    Block,
    Command {
        name: Option<String>,
        signature_data: Vec<Parameter>,
        signature_string: String,
        short_help: String,
        long_help: String,
    },
}

static BLOCK_SIGNATURE_DATA: Vec<Parameter> = vec![];

impl ClosureType {
    fn scope_type(&self) -> ScopeType {
        match self {
            ClosureType::Block => ScopeType::Block,
            ClosureType::Command { .. } => ScopeType::Command,
        }
    }

    fn completion_data(&self) -> &[Parameter] {
        match self {
            ClosureType::Block => &BLOCK_SIGNATURE_DATA,
            ClosureType::Command { signature_data, .. } => &signature_data,
        }
    }

    fn signature(&self) -> String {
        match self {
            ClosureType::Block => "block".to_string(),
            ClosureType::Command {
                signature_string, ..
            } => signature_string.clone(),
        }
    }

    fn short_help(&self) -> String {
        match self {
            ClosureType::Block => "A block of code".to_string(),
            ClosureType::Command { short_help, .. } => short_help.clone(),
        }
    }

    fn long_help(&self) -> Option<String> {
        match self {
            ClosureType::Block => None,
            ClosureType::Command { long_help, .. } => Some(long_help.clone()),
        }
    }

    fn name(&self) -> &str {
        match self {
            ClosureType::Block => "<block>",
            ClosureType::Command { name, .. } => name.as_ref().map(|n| n.as_str()).unwrap_or("<anonymous command>"),
        }
    }

    fn push_arguments_to_env(
        &self,
        mut arguments: Vec<Argument>,
        context: &mut CompileContext,
    ) -> CrushResult<()> {
        match self {
            ClosureType::Block => {
                let mut unnamed = vec![];
                for arg in arguments.drain(..) {
                    match arg.argument_type {
                        Some(name) => {
                            context.env.redeclare(name.as_ref(), arg.value)?;
                        }
                        None => {
                            unnamed.push(arg.value);
                        }
                    }
                }

                if !unnamed.is_empty() {
                    context
                        .env
                        .redeclare_reserved("__unnamed__", Value::List(List::new_without_type(unnamed)))?;
                }
                Ok(())
            }
            ClosureType::Command { signature_data, .. } => {
                let mut named = OrderedMap::new();
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
                for param in signature_data {
                    match (param.named, param.unnamed) {
                        (false, false) => {
                            let value: Value;

                            if named.contains_key(&param.name) {
                                value = named.remove(&param.name).unwrap();
                            } else if !unnamed.is_empty() {
                                value = unnamed.remove(0);
                            } else if let Some(default) = &param.default {
                                value = default.clone();
                            } else {
                                return argument_error_legacy(
                                    format!("Missing argument {}.", param.name),
                                );
                            };

                            if !param.value_type.is(&value) {
                                return argument_error_legacy(
                                    format!(
                                        "Wrong type {} for argument {}, expected {}",
                                        value.value_type(),
                                        param.name,
                                        param.value_type
                                    )
                                );
                            }
                            context.env.redeclare(&param.name, value)?;
                        }
                        (false, true) => {
                            if named_name.is_some() {
                                return argument_error_legacy(
                                    "Multiple named argument destinations specified",
                                );
                            }
                            named_name = Some(param.name[1..].to_string());
                        }
                        (true, false) => {
                            if unnamed_name.is_some() {
                                return argument_error_legacy(
                                    "Multiple unnamed argument destinations specified",
                                );
                            }
                            unnamed_name = Some(param.name[1..].to_string());
                        }
                        (true, true) => {
                            return argument_error_legacy(
                                "Same parameter can't be both the destination for named and unnamed arguments",
                            );
                        }
                    }
                }

                if let Some(unnamed_name) = unnamed_name {
                    context.env.redeclare(
                        &unnamed_name,
                        List::new(ValueType::Any, unnamed).into(),
                    )?;
                } else if !unnamed.is_empty() {
                    return argument_error_legacy(format!(
                        "Stray unnamed argument of type {}",
                        unnamed[0].value_type()
                    ));
                }

                if let Some(named_name) = named_name {
                    let d = Dict::new(ValueType::String, ValueType::Any)?;
                    for (k, v) in named {
                        d.insert(Value::from(k), v)?;
                    }
                    context
                        .env
                        .redeclare(&named_name, d.into())?;
                } else if !named.is_empty() {
                    return argument_error_legacy(format!(
                        "Unrecognized named arguments {}",
                        named.keys().join(", ")
                    ));
                }
                Ok(())
            }
        }
    }
}

pub struct Closure {
    jobs: Vec<Job>,
    parent_scope: Scope,
    closure_type: ClosureType,
}

impl Help for Closure {
    fn signature(&self) -> String {
        self.closure_type.signature()
    }

    fn short_help(&self) -> String {
        self.closure_type.short_help()
    }

    fn long_help(&self) -> Option<String> {
        self.closure_type.long_help()
    }
}

impl CrushCommand for Closure {
    fn eval(&self, context: CommandContext) -> CrushResult<()> {
        let job_definitions = self.jobs.clone();
        let parent_env = self.parent_scope.clone();

        let scope_type = self.closure_type.scope_type();

        let env = parent_env.create_child(&context.scope, scope_type);

        let mut cc =
            CompileContext::from(&context.clone().with_output(black_hole())).with_scope(&env);
        if let Some(this) = context.this {
            env.redeclare("this", this)?;
        }
        
        self.closure_type.push_arguments_to_env(context.arguments, &mut cc)?;
        
        if env.is_stopped() {
            return Ok(());
        }

        for (idx, job_definition) in job_definitions.iter().enumerate() {
            let first = idx == 0;
            let last = idx == job_definitions.len() - 1;
            let input = if first {
                context.input.clone()
            } else {
                empty_channel()
            };
            let output = if last {
                context.output.clone()
            } else {
                black_hole()
            };

            let job = job_definition.eval(JobContext::new(
                input,
                output,
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
        if scope_type == ScopeType::Command {
            context.output.empty()?;
        }
        Ok(())
    }

    fn might_block(&self, _arg: &[ArgumentDefinition], _context: &mut CompileContext) -> bool {
        true
    }

    fn name(&self) -> &str {
        self.closure_type.name()
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

    fn completion_data(&self) -> &[Parameter] {
        &self.closure_type.completion_data()
    }
}

fn compile_signature(
    signature: &Vec<ParameterDefinition>,
    env: &Scope,
    state: &GlobalState,
) -> CrushResult<Vec<Parameter>> {
    let mut result = Vec::new();
    let cc = CompileContext::new(env.clone(), state.clone());
    for p in signature {
        match p {
            ParameterDefinition::Normal(name, value_type, default, description) => {
                let default = match default {
                    None => None,
                    Some(definition) => Some(definition.eval(&mut CompileContext::new(env.clone(), state.clone()))?.1),
                };
                let value_type = match value_type.eval(&mut CompileContext::new(env.clone(), state.clone()))?.1 {
                    Value::Type(vt) => vt,
                    _ => return argument_error(format!("Invalid type for argument {}", &name.string), name.location),
                };

                result.push(Parameter {
                    name: name.string.clone(),
                    value_type,
                    allowed: None,
                    description: description.as_ref().map(|s| s.string.clone()),
                    complete: None,
                    named: false,
                    unnamed: false,
                    default,
                })
            }
            ParameterDefinition::Named{name, description } => {
                result.push(Parameter {
                    name: name.string.clone(),
                    value_type: ValueType::Any,
                    allowed: None,
                    description: description.as_ref().map(|s| s.string.clone()),
                    complete: None,
                    named: true,
                    unnamed: false,
                    default: None,
                })
            }
            ParameterDefinition::Unnamed{name, description} => {
                result.push(Parameter {
                    name: name.string.clone(),
                    value_type: ValueType::Any,
                    allowed: None,
                    description: description.as_ref().map(|s| s.string.clone()),
                    complete: None,
                    named: false,
                    unnamed: true,
                    default: None,
                })
            }
            ParameterDefinition::Meta(_, _) => {}
        }
    }
    Ok(result)
}

fn format_default(default: &Option<ValueDefinition>) -> String {
    match default {
        None => "".to_string(),
        Some(v) => format!("({}) ", v.to_string()),
    }
}

fn create_signature_string(name: &Option<String>, signature: &Vec<ParameterDefinition>) -> String {
    format!(
        "{} {}",
        name.as_deref().unwrap_or("<anonymous command>"),
        signature
            .iter()
            .filter(|p| match p {
                ParameterDefinition::Meta(_, _) => false,
                _ => true,
            })
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(" "))
}

fn create_short_help(signature: &Vec<ParameterDefinition>) -> String {
    signature
        .iter()
        .flat_map(|p| match p {
            ParameterDefinition::Meta(key, value) => {
                if key.string == "short_help" {
                    Some(unescape(&value.string).unwrap_or("<Invalid help string>".to_string()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

fn create_long_help(signature: &Vec<ParameterDefinition>) -> String {
    let mut long_help = Vec::new();
    let mut param_help = Vec::new();
    let mut example = Vec::new();

    for i in signature {
        match i {
            ParameterDefinition::Normal(name, _arg_type, default, doc) => {
                param_help.push(format!(
                    " * `{}` {}{}",
                    &name.string,
                    format_default(default),
                    doc.as_ref()
                        .map(|d| unescape(&d.string))
                        .unwrap_or(Ok("<Invalid help string>".to_string()))
                        .unwrap_or("".to_string())
                ));
            }
            ParameterDefinition::Meta(key, value) => match key.string.as_ref() {
                "long_help" => {
                    long_help.push(
                        unescape(&value.string).unwrap_or("<Invalid help string>".to_string()),
                    );
                }
                "example" => {
                    example.push(format!(
                        "    {}",
                        unescape(&value.string).unwrap_or("<Invalid encoding>".to_string())
                    ));
                }
                _ => {}
            },
            _ => {}
        }
    }

    if param_help.len() > 0 {
        long_help.push("".to_string());
        long_help.push("This command accepts the following arguments:".to_string());
        long_help.push("".to_string());
        long_help.append(&mut param_help);
    }

    if example.len() > 0 {
        long_help.push(format!("# Examples"));
        long_help.push("".to_string());
        long_help.append(&mut example);
    }

    long_help.join("\n")
}

impl Closure {
    pub fn command(
        name: Option<TrackedString>,
        signature: Vec<ParameterDefinition>,
        job_definitions: Vec<Job>,
        parent_scope: &Scope,
        state: &GlobalState,
    ) -> CrushResult<Closure> {
        let name = name.map(|n| n.string);
        Ok(Closure {
            jobs: job_definitions,
            parent_scope: parent_scope.clone(),
            closure_type: ClosureType::Command {
                signature_data: compile_signature(&signature, parent_scope, state)?,
                signature_string: create_signature_string(&name, &signature),
                short_help: create_short_help(&signature),
                long_help: create_long_help(&signature),
                name,
            },
        })
    }

    pub fn block(job_definitions: Vec<Job>, parent_scope: &Scope) -> Closure {
        Closure {
            jobs: job_definitions,
            parent_scope: parent_scope.clone(),
            closure_type: ClosureType::Block,
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
        f.write_str("{ ")?;
        match &self.closure_type {
            ClosureType::Command { signature_data, .. } => {
                f.write_str("| ")?;
                for param in signature_data {
                    param.fmt(f)?;
                    f.write_str(" ")?;
                }
                f.write_str("| ")?;
            }
            _ => {}
        }
        let mut first = true;
        for j in &self.jobs {
            if first {
                first = false;
            } else {
                f.write_str("; ")?;
            }
            j.fmt(f)?;
        }
        f.write_str("} ")?;
        Ok(())
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

        match &closure.closure_type { 
            ClosureType::Block => {
                serialized.closure_type = Some(model::closure::ClosureType::Block(true));
            }
            ClosureType::Command { signature_data, name, signature_string, short_help, long_help } => {
                let name = Some(match &name {
                    None => model::command_closure::Name::HasName(false),
                    Some(name) => {
                        model::command_closure::Name::NameValue(name.serialize(self.elements, self.state)? as u64)
                    }
                });
                
                serialized.closure_type = Some(model::closure::ClosureType::Command(
                    model::CommandClosure {
                        signature_data: vec![],
                        signature_string: signature_string.serialize(self.elements, self.state)? as u64,
                        short_help: short_help.serialize(self.elements, self.state)? as u64,
                        long_help: long_help.serialize(self.elements, self.state)? as u64,
                        name,
                    }
                ));
            }
        }

        for j in &closure.jobs {
            serialized.job_definitions.push(self.job(j)?)
        }


        serialized.env = closure.parent_scope.serialize(self.elements, self.state)? as u64;

        let idx = self.elements.len();
        self.elements.push(model::Element {
            element: Some(model::element::Element::Closure(serialized)),
        });
        Ok(idx)
    }

    fn parameter(
        &mut self,
        signature: &Parameter,
    ) -> CrushResult<model::Parameter> {
        Ok(model::Parameter{
            name: signature.name.serialize(self.elements, self.state)? as u64,
            value_type: signature.value_type.serialize(self.elements, self.state)? as u64,
            named: signature.named,
            unnamed: signature.unnamed,
            default: Some(match &signature.default {
                None => model::parameter::Default::HasDefault(false),
                Some(default) => model::parameter::Default::DefaultValue(default.serialize(self.elements, self.state)? as u64),
            }),
            allowed: Some(match &signature.allowed {
                None => model::parameter::Allowed::HasAllowed(false),
                Some(allowed) => model::parameter::Allowed::AllowedValues(self.values(allowed)?)
            }),
            description: None,
        })
    }
    
    fn values(&mut self, values: &Vec<Value>) -> CrushResult<Values> {
        let mut vv = vec![];
        for v in values {
            vv.push(v.serialize(self.elements, self.state)? as u64);
        }
        Ok(Values{
            value: vv,
        })
    }

    fn signature_definition(
        &mut self,
        signature: &Option<Vec<ParameterDefinition>>,
    ) -> CrushResult<Option<model::closure_definition::Signature>> {
        Ok(Some(if let Some(s) = signature {
            model::closure_definition::Signature::SignatureValue(self.signature2(s)?)
        } else {
            model::closure_definition::Signature::HasSignature(false)
        }))
    }

    fn signature2(&mut self, signature: &[ParameterDefinition]) -> CrushResult<SignatureDefinition> {
        Ok(model::SignatureDefinition {
            parameter: signature
                .iter()
                .map(|p| self.parameter_definition(p))
                .collect::<CrushResult<Vec<_>>>()?,
        })
    }

    fn parameter_definition(&mut self, param: &ParameterDefinition) -> CrushResult<model::ParameterDefinition> {
        Ok(model::ParameterDefinition {
            parameter: Some(match param {
                ParameterDefinition::Normal(n, t, d, doc) => {
                    model::parameter_definition::Parameter::Normal(model::NormalParameterDefinition {
                        name: n.serialize(self.elements, self.state)? as u64,

                        r#type: Some(self.value_definition(t)?),

                        default: Some(match d {
                            None => model::normal_parameter_definition::Default::HasDefault(false),
                            Some(dv) => model::normal_parameter_definition::Default::DefaultValue(
                                self.value_definition(dv)?,
                            ),
                        }),
                        doc: Some(match doc {
                            None => model::normal_parameter_definition::Doc::HasDoc(false),
                            Some(v) => model::normal_parameter_definition::Doc::DocValue(
                                v.serialize(self.elements, self.state)? as u64,
                            ),
                        }),
                    })
                }
                ParameterDefinition::Named{name, description } => model::parameter_definition::Parameter::Named(model::VarArgDefinition {
                    name: name.serialize(self.elements, self.state)? as u64,
                    doc: match description {
                        None => Some(model::var_arg_definition::Doc::HasDoc(false)),
                        Some(d) => Some(model::var_arg_definition::Doc::DocValue(
                            d.serialize(self.elements, self.state)? as u64,
                        )),
                    },
                }),
                ParameterDefinition::Unnamed{name, description } => model::parameter_definition::Parameter::Unnamed(model::VarArgDefinition {
                    name: name.serialize(self.elements, self.state)? as u64,
                    doc: match description {
                        None => Some(model::var_arg_definition::Doc::HasDoc(false)),
                        Some(d) => Some(model::var_arg_definition::Doc::DocValue(
                            d.serialize(self.elements, self.state)? as u64,
                        )),
                    },
                }),
                ParameterDefinition::Meta(key, value) => model::parameter_definition::Parameter::Meta(model::MetaDefinition {
                    key: key.serialize(self.elements, self.state)? as u64,
                    value: value.serialize(self.elements, self.state)? as u64,
                }),
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
            ArgumentType::Named(s) => model::argument_definition::ArgumentType::Some(
                s.serialize(self.elements, self.state)? as u64,
            ),
            ArgumentType::Unnamed => model::argument_definition::ArgumentType::None(false),
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
                ValueDefinition::Value(v, location) => {
                    model::value_definition::ValueDefinition::Value(model::Value {
                        value: v.serialize(self.elements, self.state)? as u64,
                        start: location.start as u64,
                        end: location.end as u64,
                    })
                }

                ValueDefinition::ClosureDefinition {
                    name,
                    signature,
                    jobs,
                    location,
                } => model::value_definition::ValueDefinition::ClosureDefinition(
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
                        signature: self.signature_definition(signature)?,
                        start: location.start as u64,
                        end: location.end as u64,
                    },
                ),

                ValueDefinition::JobDefinition(j) => {
                    model::value_definition::ValueDefinition::Job(self.job(j)?)
                }

                ValueDefinition::Identifier(l) => model::value_definition::ValueDefinition::Label(
                    l.serialize(self.elements, self.state)? as u64,
                ),

                ValueDefinition::GetAttr(parent, element) => {
                    model::value_definition::ValueDefinition::GetAttr(Box::from(model::Attr {
                        parent: Some(Box::from(self.value_definition(parent)?)),
                        element: element.serialize(self.elements, self.state)? as u64,
                    }))
                }
                ValueDefinition::JobListDefinition(jobs) => {
                    model::value_definition::ValueDefinition::JobList(model::JobList {
                        jobs: jobs
                            .iter()
                            .map(|j| self.job(j))
                            .collect::<CrushResult<Vec<_>>>()?,
                    })
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
                    jobs: s
                        .job_definitions
                        .iter()
                        .map(|j| self.job(j))
                        .collect::<CrushResult<Vec<_>>>()?,

                     closure_type : match &s.closure_type {
                         Some(model::closure::ClosureType::Block(_)) => 
                             ClosureType::Block,
                         Some(model::closure::ClosureType::Command(command_closure)) => {
                                 ClosureType::Command {
                                     name: match command_closure.name {
                                         None | Some(model::command_closure::Name::HasName(_)) => None,
                                         Some(model::command_closure::Name::NameValue(idx)) => Some(String::deserialize(
                                             idx as usize,
                                             self.elements,
                                             self.state,
                                         )?),
                                     },
                                     signature_data: self.signature(&command_closure.signature_data)?,
                                     signature_string: String::deserialize(command_closure.signature_string as usize, self.elements, self.state)?,
                                     short_help: String::deserialize(command_closure.short_help as usize, self.elements, self.state)?,
                                     long_help: String::deserialize(command_closure.long_help as usize, self.elements, self.state)?,
                                 }
                             }
                         None => return serialization_error("Invalid command signature"),
                     },        
                    parent_scope: env,

                }))
            }
            _ => error("Expected a closure"),
        }
    }

    fn signature(&mut self, signature: &Vec<model::Parameter>) -> CrushResult<Vec<Parameter>> {
        let mut res = vec![];
        for p in signature {
            res.push(self.parameter(p)?)
        }
        Ok(res)
    }

    fn parameter(&mut self, parameter: &model::Parameter) -> CrushResult<Parameter> {
        Ok(Parameter {
            name: String::deserialize(parameter.name as usize, self.elements, self.state)?,
            value_type: ValueType::deserialize(parameter.value_type as usize, self.elements, self.state)?,
            default: match &parameter.default {
                None => return serialization_error("Invalid default value for command parameter"),
                Some(model::parameter::Default::HasDefault(_)) => None,
                Some(model::parameter::Default::DefaultValue(value)) => Some(Value::deserialize(*value as usize, self.elements, self.state)?),
            },
            allowed: match &parameter.allowed {
                None => return serialization_error("Invalid allowed values for command parameter"),
                Some(model::parameter::Allowed::HasAllowed(_)) => None,
                Some(model::parameter::Allowed::AllowedValues(values)) => Some(self.values(values)?),
            },
            description: match &parameter.description {
                None => return serialization_error("Invalid description for command parameter"),
                Some(model::parameter::Description::HasDescription(_)) => None,
                Some(model::parameter::Description::DescriptionValue(value)) => Some(String::deserialize(*value as usize, self.elements, self.state)?),
            },
            complete: None,
            named: parameter.named,
            unnamed: parameter.unnamed,
        })
    }

    fn values(&mut self, values: &Values) -> CrushResult<Vec<Value>> {
        values.value.iter().map(|v| {Value::deserialize(*v as usize, self.elements, self.state)}).collect()
    }

    fn signature_definition(&mut self, signature: &model::SignatureDefinition) -> CrushResult<Vec<ParameterDefinition>> {
        Ok(
            signature
                .parameter
                .iter()
                .map(|p| self.parameter_definition(p))
                .collect::<CrushResult<Vec<_>>>()?,
        )
    }

    fn doc(&mut self, doc: &Option<model::var_arg_definition::Doc>) -> CrushResult<Option<TrackedString>> {
        match doc {
            None | Some(model::var_arg_definition::Doc::HasDoc(_)) => Ok(None),
            Some(model::var_arg_definition::Doc::DocValue(idx)) => Ok(Some(TrackedString::deserialize(
                *idx as usize,
                self.elements,
                self.state,
            )?)),
        }
    }

    fn parameter_definition(&mut self, parameter: &model::ParameterDefinition) -> CrushResult<ParameterDefinition> {
        match &parameter.parameter {
            None => error("Missing parameter"),
            Some(model::parameter_definition::Parameter::Normal(param)) => {
                Ok(ParameterDefinition::Normal(
                    TrackedString::deserialize(param.name as usize, self.elements, self.state)?,
                    self.value_definition(param.r#type.as_ref().ok_or("Invalid parameter")?)?,
                    match &param.default {
                        None | Some(model::normal_parameter_definition::Default::HasDefault(_)) => None,
                        Some(model::normal_parameter_definition::Default::DefaultValue(def)) => {
                            Some(self.value_definition(def)?)
                        }
                    },
                    match &param.doc {
                        None | Some(normal_parameter_definition::Doc::HasDoc(_)) => None,
                        Some(normal_parameter_definition::Doc::DocValue(id)) => Some(
                            TrackedString::deserialize(*id as usize, self.elements, self.state)?,
                        ),
                    },
                ))
            }
            Some(model::parameter_definition::Parameter::Named(param)) => Ok(ParameterDefinition::Named {
                name: TrackedString::deserialize(param.name as usize, self.elements, self.state)?,
                description: self.doc(&param.doc)?,
            }),
            Some(model::parameter_definition::Parameter::Unnamed(param)) => Ok(ParameterDefinition::Unnamed {
                name: TrackedString::deserialize(param.name as usize, self.elements, self.state)?,
                description: self.doc(&param.doc)?,
            }),
            Some(model::parameter_definition::Parameter::Meta(meta)) => Ok(ParameterDefinition::Meta(
                TrackedString::deserialize(meta.key as usize, self.elements, self.state)?,
                TrackedString::deserialize(meta.value as usize, self.elements, self.state)?,
            )),
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
            value: self.value_definition(s.value.as_ref().ok_or("Missing argument value")?)?,
            argument_type: match s.argument_type.as_ref().ok_or("Missing argument type")? {
                model::argument_definition::ArgumentType::Some(s) => ArgumentType::Named(
                    TrackedString::deserialize(*s as usize, self.elements, self.state)?,
                ),
                model::argument_definition::ArgumentType::None(_) => ArgumentType::Unnamed,
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
            match s
                .value_definition
                .as_ref()
                .ok_or("Invalid value definition")?
            {
                model::value_definition::ValueDefinition::Value(val) => ValueDefinition::Value(
                    Value::deserialize(val.value as usize, self.elements, self.state)?,
                    Location {
                        start: val.start as usize,
                        end: val.end as usize,
                    },
                ),
                model::value_definition::ValueDefinition::ClosureDefinition(c) => {
                    ValueDefinition::ClosureDefinition {
                        name: match c.name {
                            None | Some(model::closure_definition::Name::HasName(_)) => None,
                            Some(model::closure_definition::Name::NameValue(id)) => Some(
                                TrackedString::deserialize(id as usize, self.elements, self.state)?,
                            ),
                        },
                        signature: match &c.signature {
                            None | Some(model::closure_definition::Signature::HasSignature(_)) => {
                                None
                            }
                            Some(model::closure_definition::Signature::SignatureValue(sig)) => {
                                Some(self.signature_definition(sig)?)
                            }
                        },
                        jobs: c
                            .job_definitions
                            .iter()
                            .map(|j| self.job(j))
                            .collect::<CrushResult<Vec<_>>>()?,
                        location: Location::new(c.start as usize, c.end as usize),
                    }
                }
                model::value_definition::ValueDefinition::Job(j) => {
                    ValueDefinition::JobDefinition(Job::new(
                        j.commands
                            .iter()
                            .map(|c| self.command(c))
                            .collect::<CrushResult<Vec<_>>>()?,
                        Location::new(j.start as usize, j.end as usize),
                    ))
                }
                model::value_definition::ValueDefinition::JobList(jobs) => {
                    let mut res = Vec::new();
                    for j in &jobs.jobs {
                        res.push(Job::new(
                            j.commands
                                .iter()
                                .map(|c| self.command(c))
                                .collect::<CrushResult<Vec<_>>>()?,
                            Location::new(j.start as usize, j.end as usize),
                        ));
                    }
                    ValueDefinition::JobListDefinition(res)
                }

                model::value_definition::ValueDefinition::Label(s) => ValueDefinition::Identifier(
                    TrackedString::deserialize(*s as usize, self.elements, self.state)?,
                ),
                model::value_definition::ValueDefinition::GetAttr(a) => {
                    ValueDefinition::GetAttr(
                        Box::from(self.value_definition(
                            a.parent.as_ref().ok_or("Invalid value definition")?,
                        )?),
                        TrackedString::deserialize(a.element as usize, self.elements, self.state)?,
                    )
                }
            },
        )
    }

}
