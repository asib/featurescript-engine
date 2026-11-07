//! Times lexing and parsing all of the vendored std.
//!
//! Run with `cargo run --release -p fs-lang --example parse_std`.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const ROUNDS: u32 = 20;

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../std");
    let mut sources = Vec::new();
    for entry in fs::read_dir(&dir).expect("std directory") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "fs") {
            sources.push(fs::read_to_string(&path).unwrap());
        }
    }
    let bytes: usize = sources.iter().map(String::len).sum();
    let lines: usize = sources.iter().map(|s| s.lines().count()).sum();
    let tokens: usize = sources.iter().map(|s| fs_lang::lexer::tokenize(s).unwrap().len()).sum();

    let time = |f: &dyn Fn()| {
        f(); // warm up
        let start = Instant::now();
        for _ in 0..ROUNDS {
            f();
        }
        start.elapsed() / ROUNDS
    };
    let lex = time(&|| {
        for source in &sources {
            std::hint::black_box(fs_lang::lexer::tokenize(source).unwrap());
        }
    });
    let parse = time(&|| {
        for source in &sources {
            std::hint::black_box(fs_lang::parse_module(source).unwrap());
        }
    });

    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("{} files, {lines} lines, {:.1} MB, {tokens} tokens", sources.len(), bytes as f64 / 1e6);
    println!("lex only:     {:7.1} ms", ms(lex));
    println!("lex + parse:  {:7.1} ms  ({:.0} lines/ms)", ms(parse), lines as f64 / ms(parse));
}
