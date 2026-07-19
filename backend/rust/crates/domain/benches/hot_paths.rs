//! Microbenchmarks for the per-request crypto on the two external hot paths:
//! node polling (`/api/v1/server/*`, every request re-verifies its token) and
//! method-2 subscribe-token derivation. These guard against performance
//! regressions in code that runs on every node heartbeat; they are not run in
//! CI (clippy `--all-targets` keeps them compiling). Run them inside Docker:
//!
//!   make shell
//!   cargo bench -p v2board-domain

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use v2board_domain::server_credentials::{derive_node_token, verify_node_token};
use v2board_domain::subscribe_link::hmac_sha1_hex;

const MASTER_KEY: &str = "a sufficiently long server master key for benchmarks";

fn node_token(c: &mut Criterion) {
    let token = derive_node_token(MASTER_KEY, "v2node", 7, 3).expect("derivable bench token");
    c.bench_function("derive_node_token", |b| {
        b.iter(|| {
            derive_node_token(
                black_box(MASTER_KEY),
                black_box("v2node"),
                black_box(7),
                black_box(3),
            )
        })
    });
    c.bench_function("verify_node_token", |b| {
        b.iter(|| {
            verify_node_token(
                black_box(MASTER_KEY),
                black_box("v2node"),
                black_box(7),
                black_box(3),
                black_box(&token),
            )
        })
    });
}

fn subscribe_token(c: &mut Criterion) {
    let counter_bytes = [0_u8, 0, 0, 0, 0, 1, 226, 64];
    c.bench_function("hmac_sha1_hex", |b| {
        b.iter(|| {
            hmac_sha1_hex(
                black_box(b"benchmark-subscribe-token"),
                black_box(&counter_bytes),
            )
        })
    });
}

criterion_group!(benches, node_token, subscribe_token);
criterion_main!(benches);
