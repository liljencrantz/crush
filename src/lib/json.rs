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
use std::io::{BufReader, Write};

use crate::lang::{r#struct::Struct, list::List, table::Table, binary::BinaryReader};
use crate::lang::errors::{CrushResult, to_crush_error, error, mandate};
use crate::lang::stream::{ValueSender, ValueReceiver};
use std::collections::HashSet;
use crate::lang::errors::Kind::InvalidData;
use crate::lang::table::ColumnType;
use crate::lang::printer::Printer;
use crate::lang::scope::Scope;
use std::fs::File;
use crate::lang::binary::binary_channel;
use std::convert::TryFrom;

fn reader(mut arguments: Vec<Argument>, input: ValueReceiver, printer: &Printer) -> CrushResult<Box<dyn BinaryReader>> {
    match arguments.len() {
        0 => match input.recv()? {
            Value::BinaryStream(b) => Ok(b),
            Value::Binary(b) => Ok(BinaryReader::vec(&b)),
            _ => argument_error("Expected either a file to read or binary pipe input"),
        },
        _ => Ok(BinaryReader::paths(arguments.files(printer)?)?),
    }
}

fn writer(mut arguments: Vec<Argument>, output: ValueSender, printer: &Printer) -> CrushResult<Box<dyn Write>> {
    match arguments.len() {
        0 => {
            let (w,r) = binary_channel();
            output.send(Value::BinaryStream(r))?;
            Ok(w)
        }
        1 => {
            let files = arguments.files(printer)?;
            if files.len() != 1 {
                return argument_error("Expected exactly one desitnation file");
            }
            Ok(Box::from(to_crush_error(File::create(files[0].clone()))?))
        },
        _ => argument_error("Too many arguments"),
    }
}

fn from_json(json_value: &serde_json::Value) -> CrushResult<Value> {
    match json_value {
        serde_json::Value::Null => Ok(Value::Empty()),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(f) => {
            if f.is_u64() {
                Ok(Value::Integer(f.as_u64().expect("") as i128))
            } else if f.is_i64() {
                Ok(Value::Integer(f.as_i64().expect("") as i128))
            } else {
                Ok(Value::Float(f.as_f64().ok_or(CrushError { kind: InvalidData, message: "Not a valid number".to_string() })?))
            }
        }
        serde_json::Value::String(s) => Ok(Value::string(s.as_str())),
        serde_json::Value::Array(arr) => {
            let mut lst = arr.iter()
                .map(|v| from_json(v))
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
        serde_json::Value::Object(o) => {
            Ok(Value::Struct(
                Struct::new(
                    o
                        .iter()
                        .map(|(k, v)| (Box::from(k.as_str()), from_json(v)))
                        .map(|(k, v)| match v {
                            Ok(vv) => Ok((k, vv)),
                            Err(e) => Err(e)
                        })
                        .collect::<Result<Vec<(Box<str>, Value)>, CrushError>>()?,
                None,
                )))
        }
    }
}

fn to_json(value: Value) -> CrushResult<serde_json::Value> {
    match value.materialize() {
        Value::File(s) =>
            Ok(serde_json::Value::from(mandate(s.to_str(),"Invalid filename")?)),

        Value::String(s) => Ok(serde_json::Value::from(s.as_ref())),

        Value::Integer(i) =>
            Ok(serde_json::Value::from(to_crush_error(i64::try_from(i))?)),

        Value::List(l) =>
            Ok(serde_json::Value::Array(
                l.dump().drain(..)
                    .map(to_json)
                    .collect::<CrushResult<Vec<_>>>()?)),

        Value::Table(t) => {
            let types = t.types().to_vec();
            let structs = t.rows()
                .iter()
                .map(|r| r.clone().into_struct(&types))
                .map(|s| to_json(Value::Struct(s)))
                .collect::<CrushResult<Vec<_>>>()?;
            Ok(serde_json::Value::Array(structs))
        }

        Value::Bool(b) => Ok(serde_json::Value::from(b)),

        Value::Float(f) => Ok(serde_json::Value::from(f)),

        Value::Struct(s) => {
            let mut map = serde_json::map::Map::new();
            for (k, v) in s.local_elements() {
                map.insert(k.to_string(), to_json(v)?);
            }
            Ok(serde_json::Value::Object(map))
        }

        Value::Duration(d) => Ok(serde_json::Value::from(d.num_seconds())),

        Value::Time(t) => Ok(serde_json::Value::from(t.to_rfc3339())),

        Value::Binary(b) => Ok(serde_json::Value::from(b)),

        Value::BinaryStream(_) => panic!("Impossible"),

        Value::TableStream(_) => panic!("Impossible"),

        v => error(format!(
            "Unsupported data type {}",
            v.value_type().to_string()).as_str()
        ),
    }
}

pub fn from(context: ExecutionContext) -> CrushResult<()> {
    let reader = BufReader::new(reader(context.arguments, context.input, &context.printer)?);
    let v = to_crush_error(serde_json::from_reader(reader))?;
    let crush_value = from_json(&v)?;
    context.output.send(crush_value)
}

fn to(context: ExecutionContext) -> CrushResult<()> {
    let mut writer = writer(context.arguments, context.output, &context.printer)?;
    let value = context.input.recv()?;
    let json_value = to_json(value)?;
    to_crush_error(writer.write(json_value.to_string().as_bytes()))?;
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("json")?;
    env.declare_command(
        "from",from, true,
        "json:from [file:file]", "Parse json", Some(
            r#"    Input can either be a binary stream or a file.

    Examples:

    json:from some_file.json

    (http "https://jsonplaceholder.typicode.com/todos/3"):body | json:from"#))?;

    env.declare_command(
        "to", to, true,
        "json:to [file:file]", "Serialize to json format", Some(
            r#"    If no file is specified, output is returned as a BinaryStream.

    Examples:

    ls | json:to"#))?;
    env.readonly();

    Ok(())
}

