//! Writes the STEP file fixture batches import with `importForeign` (uploaded to the fixture document through
//! Onshape's UI): a 40 × 30 × 10 mm block from the origin named "Block", and a ball of radius 8 mm at
//! (60, 15, 8) mm named "Ball".
//!
//! Usage: cargo run --release -p fs-fixtures --example make_blob_step -- fixtures/blobs/fixture-parts.step

use occt::{Frame, Vec3};

fn main() {
    let path = std::env::args().nth(1).expect("usage: make_blob_step <file.step>");
    let mm = 1e-3;
    let base =
        Frame { origin: Vec3::new(0.0, 0.0, 0.0), normal: Vec3::new(0.0, 0.0, 1.0), x: Vec3::new(1.0, 0.0, 0.0) };
    let rectangle = occt::plane_face(base, 0.0, 40.0 * mm, 0.0, 30.0 * mm).expect("a rectangle");
    let block = occt::prism(&rectangle, Vec3::new(0.0, 0.0, 10.0 * mm)).expect("a block").shape();
    let ball = occt::sphere(Vec3::new(60.0 * mm, 15.0 * mm, 8.0 * mm), 8.0 * mm).expect("a ball");
    occt::write_step(&[block, ball], &["Block".to_string(), "Ball".to_string()], &path).expect("written");
    println!("wrote {path}");
}
