use crate::builtins::grpc::codec::DynamicMessageCodec;
use crate::lang::data::dict::Dict;
use crate::lang::data::list::List;
use crate::lang::data::r#struct::Struct;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::CrushErrorType::GenericError;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::pipe::{Stream, ValueSender};
use crate::lang::printer::Printer;
use crate::lang::value::{Value, ValueType};
use bytes::Bytes;
use chrono::Duration;
use num_format::Locale::fi;
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use prost::Message;
use prost_reflect::{
    DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MapKey, MessageDescriptor,
    MethodDescriptor,
};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicI32, Ordering};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;
use tonic::transport::{Channel, Endpoint};
use tonic_reflection::pb::v1::server_reflection_client::ServerReflectionClient;
use tonic_reflection::pb::v1::{
    ServerReflectionRequest, ServiceResponse, server_reflection_request, server_reflection_response,
};

static CONNECTIONS: LazyLock<RwLock<HashMap<i32, Option<GrpcClient>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static NEXT_ID: AtomicI32 = AtomicI32::new(1);

pub struct GrpcClient {
    channel: Channel,
    host: String,
    timeout: Duration,
}

async fn connect_channel(
    host: &str,
    port: i128,
    timeout: Duration,
    plaintext: bool,
) -> CrushResult<Channel> {
    let uri = format!(
        "{}://{}:{}",
        if plaintext { "http" } else { "https" },
        host,
        port
    );

    let mut endpoint = Endpoint::from_shared(uri)?.timeout(
        timeout
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(5)),
    );

    if !plaintext {
        endpoint = endpoint.tls_config(tonic::transport::ClientTlsConfig::new())?;
    }

    Ok(endpoint.connect().await?)
}

