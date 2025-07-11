use std::sync::OnceLock;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::{Value, ValueType};

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Call::declare_method(&mut res);
        res
    })
}

#[signature(
    types.one_of.__call__,
    can_block = false,
    output = Known(ValueType::Type),
    short = "Construct a one_of value type with the specified allowed types",
    example = "one_of:of $file $string $regex",
)]
#[allow(unused)]
struct Call {
    #[description("The allowed types")]
    #[unnamed]
    types: Vec<ValueType>,
}

fn __call__(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Call::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Type(ValueType::OneOf(cfg.types)))
}
