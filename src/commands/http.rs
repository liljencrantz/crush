use crate::data::{Argument, Value, Struct, Rows, ColumnType, ValueType, Row, binary_channel};
use crate::commands::CompileContext;
use crate::errors::{argument_error, to_job_error, CrushResult, demand};
use reqwest::{StatusCode, Method};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;

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
    let mut cache = false;
    let mut headers= Vec::new();
    let mut form = None;
    let mut method = Method::GET;

    for arg in arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (None, Value::Text(t)) | (Some("url"), Value::Text(t)) => { url = Some(t); }
//            (Some("cache"), Value::Bool(v)) => { cache = v; }
            (Some("form"), Value::Text(t)) => { form = Some(t.to_string()); }
            (Some("method"), Value::Text(t)) => { method = parse_method(t.as_ref())?; }
            (Some("header"), Value::Text(t)) => {
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

pub fn perform(context: CompileContext) -> CrushResult<()> {
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

    let mut b = to_job_error(request.send())?;

    let status: StatusCode = b.status();
    let header_map: &HeaderMap = b.headers();
    let headers = Rows::new(
        vec![
            ColumnType::named("name", ValueType::Text),
            ColumnType::named("value", ValueType::Text),
        ],
        header_map
            .iter()
            .map(|(n, v)| Row::new(vec![Value::text(n.as_str()), Value::text(v.to_str().unwrap())]))
            .collect());
    context.output.send(
        Value::Struct(Struct::new(
            vec![
                (Box::from("status"), Value::Integer(status.as_u16() as i128)),
                (Box::from("headers"), Value::Rows(headers)),
                (Box::from("body"), Value::BinaryReader(input))
            ]
        )));
    to_job_error(b.copy_to(output.as_mut()))?;
    Ok(())
}
