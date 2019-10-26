use crate::commands::CompileContext;
use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellDefinition,
        Cell
    },
    stream::{OutputStream, InputStream},
    replace::Replace,
    printer::Printer,
    env::Env,
    data::ColumnType,
    errors::JobResult
};

pub struct Config {
    columns: Vec<(usize, Option<Box<str>>)>,
}

fn parse(input_type: &Vec<ColumnType>, arguments: &Vec<Argument>) -> JobResult<Config> {
    let columns: JobResult<Vec<(usize, Option<Box<str>>)>> = arguments.iter().enumerate().map(|(idx, a)| {
    match &a.cell {
        Cell::Text(s) | Cell::Field(s) => match find_field(s, input_type) {
            Ok(idx) => Ok((idx, a.name.clone().or(input_type[idx].name.clone()))),
            Err(e) => Err(e),
        }
        _ => Err(argument_error(format!("Expected Field, not {:?}", a.cell.cell_type()).as_str())),
    }
}).collect();

    Ok(Config {
        columns: columns?,
    })
}

pub fn run(
    config: Config,
    input: InputStream,
    output: OutputStream,
) -> JobResult<()> {
    loop {
        match input.recv() {
            Ok(mut row) => {
                output.send(
                    Row { cells: config.columns
                        .iter()
                        .map(|(idx, name)| row.cells.replace(*idx, Cell::Integer(0)))
                        .collect() })?;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let config = parse(&context.input_type, &context.arguments)?;
    let input_type = context.input_type.clone();

    let output_type = config.columns.iter()
        .map(|(idx, name)| ColumnType {cell_type: input_type[*idx].cell_type.clone(), name: name.clone() })
        .collect();
    Ok((Exec::Command(Box::from(move || run(config, context.input, context.output))), output_type))
}
