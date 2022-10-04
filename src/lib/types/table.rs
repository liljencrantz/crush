use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use crate::lib::types::column_types;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::state::this::This;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "table"];
        Call::declare_method(&mut res, &path);
        Len::declare_method(&mut res, &path);
        GetItem::declare_method(&mut res, &path);
        res
    };
}

#[signature(
__call__,
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
            let cfg: Call = Call::parse(context.arguments, &context.global_state.printer())?;
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableInputStream(column_types(&cfg.columns))))
            } else if cfg.columns.is_empty() {
                context.output.send(Value::Type(ValueType::Table(c)))
            } else {
                argument_error_legacy("Tried to set columns on a table type that already has columns")
            }
        }
        _ => argument_error_legacy("Invalid this, expected type table"),
    }
}

#[signature(
len,
can_block = false,
output = Known(ValueType::Integer),
short = "The number of rows in the table.",
)]
struct Len {}

fn len(mut context: CommandContext) -> CrushResult<()> {
    let table = context.this.table()?;
    context
        .output
        .send(Value::Integer(table.len() as i128))
}

#[signature(
__getitem__,
can_block = false,
output = Known(ValueType::Struct),
short = "Returns the specified row of the table as a struct.",
example = "(bin:from Cargo.toml|materialize)[4]"
)]
struct GetItem {
    index: usize,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.arguments, &context.global_state.printer())?;
    let o = context.this.table()?;
    context.output.send(Value::Struct(
        o.row(cfg.index)?
            .into_struct(o.types()),
    ))
}
