//! The engine's customizer interface, as the WebAssembly module exports it (src/main.rs) and native tests use it: a
//! Feature Studio's features and parameters as JSON in the shape OpenSCAD's Customizer gives (so a site's parameter
//! panel can show either), and a feature built with edited values into an STL and a STEP.

use fs_geom::model::BodyType;
use fs_interp::features::{FeatureParameter, ParameterKind, ParameterValue};
use fs_interp::{Engine, Value};

/// The Feature Studio in `source` (at `path`, for its imports), loaded on an engine over std in `std_dir`.
fn load(std_dir: &str, path: &str, source: &str) -> Result<(Engine, fs_interp::module::ModuleId), String> {
    let mut engine = fs_geom::engine_with_std(std_dir);
    let module = engine.load_source(path, source)?;
    Ok((engine, module))
}

/// The feature to build: `feature`, or the first the module exports.
fn chosen(engine: &Engine, module: fs_interp::module::ModuleId, feature: &str) -> Result<String, String> {
    let features = engine.features(module);
    if feature.is_empty() {
        features.first().cloned().ok_or_else(|| "This FeatureScript defines no feature (an exported const defined by defineFeature).".to_string())
    } else if features.iter().any(|f| f == feature) {
        Ok(feature.to_string())
    } else {
        Err(format!("This FeatureScript has no feature named {feature}."))
    }
}

/// The features and the chosen one's parameters, as JSON:
/// `{"feature": ..., "features": [...], "params": [{"name", "caption", "description", "group", "type", "value",
/// "units", "control": {"kind", "min", "max", "step", "options"}, "condition"}]}`. Lengths are millimetres and angles
/// degrees; queries, which can't be edited, are left out.
pub fn parameters_json(std_dir: &str, path: &str, source: &str, feature: &str) -> Result<String, String> {
    let (mut engine, module) = load(std_dir, path, source)?;
    let feature = chosen(&engine, module, feature)?;
    let features = engine.features(module);
    let parameters = engine.feature_parameters(module, &feature)?;
    let params: Vec<String> = parameters.iter().filter(|p| p.kind != ParameterKind::Query).map(parameter_json).collect();
    Ok(format!(
        "{{\"feature\":{},\"features\":[{}],\"params\":[{}]}}",
        json_string(&feature),
        features.iter().map(|f| json_string(f)).collect::<Vec<_>>().join(","),
        params.join(",")
    ))
}

fn parameter_json(p: &FeatureParameter) -> String {
    let (ty, units, control) = match &p.kind {
        ParameterKind::Length | ParameterKind::Angle | ParameterKind::Real | ParameterKind::Integer => {
            let units = match p.kind {
                ParameterKind::Length => "mm",
                ParameterKind::Angle => "°",
                _ => "",
            };
            let step = if p.kind == ParameterKind::Integer { 1.0 } else { step_for(p) };
            let range = [("min", p.min), ("max", p.max)]
                .iter()
                .filter_map(|(k, v)| v.filter(|v| v.abs() < 1e9).map(|v| format!(",\"{k}\":{}", json_number(v))))
                .collect::<String>();
            ("number", units, format!("{{\"kind\":\"spinbox\",\"step\":{}{range}}}", json_number(step)))
        }
        ParameterKind::Boolean => ("bool", "", "{\"kind\":\"checkbox\"}".to_string()),
        ParameterKind::Text => ("string", "", "{\"kind\":\"text\"}".to_string()),
        ParameterKind::Choice { options, .. } => {
            let options: Vec<String> = options
                .iter()
                .map(|(v, l)| format!("{{\"value\":{},\"label\":{}}}", json_string(v), json_string(l)))
                .collect();
            ("string", "", format!("{{\"kind\":\"dropdown\",\"options\":[{}]}}", options.join(",")))
        }
        ParameterKind::Query => unreachable!("queries are left out"),
    };
    let condition = p
        .shown_when
        .as_ref()
        .map(|(n, v)| format!(",\"condition\":{{\"parameter\":{},\"value\":{}}}", json_string(n), value_json(v)))
        .unwrap_or_default();
    format!(
        "{{\"name\":{},\"caption\":{},\"description\":{},\"group\":\"Parameters\",\"type\":\"{ty}\",\"value\":{},\"units\":\"{units}\",\"control\":{control}{condition}}}",
        json_string(&p.name),
        json_string(&p.label),
        json_string(&p.description),
        value_json(&p.default)
    )
}

/// A step for a number's field: the power of ten below a twentieth of its range, or below a tenth of its default.
fn step_for(p: &FeatureParameter) -> f64 {
    let scale = match (&p.default, p.min, p.max) {
        (_, Some(lo), Some(hi)) if hi > lo && hi - lo < 1e6 => (hi - lo) / 20.0,
        (ParameterValue::Number(d), _, _) if *d != 0.0 => d.abs() / 10.0,
        _ => 1.0,
    };
    let step = 10f64.powf(scale.log10().floor());
    step.clamp(1e-3, 10.0)
}

