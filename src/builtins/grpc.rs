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
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use prost::Message as ProstMessage;
use prost_reflect::{DescriptorPool, DynamicMessage, Kind, MessageDescriptor, MethodDescriptor};
use signature::signature;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{LazyLock, OnceLock};
use tokio::runtime::Runtime;
use tonic::codec::{Codec, DecodeBuf, EncodeBuf};
use tonic::codegen::Service;
use tonic::transport::{Channel, Endpoint};
use tonic_reflection::pb::v1::{
    ServerReflectionRequest, server_reflection_client::ServerReflectionClient,
    server_reflection_request, server_reflection_response,
};

static CONNECTIONS: LazyLock<RwLock<HashMap<i32, Option<GrpcClient>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
static NEXT_ID: AtomicI32 = AtomicI32::new(1);

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize static Tokio runtime")
});

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

struct GrpcClient {
    channel: Channel,
    host: String,
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

    let mut endpoint = Endpoint::from_shared(uri)
        .map_err(|e| GenericError(e.to_string()))?
        .timeout(
            timeout
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(5)),
        );

    if !plaintext {
        endpoint = endpoint
            .tls_config(tonic::transport::ClientTlsConfig::new())
            .map_err(|e| GenericError(e.to_string()))?;
    }

    Ok(endpoint
        .connect()
        .await
        .map_err(|e| GenericError(e.to_string()))?)
}

impl GrpcClient {
    fn connections() -> &'static RwLock<HashMap<i32, Option<GrpcClient>>> {
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

    fn get(s: &Struct) -> CrushResult<MappedRwLockReadGuard<'static, GrpcClient>> {
        match s.get("id") {
            Some(Value::Integer(id)) => {
                let id = id as i32;
                Self::get_from_id(id)
            }

            _ => command_error("Missing or invalid id field in grpc method object"),
        }
    }

    fn get_from_id(id: i32) -> CrushResult<MappedRwLockReadGuard<'static, GrpcClient>> {
        let res = RwLockReadGuard::try_map(Self::connections().read(), |map| match map.get(&id) {
            Some(Some(opt)) => Some(opt),
            _ => None,
        });
        match res {
            Ok(client) => {
                Ok(client)
            }
            _ => command_error(format!("Invalid id {}", id)),
        }
    }

    async fn create(
        host: &str,
        plaintext: bool,
        timeout: Duration,
        port: i128,
    ) -> CrushResult<i32> {
        let client = GrpcClient {
            channel: connect_channel(host.as_ref(), port, timeout, plaintext).await?,
            host: host.to_string(),
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

    async fn list_services(&self) -> CrushResult<String> {
        let resp = self
            .reflection_request(server_reflection_request::MessageRequest::ListServices(
                String::new(),
            ))
            .await?;

        match resp {
            server_reflection_response::MessageResponse::ListServicesResponse(list) => {
                let names: Vec<String> = list.service.into_iter().map(|s| s.name).collect();
                Ok(names.join("\n"))
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

    async fn list_methods(&self, service: &str) -> CrushResult<String> {
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

    async fn describe_message(&self, message: &str) -> CrushResult<MessageDescriptor> {
        let pool = self.get_descriptor_pool(&message).await?;

        if let Some(msg_desc) = pool.get_message_by_name(&message) {
            Ok(msg_desc)
        } else {
            command_error(format!("Message {} not found", message))
        }
    }

    async fn describe_method(&self, service: &str, method: &str) -> CrushResult<MethodDescriptor> {
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

    async fn invoke_method(
        &self,
        service_name: &str,
        method_name: &str,
        data: Option<String>,
    ) -> CrushResult<String> {
        let pool = self.get_descriptor_pool(&format!("{}.{}", service_name, method_name)).await?;

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

        let mut grpc_client = tonic::client::Grpc::new(self.channel.clone());
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

fn crush_type(kind: Kind) -> ValueType {
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
        Kind::Message(_) => ValueType::Struct,
        Kind::Sint32 => ValueType::Integer,
        Kind::Sint64 => ValueType::Integer,
        Kind::Fixed32 => ValueType::Integer,
        Kind::Fixed64 => ValueType::Integer,
        Kind::Sfixed32 => ValueType::Integer,
        Kind::Sfixed64 => ValueType::Integer,
        Kind::Enum(_) => ValueType::Integer,
    }
}

fn runtime() -> &'static Runtime {
    &RUNTIME
}

fn connect(context: CommandContext) -> CrushResult<()> {
    runtime().block_on(connect_async(context))
}

async fn connect_async(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Connect = Connect::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.service.is_empty() {
        return command_error(
            "You must specify at least one service to connect to. You can use globs, such as `*`.",
        );
    }

    let id = GrpcClient::create(&cfg.host, cfg.plaintext, cfg.timeout, cfg.port).await?;
    let grpc_client = GrpcClient::get_from_id(id)?;
    let grpc_struct = Struct::from_vec(vec![], vec![]);
    let list = grpc_client.list_services().await?;
    let mut available_services = list.lines().collect::<Vec<&str>>();
    let services = available_services
        .drain(..)
        .filter(|s| cfg.service.test(s))
        .collect::<Vec<&str>>();

    if services.is_empty() {
        return command_error(format!(
            "No match for service pattern `{}`. Found the following services: {}.",
            cfg.service.to_string(),
            list.lines().map(|s| format!("`{}`", s)).join(", ")
        ));
    }

    for service in services {
        let out = grpc_client.list_methods(service).await?;
        for method in out.lines() {
            let stripped = method.strip_prefix(&format!("{}.", service));
            if let Some(method) = stripped {
                let signature = grpc_client.describe_method(service, method).await?;
                let input = signature.input();

                let signature = input
                    .fields()
                    .map(|field| {
                        format!("{}={}", field.name(), crush_type(field.kind()).to_string())
                    })
                    .join(" ");

                grpc_struct.set(
                    method,
                    Value::Struct(Struct::new(
                        vec![
                            ("id", Value::from(id)),
                            ("method", Value::from(method)),
                            ("service", Value::from(service)),
                            (
                                "__call__",
                                Value::Command(<dyn CrushCommand>::command(
                                    grpc_method_call,
                                    true,
                                    &["global", "grpc", "connect", method, "__call__"],
                                    format!("{} {}", method, signature),
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
    context.output.send(Value::Struct(grpc_struct))
}

fn grpc_method_call(mut context: CommandContext) -> CrushResult<()> {
    runtime().block_on(grpc_method_call_async(context))
}

async fn grpc_method_call_async(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let grpc = GrpcClient::get(&this)?;
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
    match (this.get("service"), this.get("method")) {
        (Some(Value::String(service)), Some(Value::String(method))) => {
            let out = grpc.invoke_method(&service, &method, data).await?;

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

            Ok(())
        }
        _ => command_error("Invalid method field."),
    }
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
