use std::sync::LazyLock;
use netstat2::*;
use signature::signature;
use crate::lang::command::OutputType::Known;
use crate::lang::data::list::List;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};

static TCP_OUTPUT_TYPE: LazyLock<[ColumnType; 6]> = LazyLock::new(|| [
    ColumnType::new("local_address", ValueType::String),
    ColumnType::new("local_port", ValueType::Integer),
    ColumnType::new("remote_address", ValueType::String),
    ColumnType::new("remote_port", ValueType::Integer),
    ColumnType::new("pids", ValueType::List(Box::from(ValueType::Integer))),
    ColumnType::new("state", ValueType::String),
]);

#[signature(
    sockets.tcp,
    can_block = true,
    output = Known(ValueType::table_input_stream(TCP_OUTPUT_TYPE.as_ref())),
    short = "List open TCP sockets",
)]

struct TCP {}

fn tcp(mut context: CommandContext) -> CrushResult<()> {
    let _ = TCP::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.initialize_output(TCP_OUTPUT_TYPE.as_ref())?;

    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP;
    let sockets_info = get_sockets_info(af_flags, proto_flags)?;

    for si in sockets_info {
        match si.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp_si) => output.send(Row::new(vec![
                Value::from(tcp_si.local_addr.to_string()),
                Value::from(tcp_si.local_port),
                Value::from(tcp_si.remote_addr.to_string()),
                Value::from(tcp_si.remote_port),
                Value::List(List::new(ValueType::Integer, si.associated_pids.iter().map(|i| Value::from(*i)).collect::<Vec<_>>())),
                Value::from(tcp_si.state.to_string()),
            ]))?,
            ProtocolSocketInfo::Udp(_) => (),
        }
    }
    Ok(())
}

static UDP_OUTPUT_TYPE: LazyLock<[ColumnType; 3]> = LazyLock::new(|| [
    ColumnType::new("local_address", ValueType::String),
    ColumnType::new("local_port", ValueType::Integer),
    ColumnType::new("pids", ValueType::List(Box::from(ValueType::Integer))),
]);

#[signature(
    sockets.udp,
    can_block = true,
    output = Known(ValueType::table_input_stream(UDP_OUTPUT_TYPE.as_ref())),
    short = "List open UDP sockets",
)]

struct UDP {}

fn udp(mut context: CommandContext) -> CrushResult<()> {
    let _ = UDP::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.initialize_output(UDP_OUTPUT_TYPE.as_ref())?;

    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::UDP;
    let sockets_info = get_sockets_info(af_flags, proto_flags)?;

    for si in sockets_info {
        match si.protocol_socket_info {
            ProtocolSocketInfo::Tcp(_) => (),
            ProtocolSocketInfo::Udp(udp_si) => output.send(Row::new(vec![
                Value::from(udp_si.local_addr.to_string()),
                Value::from(udp_si.local_port),
                Value::List(List::new(ValueType::Integer, si.associated_pids.iter().map(|i| Value::from(*i)).collect::<Vec<_>>())),
            ]))?
        }
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "sockets",
        "List opened sockets",
        Some(
            "Lists the TCP and UDP sockets currently open on this machine -- the same \
             information tools like `netstat` or `ss` report. Read-only: this namespace can \
             tell you what's listening or connected, but (unlike `grpc:connect` or `io:http`) \
             has no way to open a connection of its own."
                .to_string(),
        ),
        Box::new(move |sockets| {
            TCP::declare(sockets)?;
            UDP::declare(sockets)?;
            Ok(())
        }),
    )?;
    Ok(())
}
