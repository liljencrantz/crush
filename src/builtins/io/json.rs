use crate::lang::errors::CrushError;
use crate::lang::state::contexts::CommandContext;
use crate::lang::{data::table::Row, value::Value, value::ValueType};
use std::io::{BufReader, Write};

use crate::lang::command::OutputType::Unknown;
use crate::lang::data::table::ColumnType;
use crate::lang::errors::{CrushResult, error};
use crate::lang::signature::files::Files;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{data::list::List, data::r#struct::Struct, data::table::Table};
use signature::signature;
use std::collections::HashSet;
use std::convert::{From, TryFrom};

fn from_json(json_value: &serde_json::Value) -> CrushResult<Value> {
    match json_value {
        serde_json::Value::Null => Ok(Value::Empty),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(f) => {
            if f.is_u64() {
                Ok(Value::Integer(f.as_u64().expect("") as i128))
            } else if f.is_i64() {
                Ok(Value::Integer(f.as_i64().expect("") as i128))
            } else {
                Ok(Value::Float(f.as_f64().ok_or("`json:from`: Not a valid number")?))
            }
        }
        serde_json::Value::String(s) => Ok(Value::from(s.as_str())),
        serde_json::Value::Array(arr) => {
            let mut lst = arr
                .iter()
                .map(|v| from_json(v))
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
                0 => Ok(Value::Empty),
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
                            Ok(Value::Table(Table::from((
                                struct_types.iter().next().unwrap().clone(),
                                row_list,
                            ))))
                        }
                        _ => Ok(List::new(list_type.clone(), lst).into()),
                    }
                }
                _ => Ok(List::new(ValueType::Any, lst).into()),
            }
        }
        serde_json::Value::Object(o) => Ok(Value::Struct(Struct::new(
            o.iter()
                .map(|(k, v)| (k.to_string(), from_json(v)))
                .map(|(k, v)| match v {
                    Ok(vv) => Ok((k, vv)),
                    Err(e) => Err(e),
                })
                .collect::<Result<Vec<(String, Value)>, CrushError>>()?,
            None,
        ))),
    }
}

fn to_json(value: Value) -> CrushResult<serde_json::Value> {
    let v = value.materialize()?;
    match v {
        Value::File(s) => Ok(serde_json::Value::from(
            s.to_str().ok_or("`json:to`: Invalid filename")?,
        )),

        Value::String(s) => Ok(serde_json::Value::from(s.to_string())),

        Value::Integer(i) => Ok(serde_json::Value::from(i64::try_from(i)?)),

        Value::List(l) => Ok(serde_json::Value::Array(
            l.iter().map(to_json).collect::<CrushResult<Vec<_>>>()?,
        )),

        Value::Table(t) => {
            let types = t.types().to_vec();
            let structs: CrushResult<Vec<serde_json::Value>> = t
                .iter()
                .map(|r| to_json(Value::from(r.clone().into_struct(&types))))
                .collect();
            Ok(serde_json::Value::Array(structs?))
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

        Value::Binary(b) => Ok(serde_json::Value::from(b.to_vec())),

        Value::BinaryInputStream(_) | Value::TableInputStream(_) => panic!("Impossible"),

        v => error(&format!("`json:to`: Unsupported data type {}", v.value_type())),
    }
}

pub fn json_to_value(s: &str) -> CrushResult<Value> {
    let serde_value = serde_json::from_str(s)?;
    from_json(&serde_value)
}

pub fn value_to_json(v: Value) -> CrushResult<String> {
    let json_value = to_json(v)?;
    Ok(json_value.to_string())
}

#[signature(
    io.json.from,
    can_block = true,
    output = Unknown,
    short = "Parse json format",
    long = "When deserializing a list, `json:from` will try to infer the type of the list. If all of the elements of the list are of the same type, the list will be parametrized to the same type. If all elements are objects, and all objects have the same set of fields with the same types, the list will be turned into a table.",
    example = "http \"https://jsonplaceholder.typicode.com/todos/3\"| member body | json:from")]
struct FromSignature {
    #[unnamed()]
    files: Files,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg: FromSignature =
        FromSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    let reader = BufReader::new(cfg.files.reader(context.input)?);
    let serde_value = serde_json::from_reader(reader)?;
    let crush_value = from_json(&serde_value)?;
    context.output.send(crush_value)
}

#[signature(
    io.json.to,
    can_block = true,
    output = Unknown,
    short = "Serialize to json format",
    long = "When serializing a list, some types have to be squashed, because json does not have all the same types that Crush does:",
    long = "* `time` values are turned into strings in the RFC 3339 format.",
    long = "* `duration` values are turned into the integer number of seconds in the duration.",
    example = "files | json:to")]
struct To {
    #[unnamed()]
    file: Files,
    #[description("Disable line breaking and indentation.")]
    #[default(false)]
    compact: bool,
}

fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut writer = cfg.file.writer(context.output)?;
    let value = context.input.recv()?;
    let json_value = to_json(value)?;
    writer.write(
        if cfg.compact {
            json_value.to_string()
        } else {
            serde_json::to_string_pretty(&json_value)?
        }
        .as_bytes(),
    )?;
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "json",
        "JSON I/O",
        Box::new(move |env| {
            FromSignature::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
