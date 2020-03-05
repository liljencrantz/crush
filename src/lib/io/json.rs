use crate::lang::command::ExecutionContext;
use crate::{
    lang::{
        argument::Argument,
        table::Row,
        value::ValueType,
        value::Value,
    },
    lang::errors::{CrushError, argument_error},
};
use std::io::BufReader;

use crate::lang::printer::Printer;
use crate::lang::{r#struct::Struct, list::List, table::Table, binary::BinaryReader};
use crate::lang::errors::{CrushResult, to_crush_error, error};
use crate::lang::stream::{ValueSender, ValueReceiver};
use std::collections::HashSet;
use crate::lib::parse_util::argument_files;
use crate::lang::errors::Kind::INVALID_DATA;

pub struct Config {
    input: Box<dyn BinaryReader>,
}

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> CrushResult<Config> {
    let reader = match arguments.len() {
        0 => match input.recv()? {
            Value::BinaryStream(b) => Ok(b),
            Value::Binary(b) => Ok(BinaryReader::vec(&b)),
            _ => argument_error("Expected either a file to read or binary pipe input"),
        },
        _ => BinaryReader::paths(argument_files(arguments)?),
    };
    Ok(Config {
        input: reader?,
    })
}

fn convert_json(json_value: &serde_json::Value) -> CrushResult<Value> {
    match json_value {
        serde_json::Value::Null => Ok(Value::Empty()),
        serde_json::Value::Bool(b) => Ok(Value::Bool(b.clone())),
        serde_json::Value::Number(f) => {
            if f.is_u64() {
                Ok(Value::Integer(f.as_u64().expect("") as i128))
            } else if f.is_i64() {
                Ok(Value::Integer(f.as_i64().expect("") as i128))
            } else {
                Ok(Value::Float(f.as_f64().ok_or(CrushError { kind: INVALID_DATA, message: "Not a valid number".to_string() })?))
            }
        }
        serde_json::Value::String(s) => Ok(Value::Text(Box::from(s.clone()))),
        serde_json::Value::Array(arr) => {
            let mut lst = arr.iter()
                .map(|v| convert_json(v))
                .collect::<CrushResult<Vec<Value>>>()?;
            let types: HashSet<ValueType> = lst.iter().map(|v| v.value_type()).collect();
            match types.len() {
                0 => Ok(Value::Empty()),
                1 => {
                    let list_type = types.iter().next().unwrap();
                    if let ValueType::Struct(r) = list_type {
                        let row_list = lst
                            .drain(..)
                            .map(|v| match v {
                                Value::Struct(r) => Ok(r.into_row()),
                                _ => error("Impossible!")
                            })
                            .collect::<CrushResult<Vec<Row>>>()?;
                        Ok(Value::Table(Table::new(r.clone(), row_list)))
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
                        .map(|(k, v)| (Box::from(k.as_str()), convert_json(v)))
                        .map(|(k, v)| match v {
                            Ok(vv) => Ok((k, vv)),
                            Err(e) => Err(e)
                        })
                        .collect::<Result<Vec<(Box<str>, Value)>, CrushError>>()?)))
        }
    }
}

fn run(cfg: Config, output: ValueSender, printer: Printer) -> CrushResult<()> {
    let mut reader = BufReader::new(cfg.input);
    let v = to_crush_error(serde_json::from_reader(reader))?;
    let crush_value = convert_json(&v)?;
    output.send(crush_value)?;
    Ok(())
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let cfg = parse(context.arguments, context.input)?;
    run(cfg, context.output, context.printer)
}
