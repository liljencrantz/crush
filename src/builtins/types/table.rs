use crate::builtins::types::column_types;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::ordered_string_map::OrderedStringMap;
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

        Call::declare_method(&mut res);
        Len::declare_method(&mut res);
        GetItem::declare_method(&mut res);

        res
    })
}

#[signature(
    types.table.__call__,
    can_block = false,
    output = Known(ValueType::Type),
    short = "Return the table_input_stream type with the specified column signature.",
)]
struct Call {
    #[description("return the table type with the specified column signature.")]
    #[named()]
    columns: OrderedStringMap<ValueType>,
}

fn __call__(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::Table(c) => {
            let cfg: Call =
                Call::parse(context.remove_arguments(), &context.global_state.printer())?;
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableInputStream(column_types(
                        &cfg.columns,
                    ))))
            } else if cfg.columns.is_empty() {
                context.output.send(Value::Type(ValueType::Table(c)))
            } else {
                command_error("Tried to set columns on a table type that already has columns")
            }
        }
        _ => command_error("Invalid `this`, expected type table"),
    }
}

#[signature(
    types.table.len,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "The number of rows in the table.",
)]
struct Len {}

fn len(mut context: CommandContext) -> CrushResult<()> {
    let table = context.this.table()?;
    context.output.send(Value::Integer(table.len() as i128))
}

#[signature(
    types.table.__getitem__,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Returns the specified row of the table as a struct.",
    example = "$(bin:from Cargo.toml|materialize)[4]",
)]
struct GetItem {
    index: usize,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    let o = context.this.table()?;
    context
        .output
        .send(Value::Struct(o.row(cfg.index)?.into_struct(o.types())))
}
