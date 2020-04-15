use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::{
    lang::{
        argument::Argument,
        table::Row,
        value::ValueType,
        value::Value,
    },
    lang::errors::{CrushError, argument_error},
};
use std::io::{BufReader, Read};

use crate::lang::{r#struct::Struct, list::List, table::Table, binary::BinaryReader};
use crate::lang::errors::{CrushResult, to_crush_error, error};
use crate::lang::stream::{ValueSender, ValueReceiver};
use std::collections::HashSet;
use crate::lang::errors::Kind::InvalidData;
use crate::lang::table::ColumnType;
use crate::lang::printer::Printer;

fn parse(mut arguments: Vec<Argument>, input: ValueReceiver, printer: &Printer) -> CrushResult<Box<dyn BinaryReader>> {
     match arguments.len() {
        0 => match input.recv()? {
            Value::BinaryStream(b) => Ok(b),
            Value::Binary(b) => Ok(BinaryReader::vec(&b)),
            _ => argument_error("Expected either a file to read or binary pipe input"),
        },
        _ => Ok(BinaryReader::paths(arguments.files(printer)?)?),
    }
}

fn convert_toml(toml_value: &toml::Value) -> CrushResult<Value> {
    match toml_value {
        toml::Value::Boolean(b) => Ok(Value::Bool(*b)),
        toml::Value::Float(f) => {
                Ok(Value::Float(*f))
        }
        toml::Value::Integer(f) => {
                Ok(Value::Integer(*f as i128))
        }
        toml::Value::String(s) => Ok(Value::string(s.as_str())),
        toml::Value::Array(arr) => {
            let mut lst = arr.iter()
                .map(|v| convert_toml(v))
                .collect::<CrushResult<Vec<Value>>>()?;
            let types: HashSet<ValueType> = lst.iter().map(|v| v.value_type()).collect();
            let struct_types: HashSet<Vec<ColumnType>> =
                lst.iter()
                    .flat_map(|v| match v {
                        Value::Struct(r) => vec![r.local_signature()],
                        _ => vec![]
                    })
                    .collect();

            match types.len() {
                0 => Ok(Value::Empty()),
                1 => {
                    let list_type = types.iter().next().unwrap();
                    match (list_type, struct_types.len()) {
                        (ValueType::Struct, 1) => {
                            let row_list = lst
                                .drain(..)
                                .map(|v| match v {
                                    Value::Struct(r) => Ok(r.into_row()),
                                    _ => error("Impossible!")
                                })
                                .collect::<CrushResult<Vec<Row>>>()?;
                            Ok(Value::Table(Table::new(struct_types.iter().next().unwrap().clone(), row_list)))
                        }
                        _ => Ok(Value::List(List::new(list_type.clone(), lst)))

                    }
                }
                _ => Ok(Value::List(List::new(ValueType::Any, lst))),
            }
        }
        toml::Value::Table(t) => {
            Ok(Value::Struct(
                Struct::new(
                    t
                        .iter()
                        .map(|(k, v)| (Box::from(k.as_str()), convert_toml(v)))
                        .map(|(k, v)| match v {
                            Ok(vv) => Ok((k, vv)),
                            Err(e) => Err(e)
                        })
                        .collect::<Result<Vec<(Box<str>, Value)>, CrushError>>()?,
                None,
                )))
        }
        toml::Value::Datetime(_) => {
            unimplemented!();
        }
    }
}

fn run(r: Box<dyn BinaryReader>, output: ValueSender) -> CrushResult<()> {
    let mut reader = BufReader::new(r);
    let mut v = Vec::new();
    
    reader.read_to_end(&mut v);
    
    let v = to_crush_error(toml::from_slice(&v))?;
    let crush_value = convert_toml(&v)?;
    output.send(crush_value)?;
    Ok(())
}

pub fn from(context: ExecutionContext) -> CrushResult<()> {
    let reader = parse(context.arguments, context.input, &context.printer)?;
    run(reader, context.output)
}
