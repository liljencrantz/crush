use crate::lang::value::Value;
use crate::lang::{value::ValueDefinition};
use crate::lang::errors::{CrushResult, error, argument_error};
use std::collections::HashSet;
use crate::lang::execution_context::CompileContext;

#[derive(Debug, Clone)]
pub enum ArgumentType {
    Some(Box<str>),
    None,
    ArgumentList,
    ArgumentDict,
}

impl ArgumentType {
    pub fn is_some(&self) -> bool {
        if let ArgumentType::Some(_) = self {
            true
        } else {
            false
        }
    }


    pub fn is_this(&self) -> bool {
        if let ArgumentType::Some(v) = self {
            v.as_ref() == "this"
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
            argument_type: ArgumentType::Some(Box::from(name)),
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

pub type Argument = BaseArgument<Option<Box<str>>, Value>;

impl Argument {
    pub fn new(name: Option<Box<str>>, value: Value) -> Argument {
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
            argument_type: Some(Box::from(name)),
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
                this = Some(a.value.compile(context)?.1);
            } else {
                match &a.argument_type {
                    ArgumentType::Some(name) =>
                        res.push(Argument::named(&name, a.value.compile(context)?.1)),

                    ArgumentType::None =>
                        res.push(Argument::unnamed(a.value.compile(context)?.1)),

                    ArgumentType::ArgumentList => {
                        match a.value.compile(context)?.1 {
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
                        match a.value.compile(context)?.1 {
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

pub fn column_names(arguments: &Vec<Argument>) -> Vec<Box<str>> {
    let mut taken = HashSet::new();
    taken.insert(Box::from("_"));
    let mut res = Vec::new();
    let mut tmp = String::new();
    for arg in arguments {
        let mut name = match &arg.argument_type {
            None => "_",
            Some(name) => name.as_ref(),
        };
        if taken.contains(&Box::from(name)) {
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
        taken.insert(Box::from(name));
        res.push(Box::from(name));
    }

    res
}
