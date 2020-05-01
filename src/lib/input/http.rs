use crate::lang::{value::Value, r#struct::Struct, table::Table, table::ColumnType, value::ValueType, table::Row, binary::binary_channel};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{argument_error, to_crush_error, CrushResult};
use reqwest::{StatusCode, Method};
use reqwest::header::HeaderMap;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

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

#[signature]
struct Signature {
    uri: String,
    #[values("get", "post", "put", "delete", "head", "options", "connect", "patch", "trace")]
    #[default("get")]
    method: String,
    form: Option<String>,
    header: Vec<String>,
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Signature = Signature::parse(context.arguments, &context.printer)?;

    let (mut output, input) = binary_channel();
    let client = reqwest::blocking::Client::new();
    let mut request = client.request(parse_method(&cfg.method)?, cfg.uri.as_str());

    for t in cfg.header.iter() {
        let h = t.splitn(2, ':').collect::<Vec<&str>>();
        match h.len() {
            2 => { request = request.header(h[0], h[1].to_string()); }
            _ => { return argument_error("Bad header format"); }
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
            .map(|(n, v)| Row::new(vec![Value::string(n.as_str()), Value::string(v.to_str().unwrap())]))
            .collect());
    let _ = context.output.send(
        Value::Struct(Struct::new(
            vec![
                ("status".to_string(), Value::Integer(status.as_u16() as i128)),
                ("headers".to_string(), Value::Table(headers)),
                ("body".to_string(), Value::BinaryStream(input))
            ],
            None,
        )));
    to_crush_error(b.copy_to(output.as_mut()))?;
    Ok(())
}
