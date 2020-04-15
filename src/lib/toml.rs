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
use std::io::{BufReader, Read, Write};

use crate::lang::{r#struct::Struct, list::List, table::Table, binary::BinaryReader};
use crate::lang::errors::{CrushResult, to_crush_error, error, mandate};
use crate::lang::stream::{ValueSender, ValueReceiver};
use std::collections::HashSet;
use crate::lang::table::ColumnType;
use crate::lang::printer::Printer;
use crate::lang::binary::binary_channel;
use std::fs::File;
use std::convert::TryFrom;
use crate::lang::scope::Scope;

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

fn from_toml(toml_value: &toml::Value) -> CrushResult<Value> {
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
                .map(|v| from_toml(v))
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
                        .map(|(k, v)| (Box::from(k.as_str()), from_toml(v)))
                        .map(|(k, v)| match v {
                            Ok(vv) => Ok((k, vv)),
                            Err(e) => Err(e)
                        })
                        .collect::<Result<Vec<(Box<str>, Value)>, CrushError>>()?,
                    None,
                )))
        }
        toml::Value::Datetime(_) => {
            unimplemented!("Dates in the TOML library are currently opaque blobs, dates are unsupported");
        }
    }
}

fn from(context: ExecutionContext) -> CrushResult<()> {
    let mut reader = BufReader::new(reader(context.arguments, context.input, &context.printer)?);
    let mut v = Vec::new();

    to_crush_error(reader.read_to_end(&mut v))?;

    let v = to_crush_error(toml::from_slice(&v))?;
    let crush_value = from_toml(&v)?;
    context.output.send(crush_value)?;
    Ok(())
}

fn to_toml(value: Value) -> CrushResult<toml::Value> {
    match value.materialize() {
        Value::File(s) =>
            Ok(toml::Value::from(mandate(s.to_str(),"Invalid filename")?)),

        Value::String(s) => Ok(toml::Value::from(s.as_ref())),

        Value::Integer(i) =>
            Ok(toml::Value::from(to_crush_error(i64::try_from(i))?)),

        Value::List(l) =>
            Ok(toml::Value::Array(
                l.dump().drain(..)
                    .map(|e| to_toml(e))
                    .collect::<CrushResult<Vec<_>>>()?)),

        Value::Table(t) => {
            let types = t.types().clone();
            let structs = t.rows()
                .iter()
                .map(|r| r.clone().into_struct(&types))
                .map(|s| to_toml(Value::Struct(s)))
                .collect::<CrushResult<Vec<_>>>()?;
            Ok(toml::Value::Array(structs))
        }

        Value::Bool(b) => Ok(toml::Value::from(b)),

        Value::Float(f) => Ok(toml::Value::from(f)),

        Value::Struct(s) => {
            let mut map = toml::map::Map::new();
            for (k, v) in s.local_elements() {
                map.insert(k.to_string(), to_toml(v)?);
            }
            Ok(toml::Value::Table(map))
        }

        Value::Duration(d) => Ok(toml::Value::from(d.num_seconds())),

        Value::Time(t) => Ok(toml::Value::from(t.to_rfc3339())),

        Value::Binary(b) => Ok(toml::Value::from(b)),

        Value::BinaryStream(_) => panic!("Impossible"),

        Value::TableStream(_) => panic!("Impossible"),

        v => error(format!(
            "Unsupported data type {}",
            v.value_type().to_string()).as_str()
        ),
    }
}

fn to(context: ExecutionContext) -> CrushResult<()> {
    let mut writer = writer(context.arguments, context.output, &context.printer)?;
    let value = context.input.recv()?;
    let toml_value = to_toml(value)?;
    to_crush_error(writer.write(toml_value.to_string().as_bytes()))?;
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("toml")?;
    env.declare_command(
        "from", from, true,
        "toml:from [file:file]", "Parse toml format", Some(
            r#"    Input can either be a binary stream or a file. All Toml types except
    datetime are supported. Datetime is not suported because the rust toml
    currently doesn't support accessing the internal state of a datetime.

    Examples:

    toml:from Cargo.toml"#))?;

    env.declare_command(
        "to", to, true,
        "toml:to [file:file]", "Serialize to toml format", Some(
            r#"    If no file is specified, output is returned as a BinaryStream.
    The following Crush types are supported: File, string, integer, float, bool, list, table,
    table_stream, struct, time, duration, binary and binary_stream.

    Examples:

    ls | toml:to"#))?;
    env.readonly();

    Ok(())
}
