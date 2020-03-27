use crate::lang::{argument::Argument, value::Value, r#struct::Struct, table::Table, table::ColumnType, value::ValueType, table::Row, binary::binary_channel};
use crate::lang::command::ExecutionContext;
use crate::lang::errors::{argument_error, to_crush_error, CrushResult, demand};
use reqwest::{StatusCode, Method};
use reqwest::header::{HeaderMap};

#[derive(Debug)]
pub struct Config {
    url: String,
    cache: bool,
    body: Option<String>,
    headers: Vec<(String, String)>,
    method: Method,
}

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
        _ => { return argument_error(format!("Unknown method {}", m).as_str()); }
    })
}

fn parse(mut arguments: Vec<Argument>) -> CrushResult<Config> {
    let mut url = None;
    let cache = false;
    let mut headers= Vec::new();
    let mut form = None;
    let mut method = Method::GET;

    for arg in arguments.drain(..) {
        match (arg.argument_type.as_deref(), arg.value) {
            (None, Value::String(t)) | (Some("url"), Value::String(t)) => { url = Some(t); }
//            (Some("cache"), Value::Bool(v)) => { cache = v; }
            (Some("form"), Value::String(t)) => { form = Some(t.to_string()); }
            (Some("method"), Value::String(t)) => { method = parse_method(t.as_ref())?; }
            (Some("header"), Value::String(t)) => {
                let h = t.splitn(2, ':').collect::<Vec<&str>>();
                match h.len() {
                    2 => { headers.push((h[0].to_string(), h[1].to_string()));}
                    _ => { return argument_error("Bad header format") }
                }
            }
            _ => { return argument_error("Unknown argument"); }
        }
    }
    return Ok(Config {
        url: demand(url, "url")?.to_string(),
        method,
        headers,
        cache,
        body: form,
    });
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let cfg = parse(context.arguments)?;
    let (mut output, input) = binary_channel()?;
    let client = reqwest::blocking::Client::new();
    let mut request = client.request(cfg.method, cfg.url.as_str());

    for (k, v) in cfg.headers {
        request = request.header(&k, &v);
    }

    if let Some(body) = cfg.body {
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
                (Box::from("status"), Value::Integer(status.as_u16() as i128)),
                (Box::from("headers"), Value::Table(headers)),
                (Box::from("body"), Value::BinaryStream(input))
            ]
        )));
    to_crush_error(b.copy_to(output.as_mut()))?;
    Ok(())
}