/// Builds the feature with `values` (a JSON object of parameter values; a parameter missing takes its default) and
/// writes `<stem>.stl` (binary, millimetres, meshed to 0.01 mm) and `<stem>.step`. Returns what it printed and each
/// part's volume.
pub fn build(std_dir: &str, path: &str, source: &str, feature: &str, values: &str, stem: &str) -> Result<String, String> {
    let (mut engine, module) = load(std_dir, path, source)?;
    let feature = chosen(&engine, module, feature)?;
    let parameters = engine.feature_parameters(module, &feature)?;
    let values = parse_values(values)?;
    let definition = engine.feature_definition(module, &parameters, &values)?;
    let exception = |e| format!("{e:?}");
    let context = engine.eval(module, "newContextWithDefaults(millimeter)").map_err(exception)?;
    let id = engine.eval(module, &format!("makeId(\"F{feature}\")")).map_err(exception)?;
    engine.call(module, &feature, vec![context.clone(), id.clone(), definition]).map_err(exception)?;
    let status = engine.call(module, "getFeatureStatus", vec![context.clone(), id]).map_err(exception)?;
    let mut log = std::mem::take(&mut engine.interp.output);
    if status.get("statusType").and_then(Value::as_str) != Some("OK") {
        let reason = status.get("statusEnum").and_then(Value::as_str).unwrap_or("an error");
        return Err(format!("{log}The feature failed: {reason}."));
    }
    let parts: Vec<(String, occt::Shape)> = fs_geom::with_model(&context, |model| {
        model
            .bodies
            .iter()
            .filter(|b| b.body_type == BodyType::Solid && !b.construction)
            .enumerate()
            .map(|(i, b)| (b.name().map_or_else(|| format!("Part {}", i + 1), str::to_string), b.shape.clone()))
            .collect()
    })
    .ok_or("The feature made no geometry.")?;
    if parts.is_empty() {
        return Err(format!("{log}The feature made no solid parts."));
    }
    let mut triangles = Vec::new();
    for (name, shape) in &parts {
        let volume = shape.volume().map_err(|e| e.to_string())? * 1e9;
        log.push_str(&format!("{name}: {volume:.3} mm³\n"));
        triangles.extend(occt::mesh_triangles(shape, 1e-5).map_err(|e| e.to_string())?);
    }
    let (names, shapes): (Vec<String>, Vec<occt::Shape>) = parts.into_iter().unzip();
    occt::write_step(&shapes, &names, &format!("{stem}.step")).map_err(|e| e.to_string())?;
    std::fs::write(format!("{stem}.stl"), stl(&triangles)).map_err(|e| e.to_string())?;
    Ok(log)
}

/// A binary STL of the triangles, in millimetres.
pub fn stl(triangles: &[[occt::Vec3; 3]]) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    out.extend((triangles.len() as u32).to_le_bytes());
    for [a, b, c] in triangles {
        let (u, v) = ((b.x - a.x, b.y - a.y, b.z - a.z), (c.x - a.x, c.y - a.y, c.z - a.z));
        let n = (u.1 * v.2 - u.2 * v.1, u.2 * v.0 - u.0 * v.2, u.0 * v.1 - u.1 * v.0);
        let len = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-300);
        for x in [n.0 / len, n.1 / len, n.2 / len] {
            out.extend((x as f32).to_le_bytes());
        }
        for p in [a, b, c] {
            for x in [p.x, p.y, p.z] {
                out.extend(((x * 1000.0) as f32).to_le_bytes());
            }
        }
        out.extend([0u8; 2]);
    }
    out
}

fn value_json(v: &ParameterValue) -> String {
    match v {
        ParameterValue::Number(n) => json_number(*n),
        ParameterValue::Boolean(b) => b.to_string(),
        ParameterValue::Text(t) => json_string(t),
    }
}

fn json_number(n: f64) -> String {
    if n.is_finite() {
        // Shortest round-tripping text, without floating noise from unit conversions.
        let rounded = (n * 1e9).round() / 1e9;
        format!("{rounded}")
    } else {
        "null".to_string()
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A flat JSON object of numbers, booleans and strings.
pub fn parse_values(text: &str) -> Result<Vec<(String, ParameterValue)>, String> {
    let mut p = JsonParser { chars: text.chars().collect(), at: 0 };
    p.space();
    if p.peek().is_none() {
        return Ok(Vec::new());
    }
    p.expect('{')?;
    let mut out = Vec::new();
    p.space();
    if p.peek() == Some('}') {
        return Ok(out);
    }
    loop {
        p.space();
        let key = p.string()?;
        p.space();
        p.expect(':')?;
        p.space();
        let value = match p.peek() {
            Some('"') => ParameterValue::Text(p.string()?),
            Some('t') => {
                p.word("true")?;
                ParameterValue::Boolean(true)
            }
            Some('f') => {
                p.word("false")?;
                ParameterValue::Boolean(false)
            }
            _ => ParameterValue::Number(p.number()?),
        };
        out.push((key, value));
        p.space();
        match p.next() {
            Some(',') => continue,
            Some('}') => break,
            _ => return Err("parameter values: expected , or }".to_string()),
        }
    }
    Ok(out)
}

struct JsonParser {
    chars: Vec<char>,
    at: usize,
}

impl JsonParser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }
    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        self.at += 1;
        c
    }
    fn space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += 1;
        }
    }
    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.next() == Some(c) { Ok(()) } else { Err(format!("parameter values: expected {c}")) }
    }
    fn word(&mut self, w: &str) -> Result<(), String> {
        for c in w.chars() {
            self.expect(c)?;
        }
        Ok(())
    }
    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.next() {
                Some('"') => return Ok(out),
                Some('\\') => match self.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('u') => {
                        let hex: String = (0..4).filter_map(|_| self.next()).collect();
                        let code = u32::from_str_radix(&hex, 16).map_err(|_| "parameter values: bad \\u escape")?;
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    Some(c) => out.push(c),
                    None => return Err("parameter values: unfinished string".to_string()),
                },
                Some(c) => out.push(c),
                None => return Err("parameter values: unfinished string".to_string()),
            }
        }
    }
    fn number(&mut self) -> Result<f64, String> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')) {
            self.at += 1;
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse().map_err(|_| format!("parameter values: bad number {text:?}"))
    }
}
