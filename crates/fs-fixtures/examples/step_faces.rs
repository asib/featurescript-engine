//! Lists the faces of a named part in a STEP file: surface type, area and a point inside each; then its volume.
//!
//! Usage: cargo run --release -p fs-fixtures --example step_faces -- <file.step> <part name>

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (file, name) = (&args[1], &args[2]);
    for (part, shape) in occt::read_step(file).expect("readable STEP") {
        if &part != name {
            if name == "?" {
                println!("{part}");
            }
            continue;
        }
        for face in shape.sub_shapes(occt::ShapeKind::Face) {
            let kind = face.surface_type().map(|t| format!("{t:?}")).unwrap_or_default();
            let area = face.area().unwrap_or(f64::NAN) * 1e6;
            let p = face.face_inner_point().map(|p| [p.x * 1e3, p.y * 1e3, p.z * 1e3]).unwrap_or([f64::NAN; 3]);
            let edges = face.sub_shapes(occt::ShapeKind::Edge).len();
            println!("{kind:10} area {area:12.6} mm² edges {edges:2} at ({:.3}, {:.3}, {:.3})", p[0], p[1], p[2]);
        }
        println!("volume {:.6} mm³", shape.volume().unwrap_or(f64::NAN) * 1e9);
    }
}