impl GrpcClient {
    pub fn connections() -> &'static RwLock<HashMap<i32, Option<GrpcClient>>> {
        &CONNECTIONS
    }

    pub fn register(client: GrpcClient) -> i32 {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let mut map = Self::connections().write();
        map.insert(id, Some(client));
        id
    }

    pub fn close(id: i32) {
        let mut map = Self::connections().write();
        if let Some(opt) = map.get_mut(&id) {
            *opt = None;
        }
    }

    pub fn get(s: &Struct) -> CrushResult<MappedRwLockReadGuard<'static, GrpcClient>> {
        match s.get("id") {
            Some(Value::Integer(id)) => {
                let id = id as i32;
                Self::get_from_id(id)
            }

            _ => command_error("Missing or invalid id field in grpc method object"),
        }
    }

    pub fn get_from_id(id: i32) -> CrushResult<MappedRwLockReadGuard<'static, GrpcClient>> {
        let res = RwLockReadGuard::try_map(Self::connections().read(), |map| match map.get(&id) {
            Some(Some(opt)) => Some(opt),
            _ => None,
        });
        match res {
            Ok(client) => Ok(client),
            _ => command_error(format!(
                "Unknown gRPC connection id {}. Did you close this connection?",
                id
            )),
        }
    }

    pub async fn create(
        host: &str,
        plaintext: bool,
        timeout: Duration,
        port: i128,
    ) -> CrushResult<i32> {
        let client = GrpcClient {
            channel: connect_channel(host.as_ref(), port, timeout, plaintext).await?,
            host: host.to_string(),
            timeout,
        };
        Ok(Self::register(client))
    }

    async fn reflection_request(
        &self,
        message_request: server_reflection_request::MessageRequest,
    ) -> CrushResult<server_reflection_response::MessageResponse> {
        let mut client = ServerReflectionClient::new(self.channel.clone());
        let req = ServerReflectionRequest {
            host: self.host.clone(),
            message_request: Some(message_request),
        };

        let response = client
            .server_reflection_info(tokio_stream::once(req))
            .await?;

        let mut stream = response.into_inner();
        let msg = stream
            .message()
            .await?
            .ok_or_else(|| GenericError("Empty reflection response".to_string()))?;

        Ok(msg.message_response.ok_or_else(|| {
            GenericError("Missing message_response in reflection response".to_string())
        })?)
    }

    pub async fn list_services(&self) -> CrushResult<Vec<ServiceResponse>> {
        let resp = self
            .reflection_request(server_reflection_request::MessageRequest::ListServices(
                String::new(),
            ))
            .await?;

        match resp {
            server_reflection_response::MessageResponse::ListServicesResponse(list) => {
                Ok(list.service)
            }
            _ => command_error("Unexpected reflection response type"),
        }
    }

    async fn get_descriptor_pool(&self, symbol: &str) -> CrushResult<DescriptorPool> {
        let resp = self
            .reflection_request(
                server_reflection_request::MessageRequest::FileContainingSymbol(symbol.to_string()),
            )
            .await?;

        match resp {
            server_reflection_response::MessageResponse::FileDescriptorResponse(fdr) => {
                let mut fds = prost_types::FileDescriptorSet { file: Vec::new() };
                for fd_bytes in &fdr.file_descriptor_proto {
                    let fd = prost_types::FileDescriptorProto::decode(&fd_bytes[..])?;
                    fds.file.push(fd);
                }
                Ok(DescriptorPool::from_file_descriptor_set(fds)?)
            }
            _ => command_error("Unexpected reflection response type"),
        }
    }

    pub async fn list_methods(&self, service: &str) -> CrushResult<String> {
        let pool = self.get_descriptor_pool(service).await?;

        let svc_desc = pool
            .get_service_by_name(service)
            .ok_or_else(|| GenericError(format!("Service {} not found in descriptors", service)))?;

        let methods: Vec<String> = svc_desc
            .methods()
            .map(|m| format!("{}.{}", service, m.name()))
            .collect();

        Ok(methods.join("\n"))
    }

    pub async fn describe_method(
        &self,
        service: &str,
        method: &str,
    ) -> CrushResult<MethodDescriptor> {
        let pool = self.get_descriptor_pool(service).await?;
        if let Some(sd) = pool.get_service_by_name(service) {
            for md in sd.methods() {
                if md.name() == method {
                    return Ok(md);
                }
            }
        }
        command_error(format!("Method {}.{} not found", service, method))
    }

    fn convert_crush_value_to_protobuf_value(
        message: &mut DynamicMessage,
        descriptor: &FieldDescriptor,
        value: &Value,
    ) -> CrushResult<()> {
        if descriptor.is_list() {
            match value {
                Value::List(list) => match (descriptor.kind(), list.element_type()) {
                    (_, ValueType::Empty) => Ok(()),
                    (Kind::String, ValueType::String | ValueType::File) => {
                        message.set_field(
                            descriptor,
                            prost_reflect::Value::List(
                                list.iter()
                                    .map(|v| prost_reflect::Value::String(v.to_string()))
                                    .collect(),
                            ),
                        );
                        Ok(())
                    }

                    (Kind::Bytes, ValueType::Binary) => {
                        message.set_field(
                            descriptor,
                            prost_reflect::Value::List(
                                list.iter()
                                    .map(|v| match v {
                                        Value::Binary(b) => Ok(prost_reflect::Value::Bytes(
                                            Bytes::copy_from_slice(b.as_ref()),
                                        )),
                                        _ => command_error("Expected a binary value"),
                                    })
                                    .collect::<CrushResult<Vec<_>>>()?,
                            ),
                        );
                        Ok(())
                    }

                    (Kind::Double, ValueType::Float) => {
                        message.set_field(
                            descriptor,
                            prost_reflect::Value::List(
                                list.iter()
                                    .map(|v| match v {
                                        Value::Float(f) => Ok(prost_reflect::Value::F64(f)),
                                        _ => command_error("Expected a floating point value"),
                                    })
                                    .collect::<CrushResult<Vec<_>>>()?,
                            ),
                        );
                        Ok(())
                    }

                    (Kind::Float, ValueType::Float) => {
                        message.set_field(
                            descriptor,
                            prost_reflect::Value::List(
                                list.iter()
                                    .map(|v| match v {
                                        Value::Float(f) => Ok(prost_reflect::Value::F32(f as f32)),
                                        _ => command_error("Expected a floating point value"),
                                    })
                                    .collect::<CrushResult<Vec<_>>>()?,
                            ),
                        );
                        Ok(())
                    }

                    (Kind::Bool, ValueType::Bool) => {
                        message.set_field(
                            descriptor,
                            prost_reflect::Value::List(
                                list.iter()
                                    .map(|v| match v {
                                        Value::Bool(b) => Ok(prost_reflect::Value::Bool(b)),
                                        _ => command_error("Expected a floating point value"),
                                    })
                                    .collect::<CrushResult<Vec<_>>>()?,
                            ),
                        );
                        Ok(())
                    }

                    (expected, actual) => command_error(format!(
                        "Unexpected type of column {}. Expected {}, got {}.",
                        descriptor.name(),
                        crush_type(expected).to_string(),
                        actual.to_string()
                    )),
                },
                _ => command_error(format!(
                    "Unexpected type of column `{}`. Expected `list {}`, got `{}`.",
                    descriptor.name(),
                    crush_type(descriptor.kind()).to_string(),
                    value.value_type().to_string()
                )),
            }
        } else {
            match (descriptor.kind(), value) {
                (_, Value::Empty) => Ok(()),
                (Kind::String, Value::String(s)) => {
                    message.set_field(descriptor, prost_reflect::Value::String(s.to_string()));
                    Ok(())
                }
                (Kind::String, Value::File(s)) => {
                    message.set_field(
                        descriptor,
                        prost_reflect::Value::String(
                            s.to_str().unwrap_or("<invalid filename>").to_string(),
                        ),
                    );
                    Ok(())
                }

                (Kind::Bytes, Value::Binary(s)) => {
                    message.set_field(
                        descriptor,
                        prost_reflect::Value::Bytes(Bytes::copy_from_slice(s)),
                    );
                    Ok(())
                }

                (Kind::Double, Value::Float(f)) => {
                    message.set_field(descriptor, prost_reflect::Value::F64(*f));
                    Ok(())
                }
                (Kind::Float, Value::Float(f)) => {
                    message.set_field(descriptor, prost_reflect::Value::F32(*f as f32));
                    Ok(())
                }
                (Kind::Bool, Value::Bool(b)) => {
                    message.set_field(descriptor, prost_reflect::Value::Bool(*b));
                    Ok(())
                }

                (Kind::Int32 | Kind::Sint32 | Kind::Sfixed32, Value::Integer(i)) => {
                    let val = i32::try_from(*i)?;
                    message.set_field(descriptor, prost_reflect::Value::I32(val));
                    Ok(())
                }

                (Kind::Int64 | Kind::Sint64 | Kind::Sfixed64, Value::Integer(i)) => {
                    let val = i64::try_from(*i)?;
                    message.set_field(descriptor, prost_reflect::Value::I64(val));
                    Ok(())
                }

                (Kind::Uint32 | Kind::Fixed32, Value::Integer(i)) => {
                    let val = u32::try_from(*i)?;
                    message.set_field(descriptor, prost_reflect::Value::U32(val));
                    Ok(())
                }

                (Kind::Uint64 | Kind::Fixed64, Value::Integer(i)) => {
                    let val = u64::try_from(*i)?;
                    message.set_field(descriptor, prost_reflect::Value::U64(val));
                    Ok(())
                }

                (Kind::Enum(_), Value::Integer(i)) => {
                    let val = i32::try_from(*i)?;
                    message.set_field(descriptor, prost_reflect::Value::EnumNumber(val));
                    Ok(())
                }

                (Kind::Message(child_message_descriptor), Value::Struct(i)) => {
                    let mut sub_message = DynamicMessage::new(child_message_descriptor.clone());
                    for column in i.keys() {
                        let field = child_message_descriptor
                            .get_field_by_name(&column)
                            .ok_or("Unknown field")?;
                        let vv = i.get(&column).ok_or("Unknown column")?;
                        Self::convert_crush_value_to_protobuf_value(&mut sub_message, &field, &vv)?;
                    }

                    Ok(())
                }

                (expected, actual) => command_error(format!(
                    "Unexpected type of column {}. Expected {}, got {}.",
                    descriptor.name(),
                    crush_type(expected).to_string(),
                    actual.value_type().to_string()
                )),
            }
        }
    }

    fn convert_map_key_to_crush_value(value: &MapKey) -> Value {
        match value {
            MapKey::Bool(b) => Value::from(*b),
            MapKey::I32(i) => Value::from(*i),
            MapKey::I64(i) => Value::from(*i as i128),
            MapKey::U32(i) => Value::from(*i),
            MapKey::U64(i) => Value::from(*i),
            MapKey::String(s) => Value::from(s),
        }
    }
    fn convert_protobuf_value_to_crush_value(
        field: &FieldDescriptor,
        value: &prost_reflect::Value,
    ) -> CrushResult<Value> {
        if field.name() == "map_string_to_int32" {
            println!("map_string_to_int32");
        }
        match value {
            prost_reflect::Value::String(s) => Ok(Value::from(s)),
            prost_reflect::Value::F64(s) => Ok(Value::from(*s)),
            prost_reflect::Value::F32(s) => Ok(Value::from(*s)),
            prost_reflect::Value::Bool(b) => Ok(Value::from(*b)),
            prost_reflect::Value::I32(v) => Ok(Value::from(*v)),
            prost_reflect::Value::I64(v) => Ok(Value::Integer(*v as i128)),
            prost_reflect::Value::U32(v) => Ok(Value::from(*v)),
            prost_reflect::Value::U64(v) => Ok(Value::from(*v)),
            prost_reflect::Value::Bytes(b) => Ok(Value::from(b)),
            prost_reflect::Value::EnumNumber(v) => Ok(Value::from(*v)),
            prost_reflect::Value::Message(m) => {
                if let Kind::Message(sub_message_descriptor) = field.kind() {
                    if sub_message_descriptor.is_map_entry() {
                        panic!();
                    }
                    Self::message_to_struct(&sub_message_descriptor, &m)
                } else {
                    command_error("Type mismatch")
                }
            }
            prost_reflect::Value::List(l) => {
                let vv = l
                    .iter()
                    .map(|v| Self::convert_protobuf_value_to_crush_value(field, v))
                    .collect::<CrushResult<Vec<Value>>>()?;
                Ok(Value::List(List::new(crush_type(field.kind()), vv)))
            }
            prost_reflect::Value::Map(l) => {
                if let Kind::Message(msg) = field.kind() {
                    if !msg.is_map_entry() {
                        return command_error("Map entry is not map");
                    }
                    let key_field = msg.map_entry_key_field();
                    let value_field = msg.map_entry_value_field();
                    let dict =
                        Dict::new(crush_type(key_field.kind()), crush_type(value_field.kind()))?;
                    for (k, v) in l.iter() {
                        let kv = Self::convert_map_key_to_crush_value(k);
                        let vv = Self::convert_protobuf_value_to_crush_value(&value_field, v)?;
                        dict.insert(kv, vv)?;
                    }
                    Ok(Value::Dict(dict))
                } else {
                    command_error("Type mismatch")
                }
            }
        }
    }

    fn row_to_message(
        descriptor: &MessageDescriptor,
        types: &[ColumnType],
        row: &Row,
    ) -> CrushResult<DynamicMessage> {
        let mut res = DynamicMessage::new(descriptor.clone());
        for (vt, vv) in types.iter().zip(row.cells().iter()) {
            let field = descriptor
                .get_field_by_name(vt.name())
                .ok_or("Unknown field")?;
            Self::convert_crush_value_to_protobuf_value(&mut res, &field, vv)?;
        }
        Ok(res)
    }

    fn message_to_row(
        descriptor: &MessageDescriptor,
        message: &DynamicMessage,
    ) -> CrushResult<Row> {
        let v = descriptor
            .fields()
            .map(|column_type| {
                let protobuf_value = message.get_field(&column_type);
                let crush_value =
                    Self::convert_protobuf_value_to_crush_value(&column_type, &protobuf_value)?;
                Ok(crush_value)
            })
            .collect::<CrushResult<Vec<_>>>()?;

        Ok(Row::new(v))
    }

    fn message_to_struct(
        descriptor: &MessageDescriptor,
        message: &DynamicMessage,
    ) -> CrushResult<Value> {
        let res = Struct::empty(None);
        for column_type in descriptor.fields() {
            let protobuf_value = message.get_field(&column_type);
            let crush_value =
                Self::convert_protobuf_value_to_crush_value(&column_type, &protobuf_value)?;
            res.set(column_type.name(), crush_value);
        }
        Ok(Value::Struct(res))
    }

    fn descriptor_to_column_types(descriptor: &MessageDescriptor) -> Vec<ColumnType> {
        descriptor
            .fields()
            .map(|field| {
                let scalar_type = crush_type(field.kind());
                let actual_type = if field.is_list() {
                    ValueType::List(Box::from(scalar_type))
                } else {
                    scalar_type
                };
                ColumnType::new_from_string(field.name().to_string(), actual_type)
            })
            .collect()
    }

    pub async fn invoke_method(
        &self,
        service_name: &str,
        method_name: &str,
        mut input: Stream,
        output: ValueSender,
        printer: &Printer,
    ) -> CrushResult<()> {
        let pool = self
            .get_descriptor_pool(&format!("{}.{}", service_name, method_name))
            .await?;

        let svc_desc = pool
            .get_service_by_name(service_name)
            .ok_or_else(|| GenericError(format!("Service {} not found", service_name)))?;

        let method_desc = svc_desc
            .methods()
            .find(|m| m.name() == method_name)
            .ok_or_else(|| {
                GenericError(format!(
                    "Method {} not found in service {}",
                    method_name, service_name
                ))
            })?;

        let encode_desc = method_desc.input();
        let decode_desc = method_desc.output();

        let grpc_path = format!("/{}/{}", service_name, method_name);
        let path = http::uri::PathAndQuery::try_from(grpc_path)?;

        let codec = DynamicMessageCodec {
            encode_desc: encode_desc.clone(),
            decode_desc: decode_desc.clone(),
        };
        let output_signature = Self::descriptor_to_column_types(&decode_desc);

        let mut grpc_client = tonic::client::Grpc::new(self.channel.clone());

        let (tx, rx) = mpsc::channel::<DynamicMessage>(16);
        let request_stream = ReceiverStream::new(rx);

        grpc_client.ready().await?;

        let timeout = self.timeout;
        let printer = printer.clone();

        tokio::spawn(async move {
            while let Ok(input_row) = input.read_timeout(timeout) {
                match Self::row_to_message(&encode_desc, input.types(), &input_row) {
                    Ok(request) => {
                        if tx.send(request).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        printer.crush_error(e);
                    }
                }
            }
        });

        let mut response = grpc_client
            .streaming(Request::new(request_stream), path, codec)
            .await?
            .into_inner();

        let output = output.initialize(&output_signature)?;

        while let Some(response_message) = response.message().await? {
            let row = Self::message_to_row(&decode_desc, &response_message)?;
            output.send(row)?;
        }

        Ok(())
    }
}

pub fn crush_type(kind: Kind) -> ValueType {
    match kind {
        Kind::Int64 => ValueType::Integer,
        Kind::Uint64 => ValueType::Integer,
        Kind::Int32 => ValueType::Integer,
        Kind::Uint32 => ValueType::Integer,
        Kind::Double => ValueType::Float,
        Kind::Float => ValueType::Float,
        Kind::Bool => ValueType::Bool,
        Kind::String => ValueType::String,
        Kind::Bytes => ValueType::Binary,
        Kind::Message(msg) => {
            if msg.is_map_entry() {
                ValueType::Dict(
                    Box::from(crush_type(msg.map_entry_key_field().kind())),
                    Box::from(crush_type(msg.map_entry_value_field().kind())),
                )
            } else {
                ValueType::Struct
            }
        }
        Kind::Sint32 => ValueType::Integer,
        Kind::Sint64 => ValueType::Integer,
        Kind::Fixed32 => ValueType::Integer,
        Kind::Fixed64 => ValueType::Integer,
        Kind::Sfixed32 => ValueType::Integer,
        Kind::Sfixed64 => ValueType::Integer,
        Kind::Enum(_) => ValueType::Integer,
    }
}
