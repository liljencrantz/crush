use std::fs::File;
use std::io::Read;
use crate::{argument_error_legacy, CrushResult, to_crush_error};
use crate::lang::state::scope::Scope;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::{Value, ValueType};
use signature::signature;
use crate::data::table::{ColumnType, Row};
use crate::lang::command::OutputType::Known;
use std::str::FromStr;
use chrono::Duration;
use trust_dns_client::client::{Client, ClientConnection, SyncClient};
use trust_dns_client::udp::UdpClientConnection;
use trust_dns_client::rr::{DNSClass, Name, RData, RecordType};
use trust_dns_client::tcp::TcpClientConnection;
use crate::data::list::List;
use crate::lang::errors::data_error;

use std::sync::OnceLock;

fn a_stream_output_type() -> &'static Vec<ColumnType> {
    static CELL: OnceLock<Vec<ColumnType>> = OnceLock::new();
    CELL.get_or_init(|| vec![
        ColumnType::new("target", ValueType::String),
        ColumnType::new("ttl", ValueType::Duration),
    ])
}

fn srv_stream_output_type() -> &'static Vec<ColumnType> {
    static CELL: OnceLock<Vec<ColumnType>> = OnceLock::new();
    CELL.get_or_init(|| vec![
        ColumnType::new("target", ValueType::String),
        ColumnType::new("priority", ValueType::Integer),
        ColumnType::new("weight", ValueType::Integer),
        ColumnType::new("port", ValueType::Integer),
        ColumnType::new("ttl", ValueType::Duration),
    ])
}

#[signature(
    dns.query,
    can_block = true,
    short = "Look up DNS record)",
)]
struct Query {
    #[description("DNS record to look up.")]
    name: String,
    #[description("DNS record type. Currently, A, AAAA and SRV are supported.")]
    #[default("A")]
    record_type: String,
    #[default(false)]
    tcp: bool,
    nameserver: Option<String>,
    #[default(53)]
    port: i128,
}

fn resolv_conf() -> CrushResult<resolv_conf::Config> {
    let mut buf = Vec::with_capacity(8192);
    let mut f = to_crush_error(File::open("/etc/resolv.conf"))?;
    to_crush_error(f.read_to_end(&mut buf))?;
    to_crush_error(resolv_conf::Config::parse(&buf))
}

fn query_internal(cfg: Query, context: CommandContext, client: SyncClient<impl ClientConnection>) -> CrushResult<()> {
    match cfg.record_type.as_ref() {
        "A" => {
            let response = to_crush_error(client.query(&to_crush_error(Name::from_str(&cfg.name))?, DNSClass::IN, RecordType::A))?;
            let output = context.output.initialize(&a_stream_output_type())?;

            for answer in response.answers() {
                match answer.data() {
                    Some(RData::A(ip)) => output.send(Row::new(vec![
                        Value::from(ip.to_string()),
                        Value::Duration(Duration::seconds(answer.ttl() as i64))]))?,
                    _ => return data_error("Missing A record"),
                }
            }
        }
        "AAAA" => {
            let response = to_crush_error(client.query(&to_crush_error(Name::from_str(&cfg.name))?, DNSClass::IN, RecordType::AAAA))?;
            let output = context.output.initialize(&a_stream_output_type())?;

            for answer in response.answers() {
                match answer.data() {
                    Some(RData::AAAA(ip)) => output.send(Row::new(vec![
                        Value::from(ip.to_string()),
                        Value::Duration(Duration::seconds(answer.ttl() as i64))]))?,
                    _ => return data_error("Missing AAAA record"),
                }
            }
        }
        "SRV" => {
            let response = to_crush_error(client.query(&to_crush_error(Name::from_str(&cfg.name))?, DNSClass::IN, RecordType::SRV))?;
            let output = context.output.initialize(&srv_stream_output_type())?;

            for answer in response.answers() {
                match answer.data() {
                    Some(RData::SRV(srv)) => output.send(Row::new(vec![
                        Value::from(srv.target().to_string()),
                        Value::Integer(srv.priority() as i128),
                        Value::Integer(srv.weight() as i128),
                        Value::Integer(srv.port() as i128),
                        Value::Duration(Duration::seconds(answer.ttl() as i64))]))?,
                    _ => return data_error("Missing A record"),
                }
            }
        }

        _ => return argument_error_legacy(format!("Unknown DNS record type {}", &cfg.record_type)),
    }
    Ok(())
}

fn query(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Query::parse(context.remove_arguments(), &context.global_state.printer())?;
    let rc = resolv_conf()?;
    let ns = rc.nameservers[0].to_string();
    let address = format!("{}:{}", cfg.nameserver.as_deref().unwrap_or_else(|| { &ns }), cfg.port).parse().unwrap();
    if cfg.tcp {
        let conn = TcpClientConnection::new(address).unwrap();
        query_internal(cfg, context, SyncClient::new(conn))
    } else {
        let conn = UdpClientConnection::new(address).unwrap();
        query_internal(cfg, context, SyncClient::new(conn))
    }
}

#[signature(
    dns.nameserver,
    can_block = true,
    short = "List of default nameservers",
)]
struct Nameserver {}

fn nameserver(context: CommandContext) -> CrushResult<()> {
    let rc = resolv_conf()?;
    context.output.send(
        List::new(
            ValueType::String,
            rc.nameservers.iter().map(|n| { Value::from(n.to_string()) }).collect::<Vec<_>>(),
        ).into())
}

#[signature(
    dns.search,
    can_block = true,
    short = "List of DNS search paths",
)]
struct Search {}

fn search(context: CommandContext) -> CrushResult<()> {
    let rc = resolv_conf()?;
    context.output.send(
        List::new(
            ValueType::String,
            rc.get_search()
                .map(|s| { s.iter().map(|n| { Value::from(n.to_string()) }).collect() })
                .unwrap_or(vec![]),
        ).into()
    )
}

#[signature(
    dns.domain,
    can_block = true,
    short = "DNS domain, if any",
    output = Known(ValueType::Any),
)]
struct Domain {}

fn domain(context: CommandContext) -> CrushResult<()> {
    let rc = resolv_conf()?;
    context.output.send(
        rc.get_domain()
            .map(|d| { Value::from(d) })
            .unwrap_or(Value::Empty))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "dns",
        "DNS querying and metadata",
        Box::new(move |dns| {
            Query::declare(dns)?;
            Nameserver::declare(dns)?;
            Search::declare(dns)?;
            Domain::declare(dns)?;
            Ok(())
        }))?;
    Ok(())
}
