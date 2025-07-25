use crate::lang::command::OutputType::Unknown;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushError;
use crate::lang::errors::CrushResult;
use crate::lang::errors::command_error;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::Stream;
use crate::lang::pipe::TableOutputStream;
use crate::lang::printer::Printer;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use ordered_map::{Entry, OrderedMap};
use signature::signature;
use std::collections::HashSet;

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
    output: &TableOutputStream,
    printer: &Printer,
) -> CrushResult<()> {
    let mut l_data: OrderedMap<Value, Vec<Row>> = OrderedMap::new();

    // Read left table into memory
    while let Ok(row) = l.read() {
        match l_data.entry(row.cells()[left_idx].clone()) {
            Entry::Occupied(o) => o.into_mut().push(row),
            Entry::Vacant(v) => v.insert(vec![row]),
        }
    }

    // Read one row at a time of right table, and join on the left table.
    while let Ok(r_row) = r.read() {
        l_data.get(&r_row.cells()[right_idx]).map(|l_rows| {
            for l_row in l_rows {
                printer.handle_error(output.send(combine(l_row, &r_row, right_idx)));
            }
        });
    }
    Ok(())
}

fn get_output_type(
    left_type: &[ColumnType],
    right_type: &[ColumnType],
    right_key_idx: usize,
) -> Result<Vec<ColumnType>, CrushError> {
    let seen = left_type.iter().map(|c| c.name()).collect::<HashSet<_>>();
    let mut res = left_type.to_vec();

    for (idx, c) in right_type.iter().enumerate() {
        let mut name = c.name().to_string();
        let mut version = 1;
        while seen.contains(name.as_str()) {
            version += 1;
            name = format!("{}_{}", c.name(), version);
        }

        let column = ColumnType::new_from_string(name.to_string(), c.cell_type.clone());

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
#[allow(unused)]
pub struct Join {
    #[named()]
    #[description("Fields to join")]
    join: OrderedStringMap<Stream>,
}

pub fn join(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let l = context.arguments.remove(0);
    let r = context.arguments.remove(0);
    match (
        l.argument_type,
        l.value.stream()?,
        r.argument_type,
        r.value.stream()?,
    ) {
        (Some(left_name), left_stream, Some(right_name), right_stream) => {
            let left_idx = left_stream.types().find(&left_name)?;
            let right_idx = right_stream.types().find(&right_name)?;

            let output_type =
                get_output_type(left_stream.types(), right_stream.types(), right_idx)?;
            let output = context.output.initialize(&output_type)?;

            do_join(
                left_stream,
                left_idx,
                right_stream,
                right_idx,
                &output,
                &context.global_state.printer(),
            )
        }
        (_, _, _, _) => command_error("Invalid inputs for joins."),
    }
}
