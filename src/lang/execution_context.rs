use crate::lang::argument::Argument;
use crate::lang::command::Command;
use crate::lang::dict::Dict;
use crate::lang::errors::{argument_error, error, CrushResult};
use crate::lang::job::JobJoinHandle;
use crate::lang::list::List;
use crate::lang::printer::Printer;
use crate::lang::r#struct::Struct;
use crate::lang::scope::Scope;
use crate::lang::stream::{InputStream, OutputStream, ValueReceiver, ValueSender};
use crate::lang::table::{Table, TableReader};
use crate::lang::value::{Value, ValueType};
use crate::util::glob::Glob;
use crate::util::replace::Replace;
use chrono::{DateTime, Duration, Local};
use regex::Regex;
use std::path::PathBuf;

pub trait ArgumentVector {
    fn check_len(&self, len: usize) -> CrushResult<()>;
    fn check_len_range(&self, min_len: usize, max_len: usize) -> CrushResult<()>;
    fn check_len_min(&self, min_len: usize) -> CrushResult<()>;
    fn string(&mut self, idx: usize) -> CrushResult<String>;
    fn integer(&mut self, idx: usize) -> CrushResult<i128>;
    fn float(&mut self, idx: usize) -> CrushResult<f64>;
    fn field(&mut self, idx: usize) -> CrushResult<Vec<String>>;
    fn file(&mut self, idx: usize) -> CrushResult<PathBuf>;
    fn command(&mut self, idx: usize) -> CrushResult<Command>;
    fn r#type(&mut self, idx: usize) -> CrushResult<ValueType>;
    fn value(&mut self, idx: usize) -> CrushResult<Value>;
    fn glob(&mut self, idx: usize) -> CrushResult<Glob>;
    fn r#struct(&mut self, idx: usize) -> CrushResult<Struct>;
    fn bool(&mut self, idx: usize) -> CrushResult<bool>;
    fn files(&mut self, printer: &Printer) -> CrushResult<Vec<PathBuf>>;
    fn optional_bool(&mut self, idx: usize) -> CrushResult<Option<bool>>;
    fn optional_integer(&mut self, idx: usize) -> CrushResult<Option<i128>>;
    fn optional_string(&mut self, idx: usize) -> CrushResult<Option<String>>;
    fn optional_command(&mut self, idx: usize) -> CrushResult<Option<Command>>;
    fn optional_field(&mut self, idx: usize) -> CrushResult<Option<Vec<String>>>;
    fn optional_value(&mut self, idx: usize) -> CrushResult<Option<Value>>;
}

pub trait ArgumentHandler {
    fn parse(arg: Vec<Argument>) -> Self;
}

macro_rules! argument_getter {
    ($name:ident, $return_type:ty, $value_type:ident, $description:literal) => {
        fn $name(&mut self, idx: usize) -> CrushResult<$return_type> {
            if idx < self.len() {
                match self
                    .replace(idx, Argument::unnamed(Value::Bool(false)))
                    .value
                {
                    Value::$value_type(s) => Ok(s),
                    v => argument_error(
                        format!(
                            concat!("Invalid value, expected a ", $description, ", found a {}"),
                            v.value_type().to_string()
                        )
                        .as_str(),
                    ),
                }
            } else {
                error("Index out of bounds")
            }
        }
    };
}

macro_rules! optional_argument_getter {
    ($name:ident, $return_type:ty, $method:ident) => {
        fn $name(&mut self, idx: usize) -> CrushResult<Option<$return_type>> {
            match self.len() - idx {
                0 => Ok(None),
                1 => Ok(Some(self.$method(idx)?)),
                _ => argument_error("Wrong number of arguments"),
            }
        }
    };
}

impl ArgumentVector for Vec<Argument> {
    fn check_len(&self, len: usize) -> CrushResult<()> {
        if self.len() == len {
            Ok(())
        } else {
            argument_error(format!("Expected {} arguments, got {}", len, self.len()).as_str())
        }
    }

