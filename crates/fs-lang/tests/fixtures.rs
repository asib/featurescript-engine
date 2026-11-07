//! The fixture case files must parse, since our engine runs them.

use std::fs;
use std::path::PathBuf;

#[test]
fn fixture_case_files_parse() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut checked = 0;
    for batch in fs::read_dir(&dir).unwrap() {
        let batch = batch.unwrap().path();
        for name in ["cases.fs", "generator.fs"] {
            let path = batch.join(name);
            if !path.exists() {
                continue;
            }
            let mut source = fs::read_to_string(&path).unwrap();
            if name == "generator.fs" {
                // In Onshape the generator is pasted after the cases, in the same Feature Studio.
                source = fs::read_to_string(batch.join("cases.fs")).unwrap() + &source;
            }
            if let Err(error) = fs_lang::parse_module(&source) {
                panic!("{}: {}", path.display(), error.describe(&source));
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "no fixture files found");
}
