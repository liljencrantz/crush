use crate::lang::argument::Argument;
use crate::lang::command::Command;
use crate::lang::data::dict::Dict;
use crate::lang::data::list::List;
use crate::data::r#struct::Struct;
use crate::lang::state::scope::Scope;
use crate::lang::data::table::Table;
use crate::lang::errors::{argument_error_legacy, error, CrushResult};
use crate::lang::state::global_state::{GlobalState, JobHandle};
use crate::lang::pipe::{
    black_hole, empty_channel, InputStream, OutputStream, ValueReceiver, ValueSender,
};
use crate::lang::printer::Printer;
use crate::lang::value::{Value, ValueType};
use crate::util::glob::Glob;
use crate::util::replace::Replace;
use chrono::{DateTime, Duration, Local};
use regex::Regex;
use std::mem::swap;
use std::path::PathBuf;
use std::thread::ThreadId;

pub trait ArgumentVector {
    fn check_len(&self, len: usize) -> CrushResult<()>;
    fn check_len_range(&self, min_len: usize, max_len: usize) -> CrushResult<()>;
    fn check_len_min(&self, min_len: usize) -> CrushResult<()>;
    fn string(&mut self, idx: usize) -> CrushResult<String>;
    fn integer(&mut self, idx: usize) -> CrushResult<i128>;
    fn float(&mut self, idx: usize) -> CrushResult<f64>;
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
    fn optional_value(&mut self, idx: usize) -> CrushResult<Option<Value>>;
}

pub trait ArgumentHandler {
    fn parse(arg: Vec<Argument>) -> Self;
}