    fn check_len_range(&self, min_len: usize, max_len: usize) -> CrushResult<()> {
        if self.len() < min_len {
            argument_error(
                format!(
                    "Expected at least {} arguments, got {}",
                    min_len,
                    self.len()
                )
                .as_str(),
            )
        } else if self.len() > max_len {
            argument_error(
                format!("Expected at most {} arguments, got {}", max_len, self.len()).as_str(),
            )
        } else {
            Ok(())
        }
    }

    fn check_len_min(&self, min_len: usize) -> CrushResult<()> {
        if self.len() >= min_len {
            Ok(())
        } else {
            argument_error(
                format!(
                    "Expected at least {} arguments, got {}",
                    min_len,
                    self.len()
                )
                .as_str(),
            )
        }
    }

    argument_getter!(string, String, String, "string");
    argument_getter!(integer, i128, Integer, "integer");
    argument_getter!(float, f64, Float, "float");
    argument_getter!(field, Vec<String>, Field, "field");
    argument_getter!(command, Command, Command, "command");
    argument_getter!(r#type, ValueType, Type, "type");
    argument_getter!(glob, Glob, Glob, "glob");
    argument_getter!(r#struct, Struct, Struct, "struct");
    argument_getter!(bool, bool, Bool, "bool");
    argument_getter!(file, PathBuf, File, "file");

    fn value(&mut self, idx: usize) -> CrushResult<Value> {
        if idx < self.len() {
            Ok(self
                .replace(idx, Argument::unnamed(Value::Bool(false)))
                .value)
        } else {
            error("Index out of bounds")
        }
    }

    fn files(&mut self, printer: &Printer) -> CrushResult<Vec<PathBuf>> {
        let mut files = Vec::new();
        for a in self.drain(..) {
            a.value.file_expand(&mut files, printer)?;
        }
        Ok(files)
    }

    optional_argument_getter!(optional_bool, bool, bool);
    optional_argument_getter!(optional_integer, i128, integer);
    optional_argument_getter!(optional_string, String, string);
    optional_argument_getter!(optional_field, Vec<String>, field);
    optional_argument_getter!(optional_command, Command, command);
    optional_argument_getter!(optional_value, Value, value);
}

pub struct CompileContext {
    pub dependencies: Vec<JobJoinHandle>,
    pub env: Scope,
    pub printer: Printer,
}

impl CompileContext {
    pub fn new(env: Scope, printer: Printer) -> CompileContext {
        CompileContext {
            dependencies: Vec::new(),
            env,
            printer,
        }
    }

    pub fn job_context(&self, input: ValueReceiver, output: ValueSender) -> JobContext {
        JobContext::new(input, output, self.env.clone(), self.printer.clone())
    }

    pub fn with_scope(&self, env: &Scope) -> CompileContext {
        CompileContext {
            dependencies: vec![],
            env: env.clone(),
            printer: self.printer.clone(),
        }
    }
}

#[derive(Clone)]
pub struct JobContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub env: Scope,
    pub printer: Printer,
}

impl JobContext {
    pub fn new(
        input: ValueReceiver,
        output: ValueSender,
        env: Scope,
        printer: Printer,
    ) -> JobContext {
        JobContext {
            input,
            output,
            env,
            printer,
        }
    }

    pub fn with_io(&self, input: ValueReceiver, output: ValueSender) -> JobContext {
        JobContext {
            input,
            output,
            env: self.env.clone(),
            printer: self.printer.clone(),
        }
    }

    pub fn compile_context(&self) -> CompileContext {
        CompileContext::new(self.env.clone(), self.printer.clone())
    }

