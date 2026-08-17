use crate::CrushResult;
use crate::builtins::io::json::{json_to_value, value_to_json};
use crate::data::r#struct::Struct;
use crate::lang::any_str::AnyStr;
use crate::lang::command::CrushCommand;
use crate::lang::command::OutputType::Unknown;
use crate::lang::data::list::List;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::{Row, Table};
use crate::lang::errors::{CrushErrorType::GenericError, command_error, error};
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use bytes::{Buf, BufMut, Bytes};
use chrono::Duration;
use itertools::Itertools;
use prost::Message as ProstMessage;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
use regex::Regex;
use signature::signature;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use tonic::codec::{Codec, DecodeBuf, EncodeBuf};
use tonic::codegen::Service;
use tonic::transport::{Channel, Endpoint};
use tonic_reflection::pb::v1::{
    ServerReflectionRequest, server_reflection_client::ServerReflectionClient,
    server_reflection_request, server_reflection_response,
};

#[signature(
    grpc.connect,
    can_block = true,
    short = "Create a connection to a gRPC service)",
    long = "This command currently uses grpcurl under the hood. It does not have a persistent gRPC connections and can therefore be slow."
)]
struct Connect {
    #[description("Host to connect to.")]
    host: String,

    #[description(
        "Service to connect to on this host. This can be a string, a glob or a regular expression, in order to allow you to easily specify multiple services, e.g. use `*` to connect to all available services."
    )]
    service: Patterns,

    #[default(false)]
    #[description("Use plaintext to connect")]
    plaintext: bool,

    #[default(Duration::seconds(5))]
    #[description("Timeout for making calls")]
    timeout: Duration,

    #[default(5990)]
    #[description("Port to connect to")]
    port: i128,
}

struct Grpc {
    host: String,
    plaintext: bool,
    timeout: Duration,
    port: i128,
}

impl Grpc {
    fn new(s: Struct) -> CrushResult<Grpc> {
        if let Some(Value::String(host)) = s.get("host") {
            if let Some(Value::Bool(plaintext)) = s.get("plaintext") {
                if let Some(Value::Duration(timeout)) = s.get("timeout") {
                    if let Some(Value::Integer(port)) = s.get("port") {
                        return Ok(Grpc {
                            host: host.to_string(),
                            plaintext,
                            timeout,
                            port,
                        });
                    }
                }
            }
        }
        command_error("Invalid struct specification.")
    }

