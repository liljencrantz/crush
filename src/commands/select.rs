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
    data::CellFnurp,
    errors::JobResult
};

pub struct Config {
    input: InputStream,
    output: OutputStream,
    columns: Vec<(usize, Option<Box<str>>)>,
}

fn parse(input_type: &Vec<CellFnurp>, arguments: &Vec<Argument>, input: InputStream, output: OutputStream) -> JobResult<Config> {
    let columns: JobResult<Vec<(usize, Option<Box<str>>)>> = arguments.iter().enumerate().map(|(idx, a)| {
    match &a.cell {
        Cell::Text(s) | Cell::Field(s) => match find_field(s, input_type) {
            Ok(idx) => Ok((idx, a.name.clone().or(input_type[idx].name.clone()))),
            Err(e) => Err(e),
        }
        _ => Err(argument_error(format!("Expected Field, not {:?}", a.cell.cell_data_type()).as_str())),
    }
}).collect();

    Ok(Config {
        input,
        output,
        columns: columns?,
    })
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    loop {
        match config.input.recv() {
            Ok(mut row) => {
                config.output.send(
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

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let config = parse(&input_type, &arguments, input, output)?;
    let output_type = config.columns.iter()
        .map(|(idx, name)| CellFnurp {cell_type: input_type[*idx].cell_type.clone(), name: name.clone() })
        .collect();
    Ok((Exec::Select(config), output_type))
}
