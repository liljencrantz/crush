use crate::lang::errors::{argument_error_legacy, mandate};
use crate::lang::errors::CrushError;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::printer::Printer;
use crate::lang::pipe::Stream;
use crate::lang::pipe::OutputStream;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::value::Value;
use std::collections::HashSet;
use ordered_map::{Entry, OrderedMap};
use crate::lang::ordered_string_map::OrderedStringMap;
use signature::signature;
use crate::lang::command::OutputType::Unknown;
use crate::lang::state::argument_vector::ArgumentVector;

fn combine(l: &Row, r: &Row, right_idx: usize) -> Row {
    let mut l = l.clone();
    for (idx, c) in r.cells().iter().enumerate() {
        if idx != right_idx {
            l.push(c.clone());
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
    let mut l_data: OrderedMap<Value, Vec<Row>> = OrderedMap::new();

    // Read left table into memory
    while let Ok(row) = l.read() {
        match l_data.entry(row.cells()[left_idx].clone()) {
            Entry::Occupied(o) =>
                o.into_mut().push(row),
            Entry::Vacant(v) =>
                v.insert(vec![row]),
        }
    }

    // Read one row at a time of right table, and join on the left table.
    while let Ok(r_row) = r.read() {
        l_data
            .get(&r_row.cells()[right_idx])
            .map(|l_rows| {
                for l_row in l_rows {
                    printer.handle_error(output.send(combine(l_row, &r_row, right_idx)));
                }
            });
    }
    Ok(())
}

fn get_output_type(left_type: &[ColumnType], right_type: &[ColumnType], right_key_idx: usize) -> Result<Vec<ColumnType>, CrushError> {
    let seen =
        left_type.iter()
            .map(|c| { c.name.clone() })
            .collect::<HashSet<_>>();
    let mut res = left_type.to_vec();

    for (idx, c) in right_type.iter().enumerate() {
        let mut name = c.name.clone();
        let mut version = 1;
        while seen.contains(&name) {
            version += 1;
            name = format!("{}_{}", c.name, version);
        }

        let column = ColumnType::new(name.as_str(), c.cell_type.clone());

        if idx != right_key_idx {
            res.push(column);
        }
    }
    Ok(res)
}

#[signature(
    stream.join,
    output = Unknown,
    short = "Join two streams together on the specified keys.",
    example = "join user=(files) name=(user:list)")]
pub struct Join {
    #[named()]
    #[description("Fields to join")]
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
            let output = context.output.initialize(&output_type)?;

            do_join(left_stream, left_idx, right_stream, right_idx, &output, &context.global_state.printer())
        }
        (_, _, _, _) => argument_error_legacy("Invalid inputs for joins"),
    }
}
