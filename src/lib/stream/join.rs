use crate::lang::errors::{argument_error_legacy, mandate};
use crate::lang::errors::CrushError;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::{ArgumentVector, CommandContext};
use crate::lang::printer::Printer;
use crate::lang::pipe::Stream;
use crate::lang::pipe::OutputStream;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::value::Value;
use std::collections::HashMap;
use crate::lang::ordered_string_map::OrderedStringMap;
use signature::signature;
use crate::lang::command::OutputType::Unknown;

fn combine(mut l: Row, r: Row, right_idx: usize) -> Row {
    for (idx, c) in Vec::from(r).drain(..).enumerate() {
        if idx != right_idx {
            l.push(c);
        }
    }
    l
}

fn do_join(
    mut l: Stream,
    left_idx: usize,
    mut r: Stream,
    right_idx: usize,
    output: &OutputStream,
    printer: &Printer,
) -> CrushResult<()> {
    let mut l_data: HashMap<Value, Row> = HashMap::new();

    while let Ok(row) = l.read() {
        l_data.insert(row.cells()[left_idx].clone(), row);
    }

    while let Ok(r_row) = r.read() {
        l_data
            .remove(&r_row.cells()[right_idx])
            .map(|l_row| {
                printer.handle_error(output.send(combine(l_row, r_row, right_idx)));
            });
    }
    Ok(())
}

fn get_output_type(left_type: &[ColumnType], right_type: &[ColumnType], right_key_idx: usize) -> Result<Vec<ColumnType>, CrushError> {
    let mut res = left_type.to_vec();
    for (idx, c) in right_type.iter().enumerate() {
        if idx != right_key_idx {
            res.push(c.clone());
        }
    }
    Ok(res)
}

#[signature(
join,
output = Unknown,
short = "Join two streams together on the specified keys.",
example = "join user=(ll) name=(user:list)")]
pub struct Join {
    #[named()]
    #[description("Field to join")]
    join: OrderedStringMap<Stream>,
}

pub fn join(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let l = context.arguments.remove(0);
    let r = context.arguments.remove(0);
    match (l.argument_type, mandate(l.value.stream()?, "Expected a stream")?,
           r.argument_type, mandate(r.value.stream()?, "Expected a stream")?) {
        (Some(left_name), left_stream, Some(right_name), right_stream) => {
            let left_idx = left_stream.types().find(&left_name)?;
            let right_idx = right_stream.types().find(&right_name)?;

            let output_type = get_output_type(left_stream.types(), right_stream.types(), right_idx)?;
            let output = context.output.initialize(output_type)?;

            do_join(left_stream, left_idx, right_stream, right_idx, &output, &context.global_state.printer())
        }
        (_, _, _, _) => argument_error_legacy("Invalid inputs for joins"),
    }
}
