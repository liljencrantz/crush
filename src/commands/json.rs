use crate::commands::CompileContext;
use crate::{
    data::{
        Argument,
        Row,
        ValueType,
        Value,
    },
    errors::{JobError, argument_error},
};
use std::{
    io::BufReader,
    fs::File,
    path::Path,
};

use crate::printer::Printer;
use crate::data::{Struct, List, Rows, BinaryReader};
use crate::errors::{JobResult, to_job_error, error};
use crate::stream::{ValueSender, ValueReceiver};
use std::collections::HashSet;

pub struct Config {
    input: BinaryReader,
}

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> JobResult<Config> {
    match arguments.len() {
        0 => {
            let v = input.recv()?;
            match v {
                Value::BinaryReader(b) => {
                    Ok(Config {
                        input: b,
                    })
                }
                _ => Err(argument_error("Expected either a file to read or binary pipe input"))
            }
        }
        1 => {
            let mut files = Vec::new();
            arguments[0].value.file_expand(&mut files);
            Ok(Config {
                input: BinaryReader::from(&files.remove(0))?,
            })
        }
        _ => Err(argument_error("Expected a file name"))
    }
}

fn convert_json(json_value: &serde_json::Value) -> JobResult<Value> {
    match json_value {
        serde_json::Value::Null => Ok(Value::Empty()),
        serde_json::Value::Bool(b) => Ok(Value::Bool(b.clone())),
        serde_json::Value::Number(f) => {
            if f.is_u64() {
                Ok(Value::Integer(f.as_u64().expect("") as i128))
            } else if f.is_i64() {
                Ok(Value::Integer(f.as_i64().expect("") as i128))
            } else {
                Ok(Value::Float(f.as_f64().ok_or(error("Not a valid number"))?))
            }
        }
        serde_json::Value::String(s) => Ok(Value::Text(Box::from(s.clone()))),
        serde_json::Value::Array(arr) => {
            let mut lst = arr.iter()
                .map(|v| convert_json(v))
                .collect::<JobResult<Vec<Value>>>()?;
            let types: HashSet<ValueType> = lst.iter().map(|v| v.value_type()).collect();
            match types.len() {
                0 => Ok(Value::Empty()),
                1 => {
                    let list_type = types.iter().next().unwrap();
                    if let ValueType::Row(r) = list_type {
                        let row_list = lst
                            .drain(..)
                            .map(|v| match v {
                                Value::Struct(r) => Ok(Row { cells: r.cells }),
                                _ => Err(error("Impossible!"))
                            })
                            .collect::<JobResult<Vec<Row>>>()?;
                        Ok(Value::Rows(Rows {
                            types: r.clone(),
                            rows: row_list,
                        }))
                    } else {
                        Ok(Value::List(List::new(list_type.clone(), lst)))
                    }
                }
                _ => Ok(Value::List(List::new(ValueType::Any, lst))),
            }
        }
        serde_json::Value::Object(o) => {
            Ok(Value::Struct(
                Struct::new(
                    o
                        .iter()
                        .map(|(k, v)| (k.as_str(), convert_json(v)))
                        .map(|(k, v)| match v {
                            Ok(vv) => Ok((k, vv)),
                            Err(e) => Err(e)
                        })
                        .collect::<Result<Vec<(&str, Value)>, JobError>>()?)))
        }
    }
}

fn run(cfg: Config, output: ValueSender, printer: Printer) -> JobResult<()> {
    let mut reader = BufReader::new(cfg.input.reader);

    let v = to_job_error(serde_json::from_reader(reader))?;
    let crush_value = convert_json(&v)?;
    output.send(crush_value)?;
    return Ok(());
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    let cfg = parse(context.arguments, context.input)?;
    run(cfg, context.output, context.printer)
}
