use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::{ArgumentVector, This};
use crate::lang::data::list::List;
use crate::lang::value::ValueType;
use crate::lang::{execution_context::CommandContext, value::Value};
use crate::util::file::cwd;
use crate::util::glob::Glob;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "glob", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        res.declare(
            full("new"),
            new,
            false,
            "glob:new pattern:string",
            "Return a new glob",
            None,
            Known(ValueType::Glob),
            vec![],
        );
        res.declare(
            full("match"),
            r#match,
            false,
            "glob:match io:string",
            "True if the io matches the pattern",
            None,
            Known(ValueType::Bool),
            vec![],
        );
        res.declare(
            full("not_match"),
            not_match,
            false,
            "glob:not_match io:string",
            "True if the io does not match the pattern",
            None,
            Known(ValueType::Bool),
            vec![],
        );
        res.declare(
            full("files"),
            r#files,
            false,
            "glob:files",
            "Perform file matching of this glob",
            None,
            Known(ValueType::List(Box::from(ValueType::File))),
            vec![],
        );
        res
    };
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    let def = context.arguments.string(0)?;
    context.output.send(Value::Glob(Glob::new(&def)))
}

fn r#match(mut context: CommandContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(g.matches(&needle)))
}

fn not_match(mut context: CommandContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(!g.matches(&needle)))
}

fn files(mut context: CommandContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let mut files = Vec::new();
    g.glob_files(&cwd()?, &mut files)?;
    context.output.send(Value::List(List::new(
        ValueType::File,
        files.drain(..).map(|f| Value::File(f)).collect(),
    )))
}
