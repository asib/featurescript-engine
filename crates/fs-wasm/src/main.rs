//! The FeatureScript engine for WebAssembly (Emscripten). `fs_build` runs one feature of a Feature Studio in the
//! module's virtual file system and writes its solid parts beside it as an STL and a STEP; std is preloaded at /std.

use fs_geom::model::BodyType;
use fs_interp::Value;
use std::ffi::{CStr, c_char};

/// Runs `feature` from the Feature Studio at `path` and writes `<stem>.stl` (binary, millimetres, meshed to 0.01 mm),
/// `<stem>.step` and `<stem>.log` (what it printed, each part's volume, or the error). Returns 0, or 1 on an error.
///
/// # Safety
/// The arguments are NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fs_build(path: *const c_char, feature: *const c_char, stem: *const c_char) -> i32 {
    let text = |p: *const c_char| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    let (path, feature, stem) = (text(path), text(feature), text(stem));
    let (code, log) = match build(&path, &feature, &stem) {
        Ok(log) => (0, log),
        Err(e) => (1, e),
    };
    let _ = std::fs::write(format!("{stem}.log"), log);
    code
}

fn build(path: &str, feature: &str, stem: &str) -> Result<String, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut engine = fs_geom::engine_with_std("/std");
    let module = engine.load_source(path, &source)?;
    let exception = |e| format!("{e:?}");
    let context = engine.eval(module, "newContextWithDefaults(millimeter)").map_err(exception)?;
    let id = engine.eval(module, &format!("makeId(\"F{feature}\")")).map_err(exception)?;
    engine.call(module, feature, vec![context.clone(), id.clone(), Value::map(Default::default())]).map_err(exception)?;
    let status = engine.call(module, "getFeatureStatus", vec![context.clone(), id]).map_err(exception)?;
    let mut log = std::mem::take(&mut engine.interp.output);
    if status.get("statusType").and_then(Value::as_str) != Some("OK") {
        return Err(format!("{log}{feature} failed: {status:?}"));
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
    .ok_or("the context has no geometry")?;
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
fn stl(triangles: &[[occt::Vec3; 3]]) -> Vec<u8> {
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

fn main() {}
