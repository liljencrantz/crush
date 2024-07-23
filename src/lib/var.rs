use std::sync::OnceLock;
use signature::signature;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::errors::{argument_error_legacy, mandate, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::value::{Value, ValueType};

#[signature(
    var.r#let,
    can_block = false,
    output = Unknown,
    short = "Declare new variables"
)]
struct Let {
    #[description("the variables to declare.")]
    #[named()]
    variables: OrderedStringMap<Value>,
}

pub fn r#let(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Let::parse(context.remove_arguments(), context.global_state.printer())?;
    for arg in cfg.variables {
        context.scope.declare(&arg.0, arg.1)?;
    }
    context.output.send(Value::Empty)
}

#[signature(
    var.set,
    can_block = false,
    output = Unknown,
    short = "Reassign existing variables"
)]
struct Set {
    #[description("the variables to declare.")]
    #[named()]
    variables: OrderedStringMap<Value>,
}

pub fn set(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Set::parse(context.remove_arguments(), context.global_state.printer())?;
    for arg in cfg.variables {
        context.scope.set(&arg.0, arg.1)?;
    }
    context.output.send(Value::Empty)
}

#[signature(
    var.get,
    can_block = false,
    output = Unknown,
    short = "Returns the current value of a variable"
)]
struct Get {
    #[description("the name of the variable to return the value of.")]
    name: String,
}

pub fn get(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Get::parse(context.remove_arguments(), context.global_state.printer())?;
    match context.scope.get(&cfg.name)? {
        None => argument_error_legacy("Unknown variable"),
        Some(value) => context.output.send(value),
    }
}

#[signature(
    var.unset,
    can_block = false,
    output = Unknown,
    short = "Removes variables from the namespace"
)]
struct Unset {
    #[description("the name of the variables to unset.")]
    #[unnamed()]
    name: Vec<String>,
}

pub fn unset(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Unset::parse(context.remove_arguments(), context.global_state.printer())?;
    for s in cfg.name {
        if s.len() == 0 {
            return argument_error_legacy("Illegal variable name");
        } else {
            context.scope.remove_str(&s)?;
        }
    }
    context.output.send(Value::Empty)
}

#[signature(
    var.r#use,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Puts the specified scope into the list of scopes to search in by default during scope lookups",
    example = "var:use $math; sqrt 2",
)]
struct Use {
    #[description("the scopes to use.")]
    #[unnamed()]
    name: Vec<Scope>,
}

pub fn r#use(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Use::parse(context.remove_arguments(), context.global_state.printer())?;
    for e in cfg.name {
        context.scope.r#use(&e);
    }
    context.output.send(Value::Empty)
}

fn env_output_type() -> &'static Vec<ColumnType> {
    static CELL: OnceLock<Vec<ColumnType>> = OnceLock::new();
    CELL.get_or_init(|| vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("type", ValueType::String),
    ])
}

#[signature(
    var.env,
    can_block = false,
    output = Known(ValueType::TableInputStream(env_output_type().clone())),
    short = "Returns a table containing the current namespace",
    long = "The columns of the table are the name, and the type of the value.",
)]
struct Env {}

pub fn env(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(env_output_type())?;
    let values = context.scope.dump()?;
    let mut keys = values.keys().collect::<Vec<&String>>();
    keys.sort();

    for k in keys {
        context.global_state.printer().handle_error(output.send(Row::new(vec![
            Value::from(k.clone()),
            Value::from(values[k].to_string()),
        ])));
    }
    context.output.send(Value::Empty)
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "var",
        "Commands related to variables",
        Box::new(move |ns| {
            Let::declare(ns)?;
            Set::declare(ns)?;
            Get::declare(ns)?;
            Unset::declare(ns)?;
            Use::declare(ns)?;
            Env::declare(ns)?;
            Ok(())
        }))?;
    Ok(())
}