    pub fn execution_context(
        &self,
        arguments: Vec<Argument>,
        this: Option<Value>,
    ) -> ExecutionContext {
        ExecutionContext {
            arguments,
            this,
            input: self.input.clone(),
            output: self.output.clone(),
            printer: self.printer.clone(),
            env: self.env.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ExecutionContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub env: Scope,
    pub this: Option<Value>,
    pub printer: Printer,
}

#[allow(unused)] // TODO: remove me?
pub struct StreamExecutionContext<C> {
    pub config: C,
    pub input: TableReader,
    pub output: OutputStream,
    pub env: Scope,
    pub printer: Printer,
}

#[allow(unused)] // TODO: remove me?
pub struct ValueExecutionContext<C> {
    pub config: C,
    pub env: Scope,
    pub printer: Printer,
}

impl ExecutionContext {
    pub fn compile_context(&self) -> CompileContext {
        CompileContext::new(self.env.clone(), self.printer.clone())
    }

    pub fn with_args(self, arguments: Vec<Argument>, this: Option<Value>) -> ExecutionContext {
        ExecutionContext {
            input: self.input,
            output: self.output,
            env: self.env,
            printer: self.printer,
            arguments,
            this,
        }
    }

    pub fn with_sender(self, sender: ValueSender) -> ExecutionContext {
        ExecutionContext {
            input: self.input,
            output: sender,
            env: self.env,
            printer: self.printer,
            arguments: self.arguments,
            this: self.this,
        }
    }
}

pub trait This {
    fn list(self) -> CrushResult<List>;
    fn dict(self) -> CrushResult<Dict>;
    fn string(self) -> CrushResult<String>;
    fn r#struct(self) -> CrushResult<Struct>;
    fn file(self) -> CrushResult<PathBuf>;
    fn re(self) -> CrushResult<(String, Regex)>;
    fn glob(self) -> CrushResult<Glob>;
    fn integer(self) -> CrushResult<i128>;
    fn float(self) -> CrushResult<f64>;
    fn r#type(self) -> CrushResult<ValueType>;
    fn duration(self) -> CrushResult<Duration>;
    fn time(self) -> CrushResult<DateTime<Local>>;
    fn table(self) -> CrushResult<Table>;
    fn table_stream(self) -> CrushResult<InputStream>;
    fn binary(self) -> CrushResult<Vec<u8>>;
    fn scope(self) -> CrushResult<Scope>;
}

macro_rules! this_method {
    ($name:ident, $return_type:ty, $value_type:ident, $description:literal) => {
        fn $name(mut self) -> CrushResult<$return_type> {
            match self.take() {
                Some(Value::$value_type(l)) => Ok(l),
                None => argument_error(concat!(
                    "Expected this to be a ",
                    $description,
                    ", but this is not set"
                )),
                Some(v) => argument_error(
                    format!(
                        concat!("Expected this to be a ", $description, ", but it is a {}"),
                        v.value_type().to_string()
                    )
                    .as_str(),
                ),
            }
        }
    };
}

impl This for Option<Value> {
    this_method!(list, List, List, "list");
    this_method!(dict, Dict, Dict, "dict");
    this_method!(string, String, String, "string");
    this_method!(r#struct, Struct, Struct, "struct");
    this_method!(file, PathBuf, File, "file");
    this_method!(table, Table, Table, "table");
    this_method!(binary, Vec<u8>, Binary, "binary");
    this_method!(glob, Glob, Glob, "glob");
    this_method!(integer, i128, Integer, "integer");
    this_method!(float, f64, Float, "float");
    this_method!(r#type, ValueType, Type, "type");
    this_method!(duration, Duration, Duration, "duration");
    this_method!(time, DateTime<Local>, Time, "time");
    this_method!(scope, Scope, Scope, "scope");
    this_method!(table_stream, InputStream, TableStream, "table_stream");

    fn re(mut self) -> CrushResult<(String, Regex)> {
        match self.take() {
            Some(Value::Regex(s, b)) => Ok((s, b)),
            _ => argument_error("Expected a regular expression"),
        }
    }
}

/*
pub struct StreamExecutionContext {
    pub argument_stream: InputStream,
    pub output: ValueSender,
    pub env: Scope,
}
*/
