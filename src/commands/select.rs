use crate::commands::CompileContext;
use crate::{
    commands::command_util::find_field_from_str,
    errors::argument_error,
    data::{
        Argument,
        Row,
        Value,
    },
    stream::{OutputStream},
    replace::Replace,
    data::ColumnType,
    errors::JobResult,
};
use crate::commands::command_util::find_field;
use crate::stream::{Readable, ValueSender};
use crate::errors::error;
use crate::data::{Struct, RowsReader};

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
                    Row::new(config.columns
                            .iter()
                            .map(|(idx, _name)| row.replace(*idx, Value::Integer(0)))
                            .collect()
                    ))?;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

fn perform_for(input: impl Readable, output: ValueSender, arguments: Vec<Argument>) -> JobResult<()> {
    let input_type = input.get_type();
    let columns = arguments.iter().map(|a| {
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
    }).collect::<JobResult<Vec<(usize, Option<Box<str>>)>>>()?;

    let output_type = columns.iter()
        .map(|(idx, name)| ColumnType { cell_type: input.get_type()[*idx].cell_type.clone(), name: name.clone() })
        .collect();
    let output = output.initialize(output_type)?;
    run(Config { columns: columns }, input, output)
}

fn perform_single(mut input: Struct, output: ValueSender, arguments: Vec<Argument>) -> JobResult<()> {
    if arguments.len() == 1 && arguments[0].name.is_none() {
        match &arguments[0].value {
            Value::Field(s) => output.send(input.remove(find_field(s, &input.types())?)),
            _ => Err(argument_error("Expected Field")),
        }
    } else {
        Err(error("NOT IMPLEMENTED!!!"))
    }
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            perform_for(input, context.output, context.arguments)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            perform_for(input, context.output, context.arguments)
        }
        Value::Struct(s) => {
            perform_single(s, context.output, context.arguments)
        }
        _ => Err(error("Expected a stream")),
    }
}
