use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{CrushResult, argument_error_legacy};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use signature::signature;

#[signature(
    var.r#let,
    can_block = false,
    output = Unknown,
    short = "Declare new variables in the current scope.",
    long = "No variable can exist in the local scope, or an error will result.",
    long = "",
    long = "The let builtin is not normally called directly, but via the syntactic sugar of the := operator.",
    example = "# These two lines are equivalent",
    example = "$x := 2",
    example = "var:let x=2",
)]
struct Let {
    #[description(
        "the variables to declare. The value you supply will be the initial value of the variable."
    )]
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
    short = "Reassign existing variables",
    long = "A variable by the specified name must already exist in some visible scope before calling 
    the `set` builtin, or an error will result.",
    long = "",
    long = "The set builtin is not normally called directly, but via the syntactic sugar of the = 
    operator.",
    example = "# These two lines are equivalent",
    example = "$x = 2",
    example = "var:set x=2",
)]
struct Set {
    #[description(
        "the variables to reassign. The value you supply will be the new value of the variable."
    )]
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
    short = "Returns the current value of a variable",
    long = "A variable by the specified name must already exist in some visible scope before calling
     the `get` builtin, or an error will result.",
    long = "",
    long = "The get builtin is not normally called directly, simply prefix the `$` sigil with the 
    name of the variable you want to get.",
    example = "# These two lines are equivalent",
    example = "$x",
    example = "var:get x",
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
    output = Known(ValueType::Empty),
    short = "Removes variables from the namespace",
    example = "# Remove the variable x",
    example = "var:unset x",
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
    short = "Make specified scopes searched by default during scope resolution.",
    example = "# Import the math scope",
    example = "var:use $math",
    example = "sqrt 2",
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

#[signature(
    var.unuse,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "Make specified scopes not searched by default during scope resolution.",
    long = "The unuse builtin will recursively go through all parent scopes and remove all uses of 
    the provided scopes through the entire chain.",
    example = "# Stop using the stream scope",
    example = "var:unuse $stream",
    example = "# This command will now fail, because stream::select is not in scope",
    example = "select",
)]
struct Unuse {
    #[description("the scopes to unuse.")]
    #[unnamed()]
    name: Vec<Scope>,
}

pub fn unuse(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Unuse::parse(context.remove_arguments(), context.global_state.printer())?;
    for e in cfg.name {
        context.scope.unuse(&e);
    }
    context.output.send(Value::Empty)
}

static LIST_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("name", ValueType::String),
    ColumnType::new("type", ValueType::String),
];

#[signature(
    var.list,
    can_block = false,
    output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
    short = "Returns a table containing all variable names currently in scope and their types.",
    long = "A variable is in scope if it exists in the current scope, any of its parents, or any of
     the scopes used in any of those scopes.",
)]
struct List {}

pub fn list(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
    let values = context.scope.dump()?;
    let mut keys = values.keys().collect::<Vec<&String>>();
    keys.sort();

    for k in keys {
        output.send(Row::new(vec![
            Value::from(k.clone()),
            Value::from(values[k].to_string()),
        ]))?;
    }
    Ok(())
}

#[signature(
    var.local,
    can_block = false,
    output = Known(ValueType::Scope),
    short = "Returns the current scope.",
)]
struct Local {}

pub fn local(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::Scope(context.scope))
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
            Unuse::declare(ns)?;
            List::declare(ns)?;
            Local::declare(ns)?;
            Ok(())
        }),
    )?;
    Ok(())
}