macro_rules! argument_getter {
    ($name:ident, $return_type:ty, $value_type:ident, $description:literal) => {
        fn $name(&mut self, idx: usize) -> CrushResult<$return_type> {
            if idx < self.len() {
                let l = self[idx].location;
                match self
                    .replace(idx, Argument::unnamed(Value::Bool(false), l))
                    .value
                {
                    Value::$value_type(s) => Ok(s),
                    v => argument_error_legacy(
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
                _ => argument_error_legacy("Wrong number of arguments"),
            }
        }
    };
}

impl ArgumentVector for Vec<Argument> {
    fn check_len(&self, len: usize) -> CrushResult<()> {
        if self.len() == len {
            Ok(())
        } else {
            argument_error_legacy(
                format!("Expected {} arguments, got {}", len, self.len()).as_str(),
            )
        }
    }

    fn check_len_range(&self, min_len: usize, max_len: usize) -> CrushResult<()> {
        if self.len() < min_len {
            argument_error_legacy(
                format!(
                    "Expected at least {} arguments, got {}",
                    min_len,
                    self.len()
                )
                    .as_str(),
            )
        } else if self.len() > max_len {
            argument_error_legacy(
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
            argument_error_legacy(
                format!(
                    "Expected at least {} arguments, got {}",
                    min_len,
                    self.len()
                )
                    .as_str(),
            )
        }
    }

    fn string(&mut self, idx: usize) -> CrushResult<String> {
        if idx < self.len() {
            let l = self[idx].location;
            match self
                .replace(idx, Argument::unnamed(Value::Empty, l))
                .value
            {
                Value::String(s) => Ok(s.to_string()),
                v => argument_error_legacy(
                    format!(
                        concat!("Invalid value, expected a string found a {}"),
                        v.value_type().to_string()
                    )
                        .as_str(),
                ),
            }
        } else {
            error("Index out of bounds")
        }
    }

    argument_getter!(integer, i128, Integer, "integer");
    argument_getter!(float, f64, Float, "float");
    argument_getter!(command, Command, Command, "command");
    argument_getter!(r#type, ValueType, Type, "type");
    argument_getter!(glob, Glob, Glob, "glob");
    argument_getter!(r#struct, Struct, Struct, "struct");
    argument_getter!(bool, bool, Bool, "bool");
    argument_getter!(file, PathBuf, File, "file");

    fn value(&mut self, idx: usize) -> CrushResult<Value> {
        if idx < self.len() {
            let l = self[idx].location;
            Ok(self
                .replace(idx, Argument::unnamed(Value::Bool(false), l))
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
    optional_argument_getter!(optional_command, Command, command);
    optional_argument_getter!(optional_value, Value, value);
}

/**
The data needed to be passed around while parsing and compiling code.
 */
pub struct CompileContext {
    pub env: Scope,
    pub global_state: GlobalState,
}

impl CompileContext {
    pub fn new(env: Scope, global_state: GlobalState) -> CompileContext {
        CompileContext { env, global_state }
    }

    pub fn job_context(&self, input: ValueReceiver, output: ValueSender) -> JobContext {
        JobContext::new(input, output, self.env.clone(), self.global_state.clone())
    }

    pub fn with_scope(&self, env: &Scope) -> CompileContext {
        CompileContext {
            env: env.clone(),
            global_state: self.global_state.clone(),
        }
    }
}

impl From<&JobContext> for CompileContext {
    fn from(c: &JobContext) -> Self {
        CompileContext::new(c.scope.clone(), c.global_state.clone())
    }
}

impl From<&CommandContext> for CompileContext {
    fn from(c: &CommandContext) -> Self {
        CompileContext::new(c.scope.clone(), c.global_state.clone())
    }
}

/**
The data needed to be passed around while executing a job.
 */
#[derive(Clone)]
pub struct JobContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub scope: Scope,
    pub global_state: GlobalState,
    pub handle: Option<JobHandle>,
}

impl JobContext {
    pub fn new(
        input: ValueReceiver,
        output: ValueSender,
        env: Scope,
        global_state: GlobalState,
    ) -> JobContext {
        JobContext {
            input,
            output,
            scope: env,
            global_state,
            handle: None,
        }
    }

    pub fn running(&self, desc: String) -> JobContext {
        JobContext {
            input: self.input.clone(),
            output: self.output.clone(),
            scope: self.scope.clone(),
            global_state: self.global_state.clone(),
            handle: Some(self.global_state.job_begin(desc)),
        }
    }

    pub fn with_io(&self, input: ValueReceiver, output: ValueSender) -> JobContext {
        JobContext {
            input,
            output,
            scope: self.scope.clone(),
            global_state: self.global_state.clone(),
            handle: self.handle.clone(),
        }
    }

    pub fn command_context(&self, arguments: Vec<Argument>, this: Option<Value>) -> CommandContext {
        CommandContext {
            arguments,
            this,
            input: self.input.clone(),
            output: self.output.clone(),
            scope: self.scope.clone(),
            global_state: self.global_state.clone(),
            handle: self.handle.clone(),
        }
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
        where
            F: FnOnce() -> CrushResult<()>,
            F: Send + 'static,
    {
        self.global_state.threads().spawn(name, self.handle.clone().map(|h| { h.id() }), f)
    }
}

/**
The data needed to be passed into a command when executing it.
 */
#[derive(Clone)]
pub struct CommandContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub scope: Scope,
    pub this: Option<Value>,
    pub global_state: GlobalState,
    handle: Option<JobHandle>,
}

impl CommandContext {
    /**
    Return a new Command context with the same scope and state, but empty I/O and arguments.
     */
    pub fn new(scope: &Scope, state: &GlobalState) -> CommandContext {
        CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            scope: scope.clone(),
            this: None,
            global_state: state.clone(),
            handle: None,
        }
    }

    /**
    Clear the argument vector and return the original.
     */
    pub fn remove_arguments(&mut self) -> Vec<Argument> {
        let mut tmp = Vec::new(); // This does not cause a memory allocation
        swap(&mut self.arguments, &mut tmp);
        tmp
    }

    /**
    Return a new Command context with the same scope and state, but otherwise empty.
     */
    pub fn empty(&self) -> CommandContext {
        CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            scope: self.scope.clone(),
            this: None,
            global_state: self.global_state.clone(),
            handle: self.handle.clone(),
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different argument vector.
     */
    pub fn with_args(self, arguments: Vec<Argument>, this: Option<Value>) -> CommandContext {
        CommandContext {
            input: self.input,
            output: self.output,
            scope: self.scope,
            arguments,
            this,
            global_state: self.global_state,
            handle: self.handle,
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different output sender.
     */
    pub fn with_output(self, sender: ValueSender) -> CommandContext {
        CommandContext {
            input: self.input,
            output: sender,
            scope: self.scope,
            arguments: self.arguments,
            this: self.this,
            global_state: self.global_state,
            handle: self.handle,
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different output sender.
     */
    pub fn with_scope(self, scope: Scope) -> CommandContext {
        CommandContext {
            input: self.input,
            output: self.output,
            scope,
            arguments: self.arguments,
            this: self.this,
            global_state: self.global_state,
            handle: self.handle,
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different input receiver.
     */
    pub fn with_input(self, input: ValueReceiver) -> CommandContext {
        CommandContext {
            input: input,
            output: self.output,
            scope: self.scope,
            arguments: self.arguments,
            this: self.this,
            global_state: self.global_state,
            handle: self.handle.clone(),
        }
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
        where
            F: FnOnce() -> CrushResult<()>,
            F: Send + 'static,
    {
        self.global_state.threads().spawn(name, self.handle.clone().map(|h| { h.id() }), f)
    }
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
    fn table_input_stream(&mut self) -> CrushResult<InputStream>;
    fn table_output_stream(&mut self) -> CrushResult<OutputStream>;
    fn binary(&mut self) -> CrushResult<Vec<u8>>;
    fn scope(&mut self) -> CrushResult<Scope>;
}

macro_rules! this_method {
    ($name:ident, $return_type:ty, $value_type:ident, $description:literal) => {
        fn $name(&mut self) -> CrushResult<$return_type> {
            let mut this = None;
            swap(self, &mut this);
            match this {
                Some(Value::$value_type(l)) => Ok(l),
                None => argument_error_legacy(concat!(
                    "Expected this to be a ",
                    $description,
                    ", but this is not set"
                )),
                Some(v) => argument_error_legacy(
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

    fn string(&mut self) -> CrushResult<String> {
        let mut this = None;
        swap(self, &mut this);
        match this {
            Some(Value::String(l)) => Ok(l.to_string()),
            None => argument_error_legacy(concat!("Expected this to be a string, but this is not set")),
            Some(v) => argument_error_legacy(
                format!(
                    concat!("Expected this to be a string, but it is a {}"),
                    v.value_type().to_string()
                ).as_str(),
            ),
        }
    }

    fn binary(&mut self) -> CrushResult<Vec<u8>> {
        let mut this = None;
        swap(self, &mut this);
        match this {
            Some(Value::Binary(l)) => Ok(l.to_vec()),
            None => argument_error_legacy(concat!("Expected this to be a string, but this is not set")),
            Some(v) => argument_error_legacy(
                format!(
                    concat!("Expected this to be a string, but it is a {}"),
                    v.value_type().to_string()
                ).as_str(),
            ),
        }
    }

    this_method!(r#struct, Struct, Struct, "struct");
    this_method!(file, PathBuf, File, "file");
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
        InputStream,
        TableInputStream,
        "table_input_stream"
    );
    this_method!(
        table_output_stream,
        OutputStream,
        TableOutputStream,
        "table_output_stream"
    );

    fn re(&mut self) -> CrushResult<(String, Regex)> {
        let mut this = None;
        swap(self, &mut this);

        match this {
            Some(Value::Regex(s, b)) => Ok((s, b)),
            _ => argument_error_legacy("Expected a regular expression"),
        }
    }
}
