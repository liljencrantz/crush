use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Len::declare_method(&mut res);
        GetItem::declare_method(&mut res);

        res
    })
}

#[signature(
    types.binary.len,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "The number of bytes in the binary.",
)]
struct Len {}

fn len(mut context: CommandContext) -> CrushResult<()> {
    let val = context.this.binary()?;
    context.output.send(Value::Integer(val.len() as i128))
}

#[signature(
    types.binary.__getitem__,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the byte at the specified offset.",
    example = "$(bin:from Cargo.toml)[4]",
)]
struct GetItem {
    #[description("index")]
    index: usize,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.arguments, &context.global_state.printer())?;
    let val = context.this.binary()?;
    context.output.send(Value::Integer(
        *val.get(cfg.index).ok_or("Index out of bounds")? as i128,
    ))
}
