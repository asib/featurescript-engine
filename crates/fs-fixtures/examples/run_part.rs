//! Runs one feature of a Feature Studio on the engine and writes its solid parts as a STEP and an STL.
//!
//! Usage: cargo run --release -p fs-fixtures --example run_part -- FILE.fs FEATURE OUT_STEM
//!
//! Writes OUT_STEM.step and OUT_STEM.stl (binary, meshed to 0.01 mm), and prints each part's volume and face count.

use fs_geom::model::BodyType;
use fs_interp::Value;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, file, feature, stem] = &args[..] else {
        eprintln!("usage: run_part FILE.fs FEATURE OUT_STEM");
        std::process::exit(2);
    };
    let (file, feature, stem) = (file.clone(), feature.clone(), stem.clone());
    let code = fs_interp::run_with_large_stack(move || match run(&file, &feature, &stem) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    });
    std::process::exit(code);
}

fn run(file: &str, feature: &str, stem: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut engine = fs_geom::engine();
    let module = engine.load_source(file, &source)?;
    let start = std::time::Instant::now();
    let exception = |e| format!("{e:?}");
    let context = engine.eval(module, "newContextWithDefaults(millimeter)").map_err(exception)?;
    let id = engine.eval(module, &format!("makeId(\"F{feature}\")")).map_err(exception)?;
    engine.call(module, feature, vec![context.clone(), id.clone(), Value::map(Default::default())]).map_err(exception)?;
    let status = engine.call(module, "getFeatureStatus", vec![context.clone(), id]).map_err(exception)?;
    print!("{}", engine.interp.output);
    if status.get("statusType").and_then(Value::as_str) != Some("OK") {
        return Err(format!("{feature} failed: {status:?}"));
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
    eprintln!("built {feature} in {:.1} s: {} parts", start.elapsed().as_secs_f64(), parts.len());
    let mut triangles = Vec::new();
    for (name, shape) in &parts {
        let volume = shape.volume().map_err(|e| e.to_string())? * 1e9;
        let faces = shape.sub_shapes(occt::ShapeKind::Face).len();
        println!("{name}: {volume:.3} mm³, {faces} faces");
        triangles.extend(occt::mesh_triangles(shape, 1e-5).map_err(|e| e.to_string())?);
    }
    let (names, shapes): (Vec<String>, Vec<occt::Shape>) = parts.into_iter().unzip();
    occt::write_step(&shapes, &names, &format!("{stem}.step")).map_err(|e| e.to_string())?;
    // Binary STL in millimetres.
    let mut stl = vec![0u8; 80];
    stl.extend((triangles.len() as u32).to_le_bytes());
    for [a, b, c] in &triangles {
        let (u, v) = ((b.x - a.x, b.y - a.y, b.z - a.z), (c.x - a.x, c.y - a.y, c.z - a.z));
        let n = (u.1 * v.2 - u.2 * v.1, u.2 * v.0 - u.0 * v.2, u.0 * v.1 - u.1 * v.0);
        let len = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-300);
        for x in [n.0 / len, n.1 / len, n.2 / len] {
            stl.extend((x as f32).to_le_bytes());
        }
        for p in [a, b, c] {
            for x in [p.x, p.y, p.z] {
                stl.extend(((x * 1000.0) as f32).to_le_bytes());
            }
        }
        stl.extend([0u8; 2]);
    }
    std::fs::File::create(format!("{stem}.stl")).and_then(|mut f| f.write_all(&stl)).map_err(|e| e.to_string())?;
    eprintln!("wrote {stem}.step and {stem}.stl ({} triangles)", triangles.len());
    Ok(())
}