    fn list_services(&self) -> CrushResult<String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| GenericError(e.to_string()))?;
        rt.block_on(self.list_services_async())
    }

    fn list_methods(&self, service: &str) -> CrushResult<String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| GenericError(e.to_string()))?;
        rt.block_on(self.list_methods_async(service))
    }

    fn describe(&self, path: &str) -> CrushResult<String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| GenericError(e.to_string()))?;
        rt.block_on(self.describe_async(path))
    }

    fn invoke_method(&self, method: &str, data: Option<String>) -> CrushResult<String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| GenericError(e.to_string()))?;
        rt.block_on(self.invoke_method_async(method, data))
    }

    async fn connect_channel(&self) -> CrushResult<Channel> {
        let uri = if self.plaintext {
            format!("http://{}:{}", self.host, self.port)
        } else {
            format!("https://{}:{}", self.host, self.port)
        };

        let mut endpoint = Endpoint::from_shared(uri)
            .map_err(|e| GenericError(e.to_string()))?
            .timeout(
                self.timeout
                    .to_std()
                    .unwrap_or(std::time::Duration::from_secs(5)),
            );

        if !self.plaintext {
            endpoint = endpoint
                .tls_config(tonic::transport::ClientTlsConfig::new())
                .map_err(|e| GenericError(e.to_string()))?;
        }

        Ok(endpoint
            .connect()
            .await
            .map_err(|e| GenericError(e.to_string()))?)
    }

    async fn reflection_request(
        &self,
        channel: Channel,
        message_request: server_reflection_request::MessageRequest,
    ) -> CrushResult<server_reflection_response::MessageResponse> {
        let mut client = ServerReflectionClient::new(channel);
        let req = ServerReflectionRequest {
            host: "".to_string(), //self.host.clone(),
            message_request: Some(message_request),
        };

        let response = client
            .server_reflection_info(tokio_stream::once(req))
            .await
            .map_err(|e| GenericError(format!("Reflection request failed: {}", e)))?;

        let mut stream = response.into_inner();
        let msg = stream
            .message()
            .await
            .map_err(|e| GenericError(format!("Failed to read reflection response: {}", e)))?
            .ok_or_else(|| GenericError("Empty reflection response".to_string()))?;

        Ok(msg.message_response.ok_or_else(|| {
            GenericError("Missing message_response in reflection response".to_string())
        })?)
    }

    async fn list_services_async(&self) -> CrushResult<String> {
        let channel = self.connect_channel().await?;
        let resp = self
            .reflection_request(
                channel,
                server_reflection_request::MessageRequest::ListServices(String::new()),
            )
            .await?;

        match resp {
            server_reflection_response::MessageResponse::ListServicesResponse(list) => {
                let names: Vec<String> = list.service.into_iter().map(|s| s.name).collect();
                Ok(names.join("\n"))
            }
            _ => command_error("Unexpected reflection response type"),
        }
    }

    async fn get_descriptor_pool(
        &self,
        channel: Channel,
        symbol: &str,
    ) -> CrushResult<DescriptorPool> {
        let resp = self
            .reflection_request(
                channel,
                server_reflection_request::MessageRequest::FileContainingSymbol(symbol.to_string()),
            )
            .await?;

        match resp {
            server_reflection_response::MessageResponse::FileDescriptorResponse(fdr) => {
                let mut fds = prost_types::FileDescriptorSet { file: Vec::new() };
                for fd_bytes in &fdr.file_descriptor_proto {
                    let fd =
                        prost_types::FileDescriptorProto::decode(&fd_bytes[..]).map_err(|e| {
                            GenericError(format!("Failed to decode file descriptor: {}", e))
                        })?;
                    fds.file.push(fd);
                }
                Ok(DescriptorPool::from_file_descriptor_set(fds)
                    .map_err(|e| GenericError(format!("Failed to build descriptor pool: {}", e)))?)
            }
            _ => command_error("Unexpected reflection response type"),
        }
    }

    async fn list_methods_async(&self, service: &str) -> CrushResult<String> {
        let channel = self.connect_channel().await?;
        let pool = self.get_descriptor_pool(channel, service).await?;

        let svc_desc = pool
            .get_service_by_name(service)
            .ok_or_else(|| GenericError(format!("Service {} not found in descriptors", service)))?;

        let methods: Vec<String> = svc_desc
            .methods()
            .map(|m| format!("{}.{}", service, m.name()))
            .collect();

        Ok(methods.join("\n"))
    }

    async fn describe_async(&self, full_path: &str) -> CrushResult<String> {
        let dot_pos = full_path
            .rfind('.')
            .ok_or_else(|| GenericError(format!("Invalid message/method path: {}", full_path)))?;
        let service = &full_path[..dot_pos];
        let method = &full_path[dot_pos + 1..];

        let channel = self.connect_channel().await?;
        let pool = self
            .get_descriptor_pool(channel.clone(), &full_path)
            .await?;

        if let Some(msg_desc) = pool.get_message_by_name(&full_path) {
            return Ok(format_message_descriptor(&msg_desc));
        }

        if let Some(svc_desc) = pool.get_service_by_name(&full_path) {
            let mut result = String::new();
            for method in svc_desc.methods() {
                result += &format!(
                    "rpc {} ( {} ) returns ( {} );\n",
                    method.name(),
                    method.input().full_name(),
                    method.output().full_name(),
                );
            }
            return Ok(result);
        }

        let pool = self.get_descriptor_pool(channel, service).await?;
        if let Some(sd) = pool.get_service_by_name(service) {
            for md in sd.methods() {
                if md.name() == method {
                    return Ok(format!(
                        "rpc {} ( {} ) returns ( {} );\n",
                        md.name(),
                        md.input().full_name(),
                        md.output().full_name(),
                    ));
                }
            }
        }

        command_error(format!("Symbol {} not found", full_path))
    }

    async fn invoke_method_async(&self, method: &str, data: Option<String>) -> CrushResult<String> {
        let channel = self.connect_channel().await?;

        let pool = self.get_descriptor_pool(channel.clone(), method).await?;

        let dot_pos = method
            .rfind('.')
            .ok_or_else(|| GenericError(format!("Invalid method name: {}", method)))?;
        let service_name = &method[..dot_pos];
        let method_name = &method[dot_pos + 1..];

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

        let input_desc = method_desc.input();
        let output_desc = method_desc.output();

        let request_bytes = if let Some(json_str) = data {
            let mut deserializer = serde_json::Deserializer::from_str(&json_str);
            let msg = DynamicMessage::deserialize(input_desc.clone(), &mut deserializer)
                .map_err(|e| GenericError(format!("Failed to parse JSON input: {}", e)))?;
            Bytes::from(msg.encode_to_vec())
        } else {
            let msg = DynamicMessage::new(input_desc.clone());
            Bytes::from(msg.encode_to_vec())
        };

        let grpc_path = format!("/{}/{}", service_name, method_name);
        let path = http::uri::PathAndQuery::try_from(grpc_path)
            .map_err(|e| GenericError(format!("Invalid method path: {}", e)))?;

        let mut grpc_client = tonic::client::Grpc::new(channel);
        grpc_client
            .ready()
            .await
            .map_err(|e| GenericError(format!("Channel not ready: {}", e)))?;

        let request = tonic::Request::new(request_bytes);
        let response = grpc_client
            .unary(request, path, RawBytesCodec)
            .await
            .map_err(|e| GenericError(format!("gRPC call failed: {}", e)))?;

        let response_bytes = response.into_inner();
        let response_msg = DynamicMessage::decode(output_desc, &response_bytes[..])
            .map_err(|e| GenericError(format!("Failed to decode response: {}", e)))?;

        Ok(serde_json::to_string_pretty(&response_msg)
            .map_err(|e| GenericError(format!("Failed to serialize response to JSON: {}", e)))?)
    }
}

