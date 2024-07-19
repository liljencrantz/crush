use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;

use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{CrushResult, mandate};
use crate::data::list::List;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Resolve::declare_method(&mut res);
        GetItem::declare_method(&mut res);
        Parent::declare_method(&mut res);
        CurrentScope::declare_method(&mut res);
        All::declare_method(&mut res);
        Local::declare_method(&mut res);
        ReadOnly::declare_method(&mut res);
        Name::declare_method(&mut res);
        Use::declare_method(&mut res);

        res
    };
}

#[signature(
    types.scope.__getitem__,
    can_block = false,
    output = Unknown,
    short = "Return the specified member in the current scope",
)]
struct GetItem {
    name: String,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let val = context.this.scope()?;
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(mandate(val.get_local(&cfg.name)?, "Unknown member")?)
}

#[signature(
    types.scope.__resolve__,
    can_block = false,
    output = Unknown,
    short = "Resolve the specified member in the current scope",
    long = "This method looks at the current scope as well as all it parents to resolve the specified member",
)]
struct Resolve {
    name: String,
}

fn __resolve__(mut context: CommandContext) -> CrushResult<()> {
    let val = context.this.scope()?;
    let cfg: Resolve = Resolve::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(mandate(val.get(&cfg.name)?, "Unknown member")?)
}

#[signature(
    types.scope.__current_scope__,
    can_block = false,
    output = Known(ValueType::Scope),
    short = "The current scope.",
)]
struct CurrentScope {}

fn __current_scope__(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::Scope(context.scope))
}

#[signature(
    types.scope.__parent__,
    can_block = false,
    output = Known(ValueType::Scope),
    short = "The parent of this scope. The root (global) scope returns itself.",
)]
struct Parent {}

fn __parent__(mut context: CommandContext) -> CrushResult<()> {
    let scope = context.this.scope()?;
    context.output.send(Value::Scope(scope.parent().unwrap_or(scope)))
}

#[signature(
    types.scope.__all__,
    can_block = false,
    output = Known(ValueType::List(Box::new(ValueType::String))),
    short = "The names of all variable visible from the current scope.",
)]
struct All {}

fn __all__(mut context: CommandContext) -> CrushResult<()> {
    let scope = context.this.scope()?;
    context.output.send(
        List::new(ValueType::String,
                  scope.dump()?.iter().map(|e| { Value::from(e.0) }).collect::<Vec<_>>()).into())
}

#[signature(
    types.scope.__local__,
    can_block = false,
    output = Known(ValueType::List(Box::new(ValueType::String))),
    short = "The names of all variables defined in the local scope.",
)]
struct Local {}

fn __local__(mut context: CommandContext) -> CrushResult<()> {
    let scope = context.this.scope()?;
    context.output.send(
        List::new(ValueType::String,
                  scope.dump_local()?.iter().map(|e| { Value::from(e.0) }).collect::<Vec<_>>()).into())
}

#[signature(
    types.scope.__read_only__,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if this scope is write protected.",
)]
struct ReadOnly {}

fn __read_only__(mut context: CommandContext) -> CrushResult<()> {
    let scope = context.this.scope()?;
    context.output.send(Value::Bool(
        scope.is_read_only()))
}

#[signature(
    types.scope.__name__,
    can_block = false,
    output = Unknown,
    short = "The name of this scope, or empty if unnamed.",
)]
struct Name {}

fn __name__(mut context: CommandContext) -> CrushResult<()> {
    let scope = context.this.scope()?;
    context.output.send(
        scope.name().map(|n| { Value::from(n) }).unwrap_or(Value::Empty))
}

#[signature(
    types.scope.__use__,
    can_block = false,
    output = Known(ValueType::List(Box::new(ValueType::Scope))),
    short = "All use imports in this scope.",
)]
struct Use {}

fn __use__(mut context: CommandContext) -> CrushResult<()> {
    let scope = context.this.scope()?;
    context.output.send(
        List::new(ValueType::Scope, scope.get_use().drain(..).map(|s| { Value::Scope(s) }).collect::<Vec<_>>()).into())
}
