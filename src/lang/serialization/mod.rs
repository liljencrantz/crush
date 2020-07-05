use crate::lang::value::{Value, ValueType};
use std::collections::HashMap;
use std::path::Path;
use std::fs::File;
use crate::lang::errors::{CrushResult, to_crush_error};
use std::io::{Write, Read, Cursor};
use prost::Message;
use model::SerializedValue;
use model::Element;
use crate::lang::list::List;
use crate::lang::r#struct::Struct;
use crate::lang::dict::Dict;
use crate::lang::scope::Scope;

mod scope_serializer;
mod struct_serializer;
mod integer_serializer;
mod string_serializer;
mod list_serializer;
mod dict_serializer;
mod value_type_serializer;
mod value_serializer;
mod table_serializer;

//pub mod model;
pub mod model {
    include!(concat!(env!("OUT_DIR"), "/model.rs"));
}

pub struct SerializationState {
    pub with_id: HashMap<u64, usize>,
    pub values: HashMap<Value, usize>,
}

pub struct DeserializationState {
    pub env: Scope,
    pub values: HashMap<usize, Value>,
    pub lists: HashMap<usize, List>,
    pub types: HashMap<usize, ValueType>,
    pub dicts: HashMap<usize, Dict>,
    pub structs: HashMap<usize, Struct>,
    pub scopes: HashMap<usize, Scope>,
}

pub fn serialize(value: &Value, buf: &mut Vec<u8>) -> CrushResult<()> {
    let mut res = SerializedValue::default();
    let mut state = SerializationState {
        with_id: HashMap::new(),
        values: HashMap::new(),
    };
    res.root = value.clone().materialize().serialize(&mut res.elements, &mut state)? as u64;

    buf.reserve(res.encoded_len());
    res.encode(buf).unwrap();
    Ok(())
}

pub fn serialize_file(value: &Value, destination: &Path) -> CrushResult<()> {
    let mut buf = Vec::new();
    serialize(value, &mut buf)?;
    let mut file_buffer = to_crush_error(File::create(destination))?;
    let mut pos = 0;

    while pos < buf.len() {
        let bytes_written = to_crush_error(file_buffer.write(&buf[pos..]))?;
        pos += bytes_written;
    }
    Ok(())
}

pub fn deserialize_file(source: &Path, env: &Scope) -> CrushResult<Value> {
    let mut buf = Vec::new();
    let mut file_buffer = to_crush_error(File::open(source))?;
    buf.reserve(to_crush_error(source.metadata())?.len() as usize);
    to_crush_error(file_buffer.read_to_end(&mut buf))?;
    deserialize(&buf, env)
}

pub fn deserialize(buf: &Vec<u8>, env: &Scope) -> CrushResult<Value> {
    let mut state = DeserializationState {
        values: HashMap::new(),
        types: HashMap::new(),
        lists: HashMap::new(),
        dicts: HashMap::new(),
        structs: HashMap::new(),
        scopes: HashMap::new(),
        env: env.clone(),
    };

    let res = SerializedValue::decode(&mut Cursor::new(buf)).unwrap();

//    println!("AAA {:?}", res);

    Ok(Value::deserialize(res.root as usize, &res.elements, &mut state)?)
}

pub trait Serializable<T> {
    fn deserialize(id: usize, elements: &[Element], state: &mut DeserializationState) -> CrushResult<T>;
    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize>;
}
