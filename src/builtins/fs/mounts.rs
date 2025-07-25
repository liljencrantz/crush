use crate::data::table::Row;
use crate::lang::command::OutputType::Known;
use crate::lang::data::table::ColumnFormat;
use crate::lang::data::table::ColumnType;
use crate::lang::errors::CrushResult;
use crate::lang::value::{Value, ValueType};
use crate::state::contexts::CommandContext;
use mountpoints::mountinfos;
use signature::signature;

static OUTPUT_TYPE: [ColumnType; 7] = [
    ColumnType::new_with_format("size", ColumnFormat::ByteUnit, ValueType::Integer),
    ColumnType::new_with_format("available", ColumnFormat::ByteUnit, ValueType::Integer),
    ColumnType::new_with_format("usage", ColumnFormat::Percentage, ValueType::Float),
    ColumnType::new("format", ValueType::String),
    ColumnType::new("readonly", ValueType::Any),
    ColumnType::new("name", ValueType::String),
    ColumnType::new("path", ValueType::File),
];

#[signature(
    fs.mounts,
    can_block = true,
    output = Known(ValueType::table_input_stream(&OUTPUT_TYPE)),
    short = "List filesystem mount points",
    long = "`mounts` outputs the following information about each mount point:",
    long = "* `size` size in bytes.",
    long = "* `available` available space in bytes.",
    long = "* `usage` usage percentage.",
    long = "* `format` filesystem type (ntfs, ext4, etc.).",
    long = "* `readonly` whether the filesystem is mounted readonly.",
    long = "* `name` name assigned to this mountpoint, if any.",
    long = "* `path` the mount location.",
)]
pub struct Mounts {}

fn mounts(mut context: CommandContext) -> CrushResult<()> {
    let _cfg: Mounts = Mounts::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.initialize(&OUTPUT_TYPE)?;

    for m in mountinfos()? {
        let size = m.size.unwrap_or(0);
        let avail = m.avail.unwrap_or(0);

        output.send(Row::new(vec![
            Value::from(size),
            Value::from(avail),
            Value::from(if size == 0 {
                0.0
            } else {
                (avail as f64) / (size as f64)
            }),
            Value::from(m.format.unwrap_or("".to_string())),
            Value::from(m.readonly.map(|r| Value::from(r)).unwrap_or(Value::Empty)),
            Value::from(m.name.unwrap_or("".to_string())),
            Value::from(m.path),
        ]))?;
    }

    Ok(())
}
