//! Parses every file of the vendored std.

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
