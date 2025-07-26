use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::binary_input;
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::state::contexts::CommandContext;
use crate::lang::{
    data::binary::binary_channel, data::r#struct::Struct, data::table::ColumnType,
    data::table::Row, data::table::Table, value::Value, value::ValueType,
};
use chrono::Duration;
use reqwest::header::HeaderMap;
use reqwest::{Method, StatusCode};
use signature::signature;
use std::io::Read;

fn parse_method(m: &str) -> CrushResult<Method> {
    Ok(match m {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "HEAD" => Method::HEAD,
        "OPTIONS" => Method::OPTIONS,
        "CONNECT" => Method::CONNECT,
        "PATCH" => Method::PATCH,
        "TRACE" => Method::TRACE,
        _ => return command_error(format!("Unknown method {}", m)),
    })
}

#[signature(
io.http,
short = "Make a http request",
long = "Returns a struct with the following fields:",
long = "* `status_code` (integer) the http status code of the reply",
long = "* `status_name` (string) the name associated with the http status code",
long = "* `header` (list) the http headers of the reply",
long = "* `body` (binary_stream) the content of the reply",
long = "",
long = "The http status codes and corresponding names are defined in",
long = "https://www.iana.org/assignments/http-status-codes/http-status-codes.xhtml",
example = "http \"https://example.com/\" header=$(\"Authorization: Bearer {}\":format $token)",
can_block = true
)]
pub struct Http {
    #[description("URI to request")]
    uri: String,
    #[description("the HTTP method to use in this request.")]
    #[values(
        "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "CONNECT", "PATCH", "TRACE"
    )]
    #[default("GET")]
    method: String,
    #[description("form content, if any.")]
    form: Option<BinaryInput>,
    #[description("HTTP headers, must be on the form \"key:value\".")]
    header: Vec<String>,
    #[description("connection timeout.")]
    #[default(Duration::seconds(5))]
    timeout: Duration,
}

fn http(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Http::parse(context.remove_arguments(), &context.global_state.printer())?;
    let (mut output, input) = binary_channel();
    let client = reqwest::blocking::Client::new();
    let t = cfg
        .timeout
        .num_nanoseconds()
        .map(|us| core::time::Duration::from_nanos(us as u64))
        .ok_or("Out of bounds timeout")?;
    let mut request = client
        .request(parse_method(&cfg.method)?, cfg.uri.as_str())
        .timeout(t);

    for t in cfg.header.iter() {
        let h = t.splitn(2, ':').collect::<Vec<&str>>();
        match h.len() {
            2 => request = request.header(h[0], h[1].to_string()),
            _ => return command_error("Bad header format. Expected `key:value`."),
        }
    }

    if let Some(body) = cfg.form {
        let mut reader = binary_input::input_reader(body)?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        request = request.body(buf);
    }

    let mut b = request.send()?;

    let status: StatusCode = b.status();
    let header_map: &HeaderMap = b.headers();
    let headers = Table::from((
        vec![
            ColumnType::new("name", ValueType::String),
            ColumnType::new("value", ValueType::String),
        ],
        header_map
            .iter()
            .map(|(n, v)| {
                Ok(Row::new(vec![
                    Value::from(n.as_str()),
                    Value::from(v.to_str()?),
                ]))
            })
            .collect::<CrushResult<Vec<_>>>()?,
    ));
    context.output.send(Value::Struct(Struct::new(
        vec![
            ("status_code", Value::Integer(status.as_u16() as i128)),
            ("status_name", Value::from(status.to_string())),
            ("headers", Value::Table(headers)),
            ("body", Value::BinaryInputStream(input)),
        ],
        None,
    )))?;
    b.copy_to(output.as_mut())?;
    Ok(())
}
