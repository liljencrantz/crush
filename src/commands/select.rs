use crate::commands::CompileContext;
use crate::{
    commands::command_util::find_field_from_str,
    errors::argument_error,
    data::{
        Argument,
        Row,
        Value,
    },
    stream::{OutputStream, InputStream},
    replace::Replace,
    data::ColumnType,
    errors::JobResult,
};
use crate::commands::command_util::find_field;
use crate::stream::{RowsReader, Readable};
use crate::errors::error;

pub struct Config {
    columns: Vec<(usize, Option<Box<str>>)>,
}

pub fn run(
    config: Config,
    mut input: impl Readable,
    output: OutputStream,
) -> JobResult<()> {
    loop {
        match input.read() {
            Ok(mut row) => {
                output.send(
                    Row {
                        cells: config.columns
                            .iter()
                            .map(|(idx, _name)| row.cells.replace(*idx, Value::Integer(0)))
                            .collect()
                    })?;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

fn perform_for(input: impl Readable, arguments: &Vec<Argument>) -> JobResult<()> {
    let columns: JobResult<Vec<(usize, Option<Box<str>>)>> = arguments.iter().map(|a| {
        match &a.value {
            Value::Text(s) => match find_field_from_str(s, input_type) {
                Ok(idx) => Ok((idx, a.name.clone().or(input_type[idx].name.clone()))),
                Err(e) => Err(e),
            }
            Value::Field(s) => match find_field(s, input_type) {
                Ok(idx) => Ok((idx, a.name.clone().or(input_type[idx].name.clone()))),
                Err(e) => Err(e),
            }
            _ => Err(argument_error(format!("Expected Field, not {:?}", a.value.value_type()).as_str())),
        }
    }).collect();

    let output_type = columns.iter()
        .map(|(idx, name)| ColumnType { cell_type: input.get_type()[*idx].cell_type.clone(), name: name.clone() })
        .collect();
    let output = context.output.initialize(output_type)?;
    run(Config { columns: columns? }, input, output)
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            perform_for(input, &context.arguments)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            perform_for(input, &context.arguments)
        }
        _ => Err(error("Expected a stream")),
    }
}