#[derive(Default, Clone)]
struct RawBytesCodec;

impl Codec for RawBytesCodec {
    type Encode = Bytes;
    type Decode = Bytes;
    type Encoder = RawBytesEncoder;
    type Decoder = RawBytesDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        RawBytesEncoder
    }

    fn decoder(&mut self) -> Self::Decoder {
        RawBytesDecoder
    }
}

#[derive(Default, Clone)]
struct RawBytesEncoder;

impl tonic::codec::Encoder for RawBytesEncoder {
    type Item = Bytes;
    type Error = tonic::Status;

    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        dst.put(item);
        Ok(())
    }
}

#[derive(Default, Clone)]
struct RawBytesDecoder;

impl tonic::codec::Decoder for RawBytesDecoder {
    type Item = Bytes;
    type Error = tonic::Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        if src.remaining() == 0 {
            return Ok(None);
        }
        Ok(Some(src.copy_to_bytes(src.remaining())))
    }
}

fn format_message_descriptor(msg_desc: &MessageDescriptor) -> String {
    let mut result = format!("message {} {{\n", msg_desc.name());
    for field in msg_desc.fields() {
        let type_str = match field.kind() {
            prost_reflect::Kind::Double => "double".to_string(),
            prost_reflect::Kind::Float => "float".to_string(),
            prost_reflect::Kind::Int64 => "int64".to_string(),
            prost_reflect::Kind::Uint64 => "uint64".to_string(),
            prost_reflect::Kind::Int32 => "int32".to_string(),
            prost_reflect::Kind::Uint32 => "uint32".to_string(),
            prost_reflect::Kind::Bool => "bool".to_string(),
            prost_reflect::Kind::String => "string".to_string(),
            prost_reflect::Kind::Bytes => "bytes".to_string(),
            prost_reflect::Kind::Message(m) => m.full_name().to_string(),
            prost_reflect::Kind::Enum(e) => e.full_name().to_string(),
            _ => "unknown".to_string(),
        };
        result += &format!("  {} {} = {};\n", type_str, field.name(), field.number());
    }
    result += "}\n";
    result
}

#[derive(Clone, Debug)]
struct ProtoMessage {
    name: String,
    fields: Vec<ProtoField>,
}

#[derive(Clone, Debug)]
struct ProtoField {
    name: String,
    proto_type: ProtoType,
}

#[derive(Clone, Debug)]
enum ProtoType {
    Int64,
    UInt64,
    Int32,
    UInt32,
    Double,
    Float,
    Bool,
    String,
    Bytes,
    Message(ProtoMessage),
}

