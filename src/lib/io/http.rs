use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::{argument_error, to_crush_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::{
    binary::binary_channel, r#struct::Struct, table::ColumnType, table::Row, table::Table,
    value::Value, value::ValueType,
};
use reqwest::header::HeaderMap;
use reqwest::{Method, StatusCode};
use signature::signature;

fn parse_method(m: &str) -> CrushResult<Method> {
    Ok(match m.to_lowercase().as_str() {
        "get" => Method::GET,
        "post" => Method::POST,
        "put" => Method::PUT,
        "delete" => Method::DELETE,
        "head" => Method::HEAD,
        "options" => Method::OPTIONS,
        "connect" => Method::CONNECT,
        "patch" => Method::PATCH,
        "trace" => Method::TRACE,
        _ => return argument_error(format!("Unknown method {}", m).as_str()),
    })
}

#[signature(
    http,
    short = "Make a http request",
    long = "Return a struct with the following fields:",
    long = "* status:integer, the http status of the reply",
    long = "* header:list, the http headers of the reply",
    long = "* body:binary_stream, the content of the reply",
    example = "http \"https://example.com/\" header=(\"Authorization: Bearer {}\":format token)",
    can_block = true
)]
pub struct Http {
    uri: String,
    #[description("HTTP method.")]
    #[values(
        "get", "post", "put", "delete", "head", "options", "connect", "patch", "trace"
    )]
    #[default("get")]
    method: String,
    #[description("form content, if any.")]
    form: Option<String>,
    #[description("HTTP headers, must be on the form \"key:value\".")]
    header: Vec<String>,
}

pub fn http(context: CommandContext) -> CrushResult<()> {
    let cfg: Http = Http::parse(context.arguments, &context.printer)?;

    let (mut output, input) = binary_channel();
    let client = reqwest::blocking::Client::new();
    let mut request = client.request(parse_method(&cfg.method)?, cfg.uri.as_str());

    for t in cfg.header.iter() {
        let h = t.splitn(2, ':').collect::<Vec<&str>>();
        match h.len() {
            2 => {
                request = request.header(h[0], h[1].to_string());
            }
            _ => {
                return argument_error("Bad header format");
            }
        }
    }

    if let Some(body) = cfg.form {
        request = request.body(body)
    }

    let mut b = to_crush_error(request.send())?;

    let status: StatusCode = b.status();
    let header_map: &HeaderMap = b.headers();
    let headers = Table::new(
        vec![
            ColumnType::new("name", ValueType::String),
            ColumnType::new("value", ValueType::String),
        ],
        header_map
            .iter()
            .map(|(n, v)| {
                Row::new(vec![
                    Value::string(n.as_str()),
                    Value::string(v.to_str().unwrap()),
                ])
            })
            .collect(),
    );
    let _ = context.output.send(Value::Struct(Struct::new(
        vec![
            (
                "status".to_string(),
                Value::Integer(status.as_u16() as i128),
            ),
            ("headers".to_string(), Value::Table(headers)),
            ("body".to_string(), Value::BinaryStream(input)),
        ],
        None,
    )));
    to_crush_error(b.copy_to(output.as_mut()))?;
    Ok(())
}
