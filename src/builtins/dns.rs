use crate::data::list::List;
use crate::data::table::{ColumnType, Row};
use crate::lang::command::OutputType::Known;
use crate::lang::errors::data_error;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::{CrushResult, argument_error_legacy};
use chrono::Duration;
use signature::signature;
use std::fs::File;
use std::io::Read;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use trust_dns_client::client::{Client, ClientConnection, SyncClient};
use trust_dns_client::rr::{DNSClass, Name, RData, Record, RecordType};
use trust_dns_client::tcp::TcpClientConnection;
use trust_dns_client::udp::UdpClientConnection;

static A_STREAM_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("target", ValueType::String),
    ColumnType::new("ttl", ValueType::Duration),
];

static MX_STREAM_OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new("target", ValueType::String),
    ColumnType::new("preference", ValueType::Integer),
    ColumnType::new("ttl", ValueType::Duration),
];

static SOA_STREAM_OUTPUT_TYPE: [ColumnType; 7] = [
    ColumnType::new("mname", ValueType::String),
    ColumnType::new("rname", ValueType::String),
    ColumnType::new("serial", ValueType::Integer),
    ColumnType::new("refresh", ValueType::Duration),
    ColumnType::new("retry", ValueType::Duration),
    ColumnType::new("expire", ValueType::Duration),
    ColumnType::new("ttl", ValueType::Duration),
];

static SRV_STREAM_OUTPUT_TYPE: [ColumnType; 5] = [
    ColumnType::new("target", ValueType::String),
    ColumnType::new("priority", ValueType::Integer),
    ColumnType::new("weight", ValueType::Integer),
    ColumnType::new("port", ValueType::Integer),
    ColumnType::new("ttl", ValueType::Duration),
];

static TXT_STREAM_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("text", ValueType::Binary),
    ColumnType::new("ttl", ValueType::Duration),
];

#[signature(
    dns.query,
    can_block = true,
    short = "Look up a DNS record",
)]
struct Query {
    #[description("DNS record to look up.")]
    name: String,
    #[description("DNS record type.")]
    #[values("A", "AAAA", "CNAME", "MX", "NS", "PTR", "SOA", "SRV", "TXT")]
    #[default("A")]
    record_type: String,
    #[description("use TCP as the transport instead of UDP.")]
    #[default(false)]
    tcp: bool,
    #[description("the nameserver to talk to. If none is given, use the nameservers configured in `/etc/resolv.conf`.")]
    nameserver: Option<String>,
    #[description("port to talk to the nameserver on.")]
    #[default(53)]
    port: i128,
    #[description(
        "if a CNAME record is encountered, do not follow it. Show the actual CNAME record instead."
    )]
    #[default(false)]
    no_follow_cname: bool,
    #[description("connection timeout.")]
    #[default(Duration::seconds(5))]
    timeout: Duration,
}

impl Query {
    fn with_name(self, name: String) -> Self {
        Self { name, ..self }
    }
}

fn parse_resolv_conf() -> CrushResult<resolv_conf::Config> {
    let mut buf = Vec::with_capacity(8192);
    let mut f = File::open("/etc/resolv.conf")?;
    f.read_to_end(&mut buf)?;
    Ok(resolv_conf::Config::parse(&buf)?)
}

fn perform_query(
    cfg: Query,
    context: CommandContext,
    client: SyncClient<impl ClientConnection>,
    query_record_type: RecordType,
    output_signature: &[ColumnType],
    process_record_callback: fn(&Record) -> CrushResult<Row>,
) -> CrushResult<()> {
    let response = client.query(&Name::from_str(&cfg.name)?, DNSClass::IN, query_record_type)?;

    if let Some(answer) = response.answers().first() {
        if let Some(RData::CNAME(cname)) = answer.data() {
            if cfg.no_follow_cname || query_record_type == RecordType::CNAME {
                let output = context.output.initialize(&A_STREAM_OUTPUT_TYPE)?;
                return output.send(Row::new(vec![
                    Value::from(cname.to_string()),
                    Value::from(Duration::seconds(answer.ttl() as i64)),
                ]));
            }
            return query_internal(cfg.with_name(cname.to_string()), context, client);
        }
    }

    let output = context.output.initialize(output_signature)?;

    for answer in response.answers() {
        output.send(process_record_callback(answer)?)?;
    }
    Ok(())
}

