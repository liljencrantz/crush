use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{mandate, CrushResult};
use crate::lang::execution_context::This;
use crate::lang::value::ValueType;
use crate::lang::{execution_context::CommandContext, value::Value};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "binary"];
        Len::declare_method(&mut res, &path);
        GetItem::declare_method(&mut res, &path);
        res
    };
}

#[signature(
len,
can_block = false,
output = Known(ValueType::Integer),
short = "The number of bytes in the binary.",
)]
struct Len {}

fn len(context: CommandContext) -> CrushResult<()> {
    let val = context.this.binary()?;
    context.output.send(Value::Integer(val.len() as i128))
}

#[signature(
__getitem__,
can_block = false,
output = Known(ValueType::Integer),
short = "Returns the byte at the specified offset.",
example = "(bin:from Cargo.toml)[4]"
)]
struct GetItem {
    index: usize,
}

fn __getitem__(context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.arguments, &context.printer)?;
    let val = context.this.binary()?;
    context.output.send(Value::Integer(
        *mandate(val.get(cfg.index), "Index out of bounds")? as i128,
    ))
}
