use crate::CrushResult;
use crate::data::dict::Dict;
use crate::data::list::List;
use crate::data::r#struct::Struct;
use crate::data::table::Table;
use crate::lang::errors::command_error;
use crate::lang::pipe::{TableInputStream, TableOutputStream};
use crate::lang::value::{Value, ValueType};
use crate::state::scope::Scope;
use crate::util::glob::Glob;
use chrono::{DateTime, Duration, Local};
use regex::Regex;
use std::mem::swap;
use std::path::PathBuf;

macro_rules! this_method {
    ($name:ident, $return_type:ty, $value_type:ident, $description:literal) => {
        fn $name(&mut self) -> CrushResult<$return_type> {
            let mut this = None;
            swap(self, &mut this);
            match this {
                Some(Value::$value_type(l)) => Ok(l),
                None => command_error(concat!(
                    "Expected `this` to be a `",
                    $description,
                    "`, but was not set."
                )),
                Some(v) => command_error(format!(
                    concat!(
                        "Expected `this` to be a `",
                        $description,
                        "`, but it was a `{}`."
                    ),
                    v.value_type().to_string()
                )),
            }
        }
    };
}

pub trait This {
    fn list(&mut self) -> CrushResult<List>;
    fn dict(&mut self) -> CrushResult<Dict>;
    fn string(&mut self) -> CrushResult<String>;
    fn r#struct(&mut self) -> CrushResult<Struct>;
    fn file(&mut self) -> CrushResult<PathBuf>;
    fn re(&mut self) -> CrushResult<(String, Regex)>;
    fn glob(&mut self) -> CrushResult<Glob>;
    fn integer(&mut self) -> CrushResult<i128>;
    fn float(&mut self) -> CrushResult<f64>;
    fn r#type(&mut self) -> CrushResult<ValueType>;
    fn duration(&mut self) -> CrushResult<Duration>;
    fn time(&mut self) -> CrushResult<DateTime<Local>>;
    fn table(&mut self) -> CrushResult<Table>;
    fn table_input_stream(&mut self) -> CrushResult<TableInputStream>;
    fn table_output_stream(&mut self) -> CrushResult<TableOutputStream>;
    fn binary(&mut self) -> CrushResult<Vec<u8>>;
    fn scope(&mut self) -> CrushResult<Scope>;
}

impl This for Option<Value> {
    this_method!(list, List, List, "list");
    this_method!(dict, Dict, Dict, "dict");

    fn string(&mut self) -> CrushResult<String> {
        let mut this = None;
        swap(self, &mut this);
        match this {
            Some(Value::String(l)) => Ok(l.to_string()),
            None => command_error(concat!(
                "Expected `this` to be a `string`, but was not set."
            )),
            Some(v) => command_error(format!(
                concat!("Expected `this` to be a `string`, but it was a `{}`."),
                v.value_type().to_string()
            )),
        }
    }

    fn file(&mut self) -> CrushResult<PathBuf> {
        let mut this = None;
        swap(self, &mut this);
        match this {
            Some(Value::File(l)) => Ok(l.to_path_buf()),
            None => command_error("Expected `this` to be a `file`, but was not set."),
            Some(v) => command_error(format!(
                concat!("Expected `this` to be a `file`, but it was a `{}`."),
                v.value_type().to_string()
            )),
        }
    }

    fn re(&mut self) -> CrushResult<(String, Regex)> {
        let mut this = None;
        swap(self, &mut this);

        match this {
            Some(Value::Regex(s, b)) => Ok((s, b)),
            None => command_error("Expected `this` to be a `re`, but was not set."),
            Some(v) => command_error(format!(
                "Expected `this` to be a `re`, but it was a `{}`.",
                v.value_type()
            )),
        }
    }

    this_method!(r#struct, Struct, Struct, "struct");
    this_method!(table, Table, Table, "table");
    this_method!(glob, Glob, Glob, "glob");
    this_method!(integer, i128, Integer, "integer");
    this_method!(float, f64, Float, "float");
    this_method!(r#type, ValueType, Type, "type");
    this_method!(duration, Duration, Duration, "duration");
    this_method!(time, DateTime<Local>, Time, "time");
    this_method!(scope, Scope, Scope, "scope");
    this_method!(
        table_input_stream,
        TableInputStream,
        TableInputStream,
        "table_input_stream"
    );
    this_method!(
        table_output_stream,
        TableOutputStream,
        TableOutputStream,
        "table_output_stream"
    );

    fn binary(&mut self) -> CrushResult<Vec<u8>> {
        let mut this = None;
        swap(self, &mut this);
        match this {
            Some(Value::Binary(l)) => Ok(l.to_vec()),
            None => command_error(concat!(
                "Expected `this` to be a `string`, but it was not set."
            )),
            Some(v) => command_error(format!(
                concat!("Expected `this` to be a `string`, but it was a `{}`."),
                v.value_type().to_string()
            )),
        }
    }
}
