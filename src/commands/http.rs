use crate::data::{Argument, Value, Struct, Rows, ColumnType, ValueType, Row};
use crate::commands::CompileContext;
use crate::data::{binary, BinaryReader};
use crate::errors::{argument_error, to_job_error, JobResult};
use reqwest::StatusCode;
use reqwest::header::HeaderMap;

pub struct Config {
    url: String,
}

fn parse(arguments: &Vec<Argument>) -> JobResult<Config> {
    match arguments.len() {
        1 => match &arguments[0].value {
            Value::Text(t) => Ok(Config { url: t.to_string() }),
            _ => Err(argument_error("Expected URI to be a string"))
        }
        _ => Err(argument_error("Expected URI"))
    }
}

pub fn perform(context: CompileContext) -> JobResult<()> {

    let cfg = parse(&context.arguments)?;
    let (mut output, input) = binary()?;
    let mut b = to_job_error(reqwest::blocking::get(cfg.url.as_str()))?;
    let status: StatusCode = b.status();
    let header_map: &HeaderMap = b.headers();
    let headers = Rows {
        types: vec![
            ColumnType::named("name", ValueType::Text),
            ColumnType::named("value", ValueType::Text),
        ],
        rows: header_map
            .iter()
            .map(|(n, v)| Row {
                cells: vec![Value::text(n.as_str()), Value::text(v.to_str().unwrap())]
            })
            .collect(),
    };
    context.output.send(
            Value::Struct(Struct::new(
                vec![
                    ("status", Value::Integer(status.as_u16() as i128)),
                    ("headers", Value::Rows(headers)),
                    ("body", Value::BinaryReader(input))
                ]
        )));
    to_job_error(b.copy_to(output.as_mut()))?;
    Ok(())
}
