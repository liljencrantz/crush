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

fn main() {
    let udp = UdpSocket::bind("127.0.0.1:0").expect("failed to bind UDP socket");
    let port = udp.local_addr().unwrap().port();
    let tcp = TcpListener::bind(("127.0.0.1", port)).expect("failed to bind TCP socket");

    // tests/system.rs synchronizes on this line before pointing the crush client at us.
    println!("PORT:{}", port);
    use std::io::Write as _;
    std::io::stdout().flush().unwrap();

    std::thread::spawn(move || serve_tcp(tcp));

    let mut buf = [0u8; 4096];
    loop {
        let (len, src) = udp.recv_from(&mut buf).expect("UDP recv failed");
        let response = respond(&buf[..len]);
        let _ = udp.send_to(&response, src);
    }
}
