use crate::lang::serialization::*;
use crate::lang::value::Value;
use std::collections::{HashSet, HashMap};
use std::path::Path;
use std::fs::File;
use crate::lang::errors::{CrushResult, to_crush_error};
use std::io::{Write, Seek, Read, Cursor};
use prost::Message;

pub struct SerializationState {
    pub with_id: HashMap<u64, usize>,
    pub hashable: HashMap<Value, usize>,
}

pub struct DeserializationState {
    pub deserialized: HashMap<usize, Value>,
}

pub fn serialize(value: &Value, destination: &Path) -> CrushResult<()>{
    let mut res = SerializedValue::default();
    let mut state = SerializationState {
        with_id: HashMap::new(),
        hashable: HashMap::new(),
    };
    res.root = value.serialize(&mut res.nodes, &mut state)? as u64;

    let mut buf = Vec::new();
    buf.reserve(res.encoded_len());
    res.encode(&mut buf).unwrap();

    let mut file_buffer = to_crush_error(File::create(destination))?;
    let mut pos = 0;

    while pos < buf.len() {
        let bytes_written = to_crush_error(file_buffer.write(&buf[pos..]))?;
        pos += bytes_written;
    }
    Ok(())
}

pub fn deserialize(source: &Path) -> CrushResult<Value> {
    let mut buf = Vec::new();
    let mut file_buffer = to_crush_error(File::open(source))?;
    buf.reserve(to_crush_error(source.metadata())?.len() as usize);
    file_buffer.read_to_end(&mut buf);

    let mut state = DeserializationState {
        deserialized: HashMap::new(),
    };

    let mut res = SerializedValue::decode(&mut Cursor::new(buf)).unwrap();

    println!("AAA {:?}", res);

    Value::deserialize(res.root as usize, &res.nodes, &mut state)
}

pub trait Serializable {
    fn deserialize(id: usize, nodes: &Vec<SerializationNode>, state: &mut DeserializationState) -> CrushResult<Value>;
    fn serialize(&self, nodes: &mut Vec<SerializationNode>, state: &mut SerializationState) -> CrushResult<usize>;
}
