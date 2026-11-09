//! Builds a batch on the engine and lists the faces of a named part: surface type, area and a point inside each;
//! then its volume.
//!
//! Usage: FS_ONLY=c012 cargo run --release -p fs-fixtures --example part_faces -- <batch dir> <part name>

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (batch, name) = (std::path::PathBuf::from(&args[1]), args[2].clone());
    fs_interp::run_with_large_stack(move || {
        let built = fs_fixtures::build(&batch).expect("the batch builds");
        for part in built.parts.iter().filter(|p| p.name == name) {
            for face in part.shape.sub_shapes(occt::ShapeKind::Face) {
                let kind = face.surface_type().map(|t| format!("{t:?}")).unwrap_or_default();
                let area = face.area().unwrap_or(f64::NAN) * 1e6;
                let p = face.face_inner_point().map(|p| [p.x * 1e3, p.y * 1e3, p.z * 1e3]).unwrap_or([f64::NAN; 3]);
                println!("{kind:10} area {area:12.6} mm² at ({:.3}, {:.3}, {:.3})", p[0], p[1], p[2]);
            }
            println!("volume {:.6} mm³", part.shape.volume().unwrap_or(f64::NAN) * 1e9);
        }
    });
}
