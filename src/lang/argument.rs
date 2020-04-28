use crate::lang::value::{Value};
use crate::lang::{value::ValueDefinition};
use crate::lang::errors::{CrushResult, error, argument_error};
use std::collections::HashSet;
use crate::lang::execution_context::CompileContext;
use crate::lang::printer::Printer;

#[derive(Debug, Clone)]
pub enum ArgumentType {
    Some(String),
    None,
    ArgumentList,
    ArgumentDict,
}

impl ArgumentType {
    pub fn is_some(&self) -> bool {
        matches!(self, ArgumentType::Some(_))
    }

    pub fn is_this(&self) -> bool {
        if let ArgumentType::Some(v) = self {
            v == "this"
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct BaseArgument<A: Clone, C: Clone> {
    pub argument_type: A,
    pub value: C,
}

pub type ArgumentDefinition = BaseArgument<ArgumentType, ValueDefinition>;

impl ArgumentDefinition {
    pub fn named(name: &str, value: ValueDefinition) -> ArgumentDefinition {
        ArgumentDefinition {
            argument_type: ArgumentType::Some(name.to_string()),
            value,
        }
    }

    pub fn unnamed(value: ValueDefinition) -> ArgumentDefinition {
        ArgumentDefinition {
            argument_type: ArgumentType::None,
            value,
        }
    }

    pub fn list(value: ValueDefinition) -> ArgumentDefinition {
        BaseArgument {
            argument_type: ArgumentType::ArgumentList,
            value,
        }
    }

    pub fn dict(value: ValueDefinition) -> ArgumentDefinition {
        BaseArgument {
            argument_type: ArgumentType::ArgumentDict,
            value,
        }
    }

    pub fn unnamed_value(&self) -> CrushResult<ValueDefinition> {
        if self.argument_type.is_some() {
            error("Expected an unnamed argument")
        } else {
            Ok(self.value.clone())
        }
    }
}

pub type Argument = BaseArgument<Option<String>, Value>;

impl Argument {
    pub fn new(name: Option<String>, value: Value) -> Argument {
        Argument {
            argument_type: name,
            value,
        }
    }

    pub fn unnamed(value: Value) -> Argument {
        Argument {
            argument_type: None,
            value,
        }
    }

    pub fn named(name: &str, value: Value) -> Argument {
        BaseArgument {
            argument_type: Some(name.to_string()),
            value,
        }
    }

}

pub trait ArgumentVecCompiler {
    fn compile(&self, context: &mut CompileContext) -> CrushResult<(Vec<Argument>, Option<Value>)>;
}

impl ArgumentVecCompiler for Vec<ArgumentDefinition> {
    fn compile(&self, context: &mut CompileContext) -> CrushResult<(Vec<Argument>, Option<Value>)> {
        let mut this = None;
        let mut res = Vec::new();
        for a in self {
            if a.argument_type.is_this() {
                this = Some(a.value.compile_bound(context)?);
            } else {
                match &a.argument_type {
                    ArgumentType::Some(name) =>
                        res.push(Argument::named(&name, a.value.compile_bound(context)?)),

                    ArgumentType::None =>
                        res.push(Argument::unnamed(a.value.compile_bound(context)?)),

                    ArgumentType::ArgumentList => {
                        match a.value.compile_bound(context)? {
                            Value::List(l) => {
                                let mut copy = l.dump();
                                for v in copy.drain(..) {
                                    res.push(Argument::unnamed(v));
                                }
                            }
                            _ => return argument_error("Argument list must be of type list"),
                        }
                    }

                    ArgumentType::ArgumentDict => {
                        match a.value.compile_bound(context)? {
                            Value::Dict(d) => {
                                let mut copy = d.elements();
                                for (key, value) in copy.drain(..) {
                                    if let Value::String(name) = key {
                                        res.push(Argument::named(&name, value));
                                    } else {
                                        return argument_error("Argument dict must have string keys");
                                    }
                                }
                            }
                            _ => return argument_error("Argument list must be of type list"),
                        }
                    }
                }
            }
        }
        Ok((res, this))
    }
}

pub fn column_names(arguments: &Vec<Argument>) -> Vec<String> {
    let mut taken = HashSet::new();
    taken.insert("_".to_string());
    let mut res = Vec::new();
    let mut tmp = String::new();
    for arg in arguments {
        let mut name = match &arg.argument_type {
            None => "_",
            Some(name) => name,
        };
        if taken.contains(name) {
            let mut idx = 1;
            tmp.truncate(0);
            tmp.push_str(name);
            loop {
                tmp.push_str(idx.to_string().as_str());
                idx += 1;
                if !taken.contains(tmp.as_str()) {
                    name = tmp.as_str();
                    break;
                }
                tmp.truncate(name.len());
            }
        }
        taken.insert(name.to_string());
        res.push(name.to_string());
    }

    res
}

pub trait ArgumentHandler : Sized {
    fn parse(arguments: Vec<Argument>, printer: &Printer) -> CrushResult<Self>;
}