impl ProtoType {
    fn crush_type(&self) -> ValueType {
        match self {
            ProtoType::Int64 => ValueType::Integer,
            ProtoType::UInt64 => ValueType::Integer,
            ProtoType::Int32 => ValueType::Integer,
            ProtoType::UInt32 => ValueType::Integer,
            ProtoType::Double => ValueType::Float,
            ProtoType::Float => ValueType::Float,
            ProtoType::Bool => ValueType::Bool,
            ProtoType::String => ValueType::String,
            ProtoType::Bytes => ValueType::Binary,
            ProtoType::Message(_) => ValueType::Struct,
        }
    }

    fn arguments(&self) -> String {
        if let ProtoType::Message(fields) = self {
            fields
                .fields
                .iter()
                .map(|f| format!("{}={}", f.name, f.proto_type.crush_type().to_string()))
                .join(" ")
        } else {
            self.crush_type().to_string()
        }
    }
}

fn insert_known_types(known_types: &mut HashMap<String, ProtoType>) {
    known_types.insert("int32".to_string(), ProtoType::Int32);
    known_types.insert("int64".to_string(), ProtoType::Int64);
    known_types.insert("uint32".to_string(), ProtoType::UInt32);
    known_types.insert("uint64".to_string(), ProtoType::UInt64);
    known_types.insert("bool".to_string(), ProtoType::Bool);
    known_types.insert("string".to_string(), ProtoType::String);
    known_types.insert("bytes".to_string(), ProtoType::Bytes);
    known_types.insert("double".to_string(), ProtoType::Double);
    known_types.insert("float".to_string(), ProtoType::Float);
}

fn parse_message_type<'a>(
    context: &CommandContext,
    name: &str,
    grpc: &Grpc,
    known_types: &'a mut HashMap<String, ProtoType>,
) -> CrushResult<ProtoType> {
    if let Some(t) = known_types.get(name) {
        return Ok(t.clone());
    }

    let signature = grpc.describe(name)?;

    static REGEX: OnceLock<Regex> = OnceLock::new();
    let re = REGEX.get_or_init(|| {
        Regex::new(r"[[:blank:]]*([a-zA-Z_.][a-zA-Z0-9_.]*)[[:blank:]]+([a-zA-Z_][a-zA-Z0-9_]*)[[:blank:]]*=[[:blank:]]*([0-9]+);[[:blank:]]*").unwrap()
    });

    let mut fields = Vec::new();

    for line in signature.lines() {
        match re.captures(line) {
            None => {}
            Some(c) => match (c.get(1), c.get(2)) {
                (Some(type_name), Some(field_name)) => {
                    let field_type =
                        parse_message_type(context, type_name.as_str(), grpc, known_types)?;
                    fields.push(ProtoField {
                        name: field_name.as_str().to_string(),
                        proto_type: field_type,
                    });
                }
                _ => {}
            },
        };
    }

    let res = ProtoType::Message(ProtoMessage {
        name: name.to_string(),
        fields,
    });

    known_types.insert(name.to_string(), res.clone());

    Ok(res)
}

