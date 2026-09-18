// Minimal canned-response DNS server for exercising src/builtins/dns.rs's
// UDP/TCP client code paths in tests -- same spirit as grpc-service/ssh-service,
// but a real DNS server doesn't need a client library or async runtime, just wire
// (de)serialization from trust-dns-proto and a plain std::net socket loop.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::str::FromStr;
use trust_dns_proto::op::{Message, MessageType, Query};
use trust_dns_proto::rr::rdata::{A, AAAA, CNAME, MX, NS, PTR, SOA, SRV, TXT};
use trust_dns_proto::rr::{Name, RData, Record, RecordType};

const TTL: u32 = 60;

/// The fake zone this server is authoritative for. Each test domain below exists
/// to exercise one record type (or code path) in src/builtins/dns.rs; see
/// tests/dns.crush for what each one actually checks.
fn answer(name: &Name, qtype: RecordType) -> Vec<Record> {
    let n = name.to_utf8().to_ascii_lowercase();
    match n.as_str() {
        "a.test." if qtype == RecordType::A => {
            vec![Record::from_rdata(name.clone(), TTL, RData::A(A::new(1, 2, 3, 4)))]
        }
        "aaaa.test." if qtype == RecordType::AAAA => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::AAAA(AAAA::new(0, 0, 0, 0, 0, 0, 0, 1)),
        )],
        // A CNAME record is returned for ANY query type against an aliased name,
        // matching real DNS behavior -- dns.rs's perform_query() is what decides
        // whether to follow it or return it as-is (no_follow_cname / qtype==CNAME).
        "cname.test." => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::CNAME(CNAME(Name::from_str("a.test.").unwrap())),
        )],
        // Points at itself, so following it never terminates -- exercises
        // dns.rs's MAX_CNAME_DEPTH guard against a CNAME loop.
        "loop.test." => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::CNAME(CNAME(name.clone())),
        )],
        "mx.test." if qtype == RecordType::MX => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::MX(MX::new(10, Name::from_str("mail.test.").unwrap())),
        )],
        "ns.test." if qtype == RecordType::NS => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::NS(NS(Name::from_str("ns1.test.").unwrap())),
        )],
        "soa.test." if qtype == RecordType::SOA => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::SOA(SOA::new(
                Name::from_str("ns1.test.").unwrap(),
                Name::from_str("hostmaster.test.").unwrap(),
                2026091401,
                3600,
                600,
                604800,
                60,
            )),
        )],
        "srv.test." if qtype == RecordType::SRV => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::SRV(SRV::new(1, 2, 8080, Name::from_str("target.test.").unwrap())),
        )],
        "txt.test." if qtype == RecordType::TXT => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::TXT(TXT::new(vec!["hello world".to_string()])),
        )],
        // Reverse zone for dns:query_reverse, whose test address is 1.2.3.4.
        "4.3.2.1.in-addr.arpa." if qtype == RecordType::PTR => vec![Record::from_rdata(
            name.clone(),
            TTL,
            RData::PTR(PTR(Name::from_str("ptr.test.").unwrap())),
        )],
        _ => vec![],
    }
}

fn respond(request: &[u8]) -> Vec<u8> {
    let query_msg = Message::from_vec(request).expect("failed to parse incoming DNS query");
    let mut response = Message::new();
    response.set_id(query_msg.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(query_msg.op_code());
    response.set_recursion_desired(query_msg.recursion_desired());
    response.set_recursion_available(false);
    response.set_authoritative(true);

    for query in query_msg.queries() {
        response.add_query(Query::clone(query));
        response.add_answers(answer(query.name(), query.query_type()));
    }

    response.to_vec().expect("failed to serialize DNS response")
}

fn serve_tcp(listener: TcpListener) {
    for stream in listener.incoming().flatten() {
        std::thread::spawn(|| handle_tcp(stream));
    }
}

fn handle_tcp(mut stream: TcpStream) {
    loop {
        let mut len_buf = [0u8; 2];
        if stream.read_exact(&mut len_buf).is_err() {
            return;
        }
        let len = u16::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        if stream.read_exact(&mut buf).is_err() {
            return;
        }
        let response = respond(&buf);
        let response_len = (response.len() as u16).to_be_bytes();
        if stream.write_all(&response_len).is_err() || stream.write_all(&response).is_err() {
            return;
        }
    }
}

/// This server runs forever, so it only ever stops via an external SIGTERM (see
/// tests/system.rs's stop_test_servers). The default disposition for SIGTERM
/// terminates the process without running Rust's atexit-registered cleanup, which is
/// what a coverage-instrumented build relies on to flush its profiling data -- so under
/// `cargo llvm-cov`, a SIGTERM-killed server always reports 0% coverage regardless of
/// what actually ran. Catching the signal on a dedicated thread and exiting normally via
/// `std::process::exit` runs that cleanup instead.
fn install_graceful_shutdown() {
    std::thread::spawn(|| {
        let mut signals = signal_hook::iterator::Signals::new([signal_hook::consts::SIGTERM])
            .expect("failed to register SIGTERM handler");
        signals.forever().next();
        std::process::exit(0);
    });
}

fn main() {
    install_graceful_shutdown();

    // Bind to an OS-assigned free port (UDP first) rather than a fixed one, so several
    // instances of this server (e.g. concurrent `cargo test` runs) never collide over
    // the same port -- then bind TCP explicitly to that same port number, since DNS
    // clients (and dns.rs's own `port` argument) expect one port to serve both
    // transports. The chosen port is announced on stdout (see main's own comment on the
    // print below) for tests/system.rs to read and hand to test scripts via DNS_PORT.
    let udp = UdpSocket::bind(("127.0.0.1", 0)).expect("failed to bind UDP socket");
    let port = udp
        .local_addr()
        .expect("failed to read UDP socket's local address")
        .port();
    let tcp =
        TcpListener::bind(("127.0.0.1", port)).expect("failed to bind TCP socket");

    // The one and only thing ever written to stdout: tests/system.rs reads exactly this
    // one line to learn which port got chosen and to know the sockets are bound and
    // ready to accept connections.
    println!("{}", port);
    std::io::stdout().flush().expect("failed to flush stdout");

    std::thread::spawn(move || serve_tcp(tcp));

    let mut buf = [0u8; 4096];
    loop {
        let (len, src) = udp.recv_from(&mut buf).expect("UDP recv failed");
        let response = respond(&buf[..len]);
        let _ = udp.send_to(&response, src);
    }
}
