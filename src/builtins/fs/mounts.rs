use crate::data::table::Row;
use crate::lang::any_sequence::AnySequence;
use crate::lang::command::OutputType::Known;
use crate::lang::data::table::ColumnFormat;
use crate::lang::data::table::ColumnType;
use crate::lang::errors::CrushResult;
use crate::lang::value::{Value, ValueType};
use crate::state::contexts::CommandContext;
use mountpoints::mountinfos;
use signature::signature;

static OUTPUT_TYPE: [ColumnType; 7] = [
    ColumnType::new_with_format("size", ColumnFormat::ByteUnit, ValueType::OneOf(AnySequence::Slice(&[ValueType::Integer, ValueType::Empty])), "size in bytes, if known."),
    ColumnType::new_with_format("available", ColumnFormat::ByteUnit, ValueType::OneOf(AnySequence::Slice(&[ValueType::Integer, ValueType::Empty])), "available space in bytes, if known."),
    ColumnType::new_with_format("usage", ColumnFormat::Percentage, ValueType::OneOf(AnySequence::Slice(&[ValueType::Float, ValueType::Empty])), "usage percentage, if known."),
    ColumnType::new("format", ValueType::OneOf(AnySequence::Slice(&[ValueType::String, ValueType::Empty])), "filesystem type (ntfs, ext4, etc.), if known."),
    ColumnType::new("readonly", ValueType::OneOf(AnySequence::Slice(&[ValueType::Bool, ValueType::Empty])), "whether the filesystem is mounted readonly, if known."),
    ColumnType::new("name", ValueType::OneOf(AnySequence::Slice(&[ValueType::String, ValueType::Empty])), "name assigned to this mountpoint, if any."),
    ColumnType::new("path", ValueType::File, "the mount location."),
];

#[signature(
    fs.mounts,
    can_block = true,
    output = Known(ValueType::table_input_stream(&OUTPUT_TYPE)),
    short = "List filesystem mount points.",
)]
pub struct Mounts {}

fn mounts(mut context: CommandContext) -> CrushResult<()> {
    let _cfg: Mounts = Mounts::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.initialize_output(&OUTPUT_TYPE)?;

    for m in mountinfos()? {
        let usage = match (m.size, m.avail) {
            (Some(size), Some(avail)) => Value::from(if size == 0 {
                0.0
            } else {
                (avail as f64) / (size as f64)
            }),
            _ => Value::Empty,
        };

        output.send(Row::new(vec![
            m.size.map(Value::from).unwrap_or(Value::Empty),
            m.avail.map(Value::from).unwrap_or(Value::Empty),
            usage,
            m.format.map(Value::from).unwrap_or(Value::Empty),
            m.readonly.map(Value::from).unwrap_or(Value::Empty),
            m.name.map(Value::from).unwrap_or(Value::Empty),
            Value::from(m.path),
        ]))?;
    }

    Ok(())
}