fn query_internal(
    cfg: Query,
    context: CommandContext,
    client: SyncClient<impl ClientConnection>,
) -> CrushResult<()> {
    match cfg.record_type.as_ref() {
        "A" => perform_query(
            cfg,
            context,
            client,
            RecordType::A,
            &A_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::A(ip)) => Ok(Row::new(vec![
                    Value::from(ip.to_string()),
                    Value::from(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an A record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No A record found"),
            },
        ),
        "AAAA" => perform_query(
            cfg,
            context,
            client,
            RecordType::AAAA,
            &A_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::AAAA(ip)) => Ok(Row::new(vec![
                    Value::from(ip.to_string()),
                    Value::from(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an AAAA record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No AAAA record found"),
            },
        ),
        "CNAME" => perform_query(
            cfg,
            context,
            client,
            RecordType::CNAME,
            &A_STREAM_OUTPUT_TYPE,
            |_| data_error("Received an unexpected record."),
        ),
        "NS" => perform_query(
            cfg,
            context,
            client,
            RecordType::NS,
            &A_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::NS(ip)) => Ok(Row::new(vec![
                    Value::from(ip.to_string()),
                    Value::from(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an NS record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No NS record found"),
            },
        ),
        "MX" => perform_query(
            cfg,
            context,
            client,
            RecordType::MX,
            &MX_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::MX(mx)) => Ok(Row::new(vec![
                    Value::from(mx.exchange().to_string()),
                    Value::Integer(mx.preference() as i128),
                    Value::Duration(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an MX record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No MX record found"),
            },
        ),
        "PTR" => perform_query(
            cfg,
            context,
            client,
            RecordType::PTR,
            &A_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::PTR(ip)) => Ok(Row::new(vec![
                    Value::from(ip.to_string()),
                    Value::from(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an PTR record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No PTR record found"),
            },
        ),
        "SOA" => perform_query(
            cfg,
            context,
            client,
            RecordType::SOA,
            &SOA_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::SOA(soa)) => Ok(Row::new(vec![
                    Value::from(soa.mname().to_string()),
                    Value::from(soa.rname().to_string()),
                    Value::from(soa.serial()),
                    Value::from(Duration::seconds(soa.refresh() as i64)),
                    Value::from(Duration::seconds(soa.retry() as i64)),
                    Value::from(Duration::seconds(soa.expire() as i64)),
                    Value::from(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an SOA record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No SOA record found"),
            },
        ),
        "SRV" => perform_query(
            cfg,
            context,
            client,
            RecordType::SRV,
            &SRV_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::SRV(srv)) => Ok(Row::new(vec![
                    Value::from(srv.target().to_string()),
                    Value::Integer(srv.priority() as i128),
                    Value::Integer(srv.weight() as i128),
                    Value::Integer(srv.port() as i128),
                    Value::Duration(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an SRV record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No SRV record found"),
            },
        ),
        "TXT" => perform_query(
            cfg,
            context,
            client,
            RecordType::TXT,
            &TXT_STREAM_OUTPUT_TYPE,
            |answer| match answer.data() {
                Some(RData::TXT(txt)) => Ok(Row::new(vec![
                    Value::from(txt.txt_data()),
                    Value::Duration(Duration::seconds(answer.ttl() as i64)),
                ])),
                Some(r) => data_error(format!(
                    "Received an unexpected record. Wanted an TXT record, got a {}",
                    r.record_type().to_string()
                )),
                None => data_error("No TXT record found"),
            },
        ),
        _ => argument_error_legacy(format!("Unknown DNS record type {}", &cfg.record_type)),
    }
}

fn create_address(nameserver: &Option<String>, port: i128) -> CrushResult<SocketAddr> {
    let srv = match nameserver {
        None => parse_resolv_conf()?.nameservers.get(0)
            .ok_or("No nameservers configured")?
        .to_string(),

        Some(server) => server.to_string(),
    };

    Ok(format!("{}:{}", srv, port).parse()?)
}

fn query(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Query::parse(context.remove_arguments(), &context.global_state.printer())?;
    let address = create_address(&cfg.nameserver, cfg.port)?;
    let t = cfg.timeout.num_nanoseconds().map(|us| core::time::Duration::from_nanos(us as u64)).ok_or("Out of bounds timeout")?;
    if cfg.tcp {
        let conn = TcpClientConnection::with_timeout(address, t)?;
        query_internal(cfg, context, SyncClient::new(conn))
    } else {
        let conn = UdpClientConnection::with_timeout(address, t)?;
        query_internal(cfg, context, SyncClient::new(conn))
    }
}

#[signature(
    dns.query_reverse,
    can_block = true,
    short = "Perform a reverse DNS lookup on a given IP address",
)]
struct QueryReverse {
    #[description("IP address to look up. Can be either IPv4 or IPv6.")]
    address: String,
    #[description("Use TCP connection instead of UDP")]
    #[default(false)]
    tcp: bool,
    #[description("Override the nameserver to talk to")]
    nameserver: Option<String>,
    #[description("DNS port")]
    #[default(53)]
    port: i128,
    #[description("Connection timeout.")]
    #[default(Duration::seconds(5))]
    timeout: Duration,
}

fn query_reverse(mut context: CommandContext) -> CrushResult<()> {
    let cfg = QueryReverse::parse(context.remove_arguments(), &context.global_state.printer())?;
    let address = create_address(&cfg.nameserver, cfg.port)?;

    let t = cfg.timeout.num_nanoseconds().map(|us| core::time::Duration::from_nanos(us as u64)).ok_or("Out of bounds timeout")?;

    if cfg.tcp {
        query_reverse_internal(
            cfg,
            context,
            SyncClient::new(TcpClientConnection::with_timeout(address, t)?),
        )
    } else {
        query_reverse_internal(
            cfg,
            context,
            SyncClient::new(UdpClientConnection::with_timeout(address, t)?),
        )
    }
}

fn query_reverse_internal(
    cfg: QueryReverse,
    context: CommandContext,
    client: SyncClient<impl ClientConnection>,
) -> CrushResult<()> {
    let name = if cfg.address.contains(".") {
        Name::from(Ipv4Addr::from_str(&cfg.address)?)
    } else {
        Name::from(Ipv6Addr::from_str(&cfg.address)?)
    };

    let response = client.query(&name, DNSClass::IN, RecordType::PTR)?;

    match response.answers().first() {
        None => Ok(()),
        Some(answer) => match answer.data() {
            Some(RData::PTR(ip)) => context.output.send(Value::from(ip.to_string())),
            _ => data_error("Missing PTR record"),
        },
    }
}

#[signature(
    dns.nameserver,
    can_block = true,
    short = "List of default nameservers",
)]
struct Nameserver {}

fn nameserver(context: CommandContext) -> CrushResult<()> {
    let rc = parse_resolv_conf()?;
    context.output.send(
        List::new(
            ValueType::String,
            rc.nameservers
                .iter()
                .map(|n| Value::from(n.to_string()))
                .collect::<Vec<_>>(),
        )
        .into(),
    )
}

#[signature(
    dns.search_paths,
    can_block = true,
    short = "List of DNS search paths",
)]
struct SearchPaths {}

fn search_paths(context: CommandContext) -> CrushResult<()> {
    let rc = parse_resolv_conf()?;
    context.output.send(
        List::new(
            ValueType::String,
            rc.get_search()
                .map(|s| s.iter().map(|n| Value::from(n.to_string())).collect())
                .unwrap_or(vec![]),
        )
        .into(),
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
    let rc = parse_resolv_conf()?;
    context.output.send(
        rc.get_domain()
            .map(|d| Value::from(d))
            .unwrap_or(Value::Empty),
    )
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "dns",
        "DNS querying and metadata",
        Box::new(move |dns| {
            Query::declare(dns)?;
            QueryReverse::declare(dns)?;
            Nameserver::declare(dns)?;
            SearchPaths::declare(dns)?;
            Domain::declare(dns)?;
            Ok(())
        }),
    )?;
    Ok(())
}
