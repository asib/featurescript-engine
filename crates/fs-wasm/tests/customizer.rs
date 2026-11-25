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

/// A STEP file's text without its header's FILE_NAME, which carries the time it was written.
fn step_body(path: &str) -> String {
    std::fs::read_to_string(path).unwrap().lines().filter(|l| !l.starts_with("FILE_NAME")).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_model_is_written_out_a_piece_at_a_time() {
    fs_interp::run_with_large_stack(|| {
        let (std_dir, path, source) = showcase();
        let dir = std::env::temp_dir().join(format!("fs-wasm-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stem = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let log = fs_wasm::build(&std_dir, &path, &source, "", "{}", &stem("built")).unwrap();
        let built = std::fs::read(stem("built") + ".stl").unwrap();

        let model = fs_wasm::model(&std_dir, &path, &source, "", "{}").unwrap();
        // A coarse mesh first, as a page shows it: fewer triangles, and the fine mesh after it is the one a build
        // writes, whatever was meshed before.
        let coarse = model.stl(0.05).unwrap();
        assert!(coarse.len() * 2 < built.len(), "{} against {}", coarse.len(), built.len());
        assert!(model.stl(0.01).unwrap() == built);
        assert!(model.stl(0.0).is_err());
        model.write_step(&(stem("model") + ".step")).unwrap();
        assert_eq!(step_body(&(stem("model") + ".step")), step_body(&(stem("built") + ".step")));
        assert_eq!(model.output.clone() + &model.volumes().unwrap(), log);
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn the_engine_kept_between_builds_builds_the_same() {
    fs_interp::run_with_large_stack(|| {
        let (std_dir, path, source) = showcase();
        let dir = std::env::temp_dir().join(format!("fs-wasm-kept-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stem = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let build = |name: &str, values: &str| fs_wasm::build(&std_dir, &path, &source, "", values, &stem(name));
        // On a new engine, then on the engine that has built the part with other values, failed to build it and
        // read its parameters.
        let first = build("first", "{}").unwrap();
        build("edited", r#"{"length": 150, "pins": 4, "label": "AGAIN"}"#).unwrap();
        assert!(build("refused", r#"{"edgeFinish": "WAVY"}"#).is_err());
        fs_wasm::parameters_json(&std_dir, &path, &source, "").unwrap();
        let again = build("again", "{}").unwrap();
        assert_eq!(again, first);
        assert!(std::fs::read(stem("again") + ".stl").unwrap() == std::fs::read(stem("first") + ".stl").unwrap());
        assert_eq!(step_body(&(stem("again") + ".step")), step_body(&(stem("first") + ".step")));
        // Another Feature Studio takes a new engine, and the first one after it another.
        let cube = std::fs::read_to_string(repo().join("crates/fs-wasm/rounded-cube.fs")).unwrap();
        let cube_log = fs_wasm::build(&std_dir, "rounded-cube.fs", &cube, "", "{}", &stem("cube")).unwrap();
        assert!(cube_log.contains("7855.841 mm³"), "{cube_log}");
        assert_eq!(build("after", "{}").unwrap(), first);
        let _ = std::fs::remove_dir_all(&dir);
    });
}