fn connect(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Connect = Connect::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.service.is_empty() {
        return command_error(
            "You must specify at least one service to connect to. You can use globs, such as `*`.",
        );
    }
    let tmp = Struct::new(
        vec![
            ("host", Value::from(cfg.host.clone())),
            ("plaintext", Value::Bool(cfg.plaintext)),
            ("timeout", Value::Duration(cfg.timeout)),
            ("port", Value::Integer(cfg.port)),
        ],
        None,
    );

    let g = Grpc::new(tmp)?;
    let s = Struct::from_vec(vec![], vec![]);
    let list = g.list_services()?;
    let mut available_services = list.lines().collect::<Vec<&str>>();
    let services = available_services
        .drain(..)
        .filter(|s| cfg.service.test(s))
        .collect::<Vec<&str>>();

    if services.is_empty() {
        return command_error(format!(
            "No match for service pattern `{}`. Found services `{}`.",
            cfg.service.to_string(),
            list.lines().join(", ")
        ));
    }

    let mut known_types = HashMap::new();
    insert_known_types(&mut known_types);

    for service in services {
        let out = g.list_methods(service)?;
        for line in out.lines() {
            let stripped = line.strip_prefix(&format!("{}.", service));
            if let Some(method) = stripped {
                let signature = g.describe(format!("{}.{}", service, method).as_str())?;
                let input_type_name = parse_input_type_from_signature(method, signature.as_str())?;
                let input_type =
                    parse_message_type(&context, &input_type_name, &g, &mut known_types)?;

                s.set(
                    method,
                    Value::Struct(Struct::new(
                        vec![
                            ("host", Value::from(cfg.host.clone())),
                            ("service", Value::from(service.to_string())),
                            ("plaintext", Value::Bool(cfg.plaintext)),
                            ("timeout", Value::Duration(cfg.timeout)),
                            ("port", Value::Integer(cfg.port)),
                            ("method", Value::from(line)),
                            (
                                "__call__",
                                Value::Command(<dyn CrushCommand>::command(
                                    grpc_method_call,
                                    true,
                                    &["global", "grpc", "connect", method, "__call__"],
                                    format!("{} {}", method, input_type.arguments()),
                                    format!(
                                        "Call the {} method of the {} service",
                                        method, service
                                    ),
                                    None::<AnyStr>,
                                    Unknown,
                                    [],
                                )),
                            ),
                        ],
                        None,
                    )),
                );
            }
        }
    }
    context.output.send(Value::Struct(s))
}

fn parse_input_type_from_signature<'a>(
    method_name: &str,
    signature: &'a str,
) -> CrushResult<&'a str> {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    let re = REGEX.get_or_init(|| Regex::new(r"\((.*)\).*\(.*\)").unwrap());
    for line in signature.lines() {
        if line.starts_with("rpc") {
            return match re.captures(line) {
                None => command_error("Failed to parse signature."),
                Some(c) => match c.get(1) {
                    None => command_error("Failed to parse signature."),
                    Some(m) => Ok(m.as_str().trim()),
                },
            };
        }
    }
    command_error(format!(
        "Failed to parse signature of method `{}`.",
        method_name
    ))
}

fn grpc_method_call(mut context: CommandContext) -> CrushResult<()> {
    let data = if context.input.is_pipeline() {
        let data = context.input.recv()?;
        Some(value_to_json(data)?)
    } else {
        if !context.arguments.is_empty() {
            let mut fields = Vec::new();
            for a in context.remove_arguments() {
                if let Some(name) = a.argument_type {
                    fields.push((name, a.value));
                } else {
                    return command_error("gRPC method invocations can only use named arguments.");
                }
            }
            let s = Struct::new(fields, None);
            Some(value_to_json(Value::Struct(s))?)
        } else {
            None
        }
    };
    let this = context.this.r#struct()?;
    if let Some(Value::String(method)) = this.get("method") {
        let grpc = Grpc::new(this)?;
        let out = grpc.invoke_method(method.as_ref(), data)?;

        let split = out.split("\n}\n{\n");

        let mut lst = split
            .into_iter()
            .map(|i| {
                let stripped = i.trim();
                match (stripped.starts_with("{"), stripped.ends_with("}")) {
                    (true, true) => json_to_value(i),
                    (true, false) => json_to_value(&format!("{}}}", i)),
                    (false, false) => json_to_value(&format!("{{{}}}", i)),
                    (false, true) => json_to_value(&format!("{{{}", i)),
                }
            })
            .collect::<CrushResult<Vec<_>>>()?;

        let types: HashSet<ValueType> = lst.iter().map(|v| v.value_type()).collect();
        let struct_types: HashSet<Vec<ColumnType>> = lst
            .iter()
            .flat_map(|v| match v {
                Value::Struct(r) => vec![r.local_signature()],
                _ => vec![],
            })
            .collect();

        let res = match types.len() {
            0 => Value::Empty,
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
                        Value::Table(Table::from((
                            struct_types.iter().next().unwrap().clone(),
                            row_list,
                        )))
                    }
                    _ => List::new(list_type.clone(), lst).into(),
                }
            }
            _ => List::new(ValueType::Any, lst).into(),
        };

        context.output.send(res)?;

        return Ok(());
    }
    command_error("Invalid method field.")
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "grpc",
        "gRPC connection",
        Box::new(move |grpc| {
            Connect::declare(grpc)?;
            Ok(())
        }),
    )?;
    Ok(())
}
