use crate::lang::signature::binary_input::ToReader;
use crate::lang::state::contexts::CommandContext;
use crate::lang::{data::table::Row, value::Value, value::ValueType};
use std::io::{BufReader, Write};

use crate::lang::command::OutputType::Unknown;
use crate::lang::data::dict::Dict;
use crate::lang::data::table::ColumnType;
use crate::lang::errors::{CrushResult, error};
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::files;
use crate::lang::signature::files::Files;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{data::list::List, data::table::Table};
use signature::signature;
use std::collections::HashSet;
use std::convert::TryFrom;

fn from_yaml(yaml_value: &serde_yaml::Value) -> CrushResult<Value> {
    match yaml_value {
        serde_yaml::Value::Null => Ok(Value::Empty),
        serde_yaml::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_yaml::Value::Number(f) => {
            if f.is_u64() {
                Ok(Value::Integer(f.as_u64().expect("") as i128))
            } else if f.is_i64() {
                Ok(Value::Integer(f.as_i64().expect("") as i128))
            } else {
                Ok(Value::Float(f.as_f64().ok_or("Not a valid number")?))
            }
        }
        serde_yaml::Value::String(s) => Ok(Value::from(s.as_str())),
        serde_yaml::Value::Sequence(arr) => {
            let mut lst = arr
                .iter()
                .map(|v| from_yaml(v))
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
        serde_yaml::Value::Mapping(o) => {
            let d = Dict::new(ValueType::Any, ValueType::Any)?;
            for (k, v) in o.into_iter() {
                d.insert(from_yaml(k)?, from_yaml(v)?)?;
            }
            Ok(d.into())
        }

        serde_yaml::Value::Tagged(t) => from_yaml(&t.value),
    }
}

fn to_yaml(value: Value) -> CrushResult<serde_yaml::Value> {
    match value.materialize()? {
        Value::File(s) => Ok(serde_yaml::Value::from(
            s.to_str().ok_or("Invalid filename")?,
        )),

        Value::String(s) => Ok(serde_yaml::Value::from(s.to_string())),

        Value::Integer(i) => Ok(serde_yaml::Value::from(i64::try_from(i)?)),

        Value::List(l) => Ok(serde_yaml::Value::Sequence(
            l.iter().map(to_yaml).collect::<CrushResult<Vec<_>>>()?,
        )),

        Value::Table(t) => {
            let types = t.types().to_vec();
            let structs = t
                .iter()
                .map(|r| r.clone().into_struct(&types))
                .map(|s| to_yaml(Value::Struct(s)))
                .collect::<CrushResult<Vec<_>>>()?;
            Ok(serde_yaml::Value::Sequence(structs))
        }

        Value::Bool(b) => Ok(serde_yaml::Value::from(b)),

        Value::Float(f) => Ok(serde_yaml::Value::from(f)),

        Value::Struct(s) => {
            let mut map = serde_yaml::Mapping::new();
            for (k, v) in s.local_elements() {
                map.insert(to_yaml(Value::from(k))?, to_yaml(v)?);
            }
            Ok(serde_yaml::Value::Mapping(map))
        }

        Value::Duration(d) => Ok(serde_yaml::Value::from(d.num_seconds())),

        Value::Time(t) => Ok(serde_yaml::Value::from(t.to_rfc3339())),

        Value::Binary(b) => Ok(serde_yaml::Value::from(b.to_vec())),

        Value::BinaryInputStream(_) => panic!("Impossible"),

        Value::TableInputStream(_) => panic!("Impossible"),

        v => error(&format!("Unsupported data type {}", v.value_type())),
    }
}

#[signature(
    io.yaml.from,
    can_block = true,
    output = Unknown,
    short = "Parse yaml format",
    long = "When deserializing a list, `yaml:from` will try to infer the type of the list. If all of the elements of the list are of the same type, the list will be parametrized to the same type. If all elements are objects, and all objects have the same set of fields with the same types, the list will be turned into a table.",
    example = "(http \"https://jsonplaceholder.typicode.com/todos/3\"):body | yaml:from")]
struct FromSignature {
    #[unnamed()]
    files: Vec<BinaryInput>,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg: FromSignature =
        FromSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    let reader = BufReader::new(cfg.files.to_reader(context.input)?);
    let serde_value = serde_yaml::from_reader(reader)?;
    let crush_value = from_yaml(&serde_value)?;
    context.output.send(crush_value)
}

#[signature(
    io.yaml.to,
    can_block = true,
    output = Unknown,
    short = "Serialize to yaml format",
    long = "When serializing a list, some types have to be squashed, because yaml does not have all the same types that Crush does:",
    long = "* `time` values are turned into strings in the RFC 3339 format.",
    long = "* `duration` values are turned into the integer number of seconds in the duration.",
    example = "ls | yaml:to")]
struct To {
    #[unnamed()]
    file: Option<Files>,
}

fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut writer = files::writer(cfg.file, context.output)?;
    let value = context.input.recv()?;
    let yaml_value = to_yaml(value)?;
    writer.write(serde_yaml::to_string(&yaml_value)?.as_bytes())?;
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "yaml",
        "YAML I/O",
        Box::new(move |env| {
            FromSignature::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
