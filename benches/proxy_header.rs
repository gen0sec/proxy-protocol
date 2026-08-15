//! Hot-path benchmarks for the PROXY protocol codec.
//!
//! Every accepted connection on the proxy front-end pays for exactly one
//! `parse`, and every upstream connection pays for exactly one `encode`, so
//! these run per-connection and are worth tracking.

use bytes::BytesMut;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use proxy_protocol::{ProxyHeader, encode, parse, version1, version2};
use std::hint::black_box;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

const V2_SIG: [u8; 12] = [
    0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A,
];

fn v2_header(family_proto: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16 + payload.len());
    buf.extend_from_slice(&V2_SIG);
    buf.push(0x21); // version 2, PROXY command
    buf.push(family_proto);
    buf.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

fn v2_inet() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&Ipv4Addr::new(192, 168, 0, 1).octets());
    p.extend_from_slice(&Ipv4Addr::new(10, 0, 0, 7).octets());
    p.extend_from_slice(&443u16.to_be_bytes());
    p.extend_from_slice(&51234u16.to_be_bytes());
    v2_header(0x11, &p) // AF_INET + STREAM
}

fn v2_inet6() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1).octets());
    p.extend_from_slice(&Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2).octets());
    p.extend_from_slice(&443u16.to_be_bytes());
    p.extend_from_slice(&51234u16.to_be_bytes());
    v2_header(0x21, &p) // AF_INET6 + STREAM
}

fn tlv(type_id: u8, value: &[u8]) -> Vec<u8> {
    let mut t = Vec::with_capacity(3 + value.len());
    t.push(type_id);
    t.extend_from_slice(&(value.len() as u16).to_be_bytes());
    t.extend_from_slice(value);
    t
}

/// AF_INET header carrying the TLV set a real edge sends: ALPN, authority
/// (SNI), unique-id and a CRC32C.
fn v2_inet_with_tlvs() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&Ipv4Addr::new(192, 168, 0, 1).octets());
    p.extend_from_slice(&Ipv4Addr::new(10, 0, 0, 7).octets());
    p.extend_from_slice(&443u16.to_be_bytes());
    p.extend_from_slice(&51234u16.to_be_bytes());
    p.extend_from_slice(&tlv(0x01, b"h2"));
    p.extend_from_slice(&tlv(0x02, b"api.example.com"));
    p.extend_from_slice(&tlv(0x05, b"0123456789abcdef"));
    p.extend_from_slice(&tlv(0x03, &0xdead_beefu32.to_be_bytes()));
    v2_header(0x11, &p)
}

fn parse_bench(c: &mut Criterion) {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "v1/tcp4",
            b"PROXY TCP4 192.168.0.1 10.0.0.7 51234 443\r\n".to_vec(),
        ),
        (
            "v1/tcp6",
            b"PROXY TCP6 2001:db8::1 2001:db8::2 51234 443\r\n".to_vec(),
        ),
        ("v1/unknown", b"PROXY UNKNOWN\r\n".to_vec()),
        ("v2/inet", v2_inet()),
        ("v2/inet6", v2_inet6()),
        ("v2/inet+tlvs", v2_inet_with_tlvs()),
    ];

    let mut group = c.benchmark_group("parse");
    for (name, bytes) in &cases {
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), bytes, |b, bytes| {
            b.iter(|| {
                let mut slice = &bytes[..];
                black_box(parse(black_box(&mut slice)).unwrap())
            })
        });
    }
    group.finish();
}

/// The rejection path matters too: a non-PROXY client hitting a
/// proxy-protocol listener must be cheap to reject.
fn parse_reject_bench(c: &mut Criterion) {
    let junk = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec();
    c.bench_function("parse/reject_non_proxy", |b| {
        b.iter(|| {
            let mut slice = &junk[..];
            black_box(parse(black_box(&mut slice)).unwrap_err())
        })
    });
}

fn encode_bench(c: &mut Criterion) {
    let v1_v4 = ProxyHeader::Version1 {
        addresses: version1::ProxyAddresses::Ipv4 {
            source: SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 51234),
            destination: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 7), 443),
        },
    };
    let v1_v6 = ProxyHeader::Version1 {
        addresses: version1::ProxyAddresses::Ipv6 {
            source: SocketAddrV6::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 51234, 0, 0),
            destination: SocketAddrV6::new(
                Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2),
                443,
                0,
                0,
            ),
        },
    };
    let v2_v4 = ProxyHeader::Version2 {
        command: version2::ProxyCommand::Proxy,
        transport_protocol: version2::ProxyTransportProtocol::Stream,
        addresses: version2::ProxyAddresses::Ipv4 {
            source: SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 51234),
            destination: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 7), 443),
        },
        extensions: vec![],
    };
    let v2_v4_tlvs = ProxyHeader::Version2 {
        command: version2::ProxyCommand::Proxy,
        transport_protocol: version2::ProxyTransportProtocol::Stream,
        addresses: version2::ProxyAddresses::Ipv4 {
            source: SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 51234),
            destination: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 7), 443),
        },
        extensions: vec![
            version2::ExtensionTlv::Alpn(b"h2".to_vec()),
            version2::ExtensionTlv::Authority("api.example.com".to_owned()),
            version2::ExtensionTlv::UniqueId(b"0123456789abcdef".to_vec()),
            version2::ExtensionTlv::Crc32c(0xdead_beef),
        ],
    };

    let mut group = c.benchmark_group("encode");
    for (name, header) in [
        ("v1/tcp4", &v1_v4),
        ("v1/tcp6", &v1_v6),
        ("v2/inet", &v2_v4),
        ("v2/inet+tlvs", &v2_v4_tlvs),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), header, |b, header| {
            b.iter(|| black_box(encode(black_box(header.clone())).unwrap()))
        });
    }
    group.finish();
}

/// Round-trip: what a connection actually costs end to end.
fn roundtrip_bench(c: &mut Criterion) {
    let header = ProxyHeader::Version2 {
        command: version2::ProxyCommand::Proxy,
        transport_protocol: version2::ProxyTransportProtocol::Stream,
        addresses: version2::ProxyAddresses::Ipv4 {
            source: SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 51234),
            destination: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 7), 443),
        },
        extensions: vec![],
    };
    c.bench_function("roundtrip/v2_inet", |b| {
        b.iter(|| {
            let buf: BytesMut = encode(black_box(header.clone())).unwrap();
            let mut slice = &buf[..];
            black_box(parse(&mut slice).unwrap())
        })
    });
}

criterion_group!(
    benches,
    parse_bench,
    parse_reject_bench,
    encode_bench,
    roundtrip_bench
);
criterion_main!(benches);
