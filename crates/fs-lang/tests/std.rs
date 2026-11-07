//! Parses every file of the vendored std, and checks that printing and re-parsing gives back the same tree.

use std::fs;
use std::path::PathBuf;

fn std_files() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../std");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("can't read {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "fs"))
        .map(|path| (path.file_name().unwrap().to_string_lossy().into_owned(), fs::read_to_string(&path).unwrap()))
        .collect();
    files.sort();
    files
}

/// A syntax tree dump with the spans (printed as `start..end`) blanked out, so trees from different source text
/// compare equal when only positions differ.
fn shape(tree: &impl std::fmt::Debug) -> String {
    let dump = format!("{tree:?}");
    let bytes = dump.as_bytes();
    let mut out = String::with_capacity(dump.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'.') {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if dump[j..].starts_with("..") && bytes.get(j + 2).is_some_and(u8::is_ascii_digit) {
                let mut k = j + 2;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                out.push('_');
                i = k;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

#[test]
fn std_has_the_expected_number_of_files() {
    assert_eq!(std_files().len(), 277);
}

#[test]
fn parses_all_of_std() {
    let mut failures = Vec::new();
    for (name, source) in std_files() {
        if let Err(error) = fs_lang::parse_module(&source) {
            failures.push(format!("{name}:{}", error.describe(&source)));
        }
    }
    assert!(failures.is_empty(), "{} files failed to parse:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn std_round_trips_through_the_printer() {
    let mut failures = Vec::new();
    for (name, source) in std_files() {
        let Ok(tree) = fs_lang::parse_module(&source) else { continue };
        let printed = fs_lang::print_module(&tree);
        match fs_lang::parse_module(&printed) {
            Err(error) => failures.push(format!("{name}: printed source doesn't parse: {}", error.describe(&printed))),
            Ok(reparsed) => {
                let (before, after) = (shape(&tree), shape(&reparsed));
                if before != after {
                    let at = before.bytes().zip(after.bytes()).take_while(|(a, b)| a == b).count();
                    let context = |s: &str| s[at.saturating_sub(150)..(at + 100).min(s.len())].to_string();
                    failures.push(format!(
                        "{name}: re-parsed tree differs\n  original: …{}…\n  re-parsed: …{}…",
                        context(&before),
                        context(&after)
                    ));
                } else if fs_lang::print_module(&reparsed) != printed {
                    failures.push(format!("{name}: printing isn't stable"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} files failed the round trip:\n{}", failures.len(), failures.join("\n"));
}
