use lazy_static::lazy_static;
use signature::signature;
use crate::state::contexts::CommandContext;
use crate::lang::errors::{CrushResult};
use crate::lang::command::OutputType::Known;
use mountpoints::mountinfos;
use crate::data::table::Row;
use crate::lang::value::{Value, ValueType};
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnFormat;

lazy_static! {
    static ref OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new_with_format("size", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new_with_format("availble", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new_with_format("usage", ColumnFormat::Percentage, ValueType::Float),
        ColumnType::new("format", ValueType::String),
        ColumnType::new("readonly", ValueType::Any),
        ColumnType::new("name", ValueType::String),
        ColumnType::new("path", ValueType::File),
    ];
}

#[signature(
    fs.mounts,
    can_block = true,
    output = Known(ValueType::TableInputStream(OUTPUT_TYPE.clone())),
    short = "List mount points",
)]
pub struct Mounts {}

fn mounts(mut context: CommandContext) -> CrushResult<()> {
    let _cfg: Mounts = Mounts::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.initialize(&OUTPUT_TYPE)?;

    for m in mountinfos()? {
        let size = m.size.unwrap_or(0);
        let avail = m.avail.unwrap_or(0);

        output.send(Row::new(
            vec![
                Value::from(size),
                Value::from(avail),
                Value::from(if size == 0 { 0.0 } else { (avail as f64) / (size as f64) }),
                Value::from(m.format.unwrap_or("".to_string())),
                Value::from(m.readonly.map(|r| { Value::from(r) }).unwrap_or(Value::Empty)),
                Value::from(m.name.unwrap_or("".to_string())),
                Value::from(m.path),
            ]
        ))?;
    }

    Ok(())
}
