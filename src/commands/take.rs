use crate::commands::CompileContext;
use crate::errors::{JobResult, mandate, error};
use crate::data::CellType;
use crate::data::Row;
use crate::data::Cell;
use crate::data::ColumnType;
use crate::env::Env;

struct Config {
    lines: i128,
    location: Env,
    name: Vec<Box<str>>,
}

fn parse(context: &CompileContext) -> JobResult<Config> {
    let mut lines = 1;
    let mut loc = None;
    for arg in &context.arguments {
        match (&arg.name, &arg.cell) {
            (None, Cell::Text(c)) =>
                loc = context.env
                    .get_location(&c.split('.')
                        .map(|e: &str| Box::from(e))
                        .collect::<Vec<Box<str>>>()[..]),
            (Some(t), _) => match (t.as_ref(), &arg.cell) {
                ("lines", Cell::Integer(i)) => lines = *i,
                _ => return Err(error("Unknown argument")),
            },
            _ => return Err(error("Unknown argument")),
        }
    }
    let yes_loc = mandate(loc, "No variable name given")?;
    Ok(Config {
        lines,
        location: yes_loc.0,
        name: yes_loc.1,
    })
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let config = parse(&context)?;
    if let Cell::Output(cell) = mandate(config.location.get(&config.name), "Unknown variable")? {
        let output = context.output
            .initialize(cell.stream.get_type().clone())?;
        for i in 0..config.lines {
            output.send(cell.stream.recv()?);
        }
        config.location.declare(&config.name, Cell::Output(cell));
    }
    Ok(())
}
