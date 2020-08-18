use crate::lang::execution_context::CommandContext;
use crate::{
    lang::errors::CrushError,
    lang::{table::Row, value::Value, value::ValueType},
};
use std::io::{BufReader, Read, Write};

use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{error, mandate, to_crush_error, CrushResult};
use crate::lang::files::Files;
use crate::lang::scope::ScopeLoader;
use crate::lang::table::ColumnType;
use crate::lang::{list::List, r#struct::Struct, table::Table};
use signature::signature;
use std::collections::HashSet;
use std::convert::TryFrom;

fn from_toml(toml_value: &toml::Value) -> CrushResult<Value> {
    match toml_value {
        toml::Value::Boolean(b) => Ok(Value::Bool(*b)),
        toml::Value::Float(f) => Ok(Value::Float(*f)),
        toml::Value::Integer(f) => Ok(Value::Integer(*f as i128)),
        toml::Value::String(s) => Ok(Value::string(s.as_str())),
        toml::Value::Array(arr) => {
            let mut lst = arr
                .iter()
                .map(|v| from_toml(v))
                .collect::<CrushResult<Vec<Value>>>()?;
            let types: HashSet<ValueType> = lst.iter().map(|v| v.value_type()).collect();
            let struct_types: HashSet<Vec<ColumnType>> = lst
                .iter()
                .flat_map(|v| match v {
                    Value::Struct(r) => vec![r.local_signature()],
                    _ => vec![],
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
                                    Value::Struct(r) => Ok(r.to_row()),
                                    _ => error("Impossible!"),
                                })
                                .collect::<CrushResult<Vec<Row>>>()?;
                            Ok(Value::Table(Table::new(
                                struct_types.iter().next().unwrap().clone(),
                                row_list,
                            )))
                        }
                        _ => Ok(Value::List(List::new(list_type.clone(), lst))),
                    }
                }
                _ => Ok(Value::List(List::new(ValueType::Any, lst))),
            }
        }
        toml::Value::Table(t) => Ok(Value::Struct(Struct::new(
            t.iter()
                .map(|(k, v)| (k, from_toml(v)))
                .map(|(k, v)| match v {
                    Ok(vv) => Ok((k.to_string(), vv)),
                    Err(e) => Err(e),
                })
                .collect::<Result<Vec<(String, Value)>, CrushError>>()?,
            None,
        ))),
        toml::Value::Datetime(_) => {
            unimplemented!(
                "Dates in the TOML library are currently opaque blobs, dates are unsupported"
            );
        }
    }
}

#[signature(
from,
can_block = true,
output = Unknown,
short = "Parse toml format",
long = "Input can either be a binary stream or a file. All Toml types except\n    datetime are supported. Datetime is not supported because the rust toml\n    currently doesn't support accessing the internal state of a datetime.",
example = "toml:from Cargo.toml")]
struct From {
    #[unnamed()]
    files: Files,
}

fn from(context: CommandContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.arguments, &context.printer)?;
    let mut reader = BufReader::new(cfg.files.reader(context.input)?);
    let mut v = Vec::new();

    to_crush_error(reader.read_to_end(&mut v))?;

    let v = to_crush_error(toml::from_slice(&v))?;
    let crush_value = from_toml(&v)?;
    context.output.send(crush_value)?;
    Ok(())
}

fn to_toml(value: Value) -> CrushResult<toml::Value> {
    match value.materialize() {
        Value::File(s) => Ok(toml::Value::from(mandate(s.to_str(), "Invalid filename")?)),

        Value::String(s) => Ok(toml::Value::from(s.as_ref())),

        Value::Integer(i) => Ok(toml::Value::from(to_crush_error(i64::try_from(i))?)),

        Value::List(l) => Ok(toml::Value::Array(
            l.dump()
                .drain(..)
                .map(to_toml)
                .collect::<CrushResult<Vec<_>>>()?,
        )),

        Value::Table(t) => {
            let types = t.types().to_vec();
            let structs = t
                .rows()
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

        v => error(format!("Unsupported data type {}", v.value_type().to_string()).as_str()),
    }
}

#[signature(
to,
can_block = true,
output = Unknown,
short = "Serialize to toml format",
long = "If no file is specified, output is returned as a BinaryStream.\n    The following Crush types are supported: File, string, integer, float, bool, list, table,\n    table_stream, struct, time, duration, binary and binary_stream.",
example = "ls | toml:to")]
struct To {
    #[unnamed()]
    file: Files,
}

fn to(context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.arguments, &context.printer)?;
    let mut writer = cfg.file.writer(context.output)?;
    let serde_value = context.input.recv()?;
    let toml_value = to_toml(serde_value)?;
    to_crush_error(writer.write(toml_value.to_string().as_bytes()))?;
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_lazy_namespace(
        "toml",
        Box::new(move |env| {
            From::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
