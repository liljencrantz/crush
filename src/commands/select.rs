use crate::commands::CompileContext;
use crate::{
     commands::command_util::find_field,
    errors::{argument_error},
    data::{
        Argument,
        Row,
        CellDefinition,
        Cell
    },
    stream::{OutputStream, InputStream},
    replace::Replace,
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
    let input = context.input.initialize()?;
    let config = parse(input.get_type(), &context.arguments)?;
    let output_type = config.columns.iter()
        .map(|(idx, name)| ColumnType {cell_type: input.get_type()[*idx].cell_type.clone(), name: name.clone() })
        .collect();
    let output = context.output.initialize(output_type)?;
    run(config, input, output)
}
