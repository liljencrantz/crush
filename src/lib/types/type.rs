use crate::lang::errors::{CrushResult};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;
use crate::lang::argument::column_names;
use crate::lang::r#struct::Struct;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res
    };
}
