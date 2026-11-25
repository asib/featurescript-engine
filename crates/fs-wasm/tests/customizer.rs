//! The customizer interface natively: the showcase part's parameters as JSON, and the part built with its defaults
//! and with edited values.

use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn showcase() -> (String, String, String) {
    let path = repo().join("examples/showcase.fs");
    let source = std::fs::read_to_string(&path).unwrap();
    (repo().join("std").to_string_lossy().into_owned(), path.to_string_lossy().into_owned(), source)
}

#[test]
fn showcase_parameters() {
    fs_interp::run_with_large_stack(|| {
        let (std_dir, path, source) = showcase();
        let json = fs_wasm::parameters_json(&std_dir, &path, &source, "").unwrap();
        for expected in [
            r#""feature":"showcase""#,
            r#"{"name":"length","caption":"Plate length","description":"","group":"Parameters","type":"number","value":120,"units":"mm","control":{"kind":"spinbox","step":1,"min":110,"max":200}}"#,
            r#""name":"edgeFinish","caption":"Top edges""#,
            r#""options":[{"value":"ROUND","label":"Rounded"},{"value":"CHAMFER","label":"Chamfered"}]"#,
            r#""name":"pins","caption":"Pins","description":"","group":"Parameters","type":"number","value":8"#,
            r#""name":"holes","caption":"Counterbored holes","description":"","group":"Parameters","type":"bool","value":true"#,
            r#""name":"handleRadius","caption":"Handle thickness""#,
            r#""condition":{"parameter":"handle","value":true}"#,
            r#""name":"label","caption":"Label","description":"","group":"Parameters","type":"string","value":"FS ENGINE""#,
        ] {
            assert!(json.contains(expected), "missing {expected} in {json}");
        }
    });
}

#[test]
fn showcase_builds_with_edited_values() {
    fs_interp::run_with_large_stack(|| {
        let (std_dir, path, source) = showcase();
        let dir = std::env::temp_dir().join(format!("fs-wasm-customizer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stem = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let volume = |log: &str| -> f64 {
            log.lines().find_map(|l| l.strip_prefix("Desk tidy: ")).unwrap().trim_end_matches(" mm³").parse().unwrap()
        };
        let defaults = fs_wasm::build(&std_dir, &path, &source, "", "{}", &stem("defaults")).unwrap();
        assert!((volume(&defaults) - 109322.24).abs() < 0.01, "{defaults}");
        let edited = r#"{"length": 160, "pins": 5, "edgeFinish": "CHAMFER", "handle": false, "holes": false, "label": "HELLO"}"#;
        let log = fs_wasm::build(&std_dir, &path, &source, "showcase", edited, &stem("edited")).unwrap();
        assert!(volume(&log) > volume(&defaults) + 10000.0, "{log}");
        assert!(std::fs::metadata(stem("edited") + ".stl").unwrap().len() > 84);
        assert!(std::fs::metadata(stem("edited") + ".step").unwrap().len() > 1000);
        let refused = fs_wasm::build(&std_dir, &path, &source, "showcase", r#"{"edgeFinish": "WAVY"}"#, &stem("refused"));
        assert_eq!(refused.unwrap_err(), "edgeFinish must be one of its choices");
        std::fs::remove_dir_all(dir).ok();
    });
}
