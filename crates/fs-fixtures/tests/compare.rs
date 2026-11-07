//! The comparison must notice differences far smaller than any modelling mistake.

use fs_fixtures::compare;
use occt::{Frame, Shape, Vec3};

const XY: Frame =
    Frame { origin: Vec3::new(0.0, 0.0, 0.0), normal: Vec3::new(0.0, 0.0, 1.0), x: Vec3::new(1.0, 0.0, 0.0) };

/// A box from (x, y, 0) of size 10 × 20 × 5 mm, with its x length changed by `stretch` metres.
fn block(x: f64, y: f64, stretch: f64) -> Shape {
    let face = occt::plane_face(XY, x, x + 0.010 + stretch, y, y + 0.020).unwrap();
    occt::prism(&face, Vec3::new(0.0, 0.0, 0.005)).unwrap().shape()
}

#[test]
fn identical_shapes_match() {
    let result = compare("same", &block(0.0, 0.0, 0.0), &block(0.0, 0.0, 0.0));
    assert!(result.problems.is_empty(), "{:?}", result.problems);
    assert_eq!(result.difference, 0.0);
}

#[test]
fn a_slightly_longer_shape_doesnt_match() {
    // 0.5 µm longer: a relative change of 5e-5 in volume.
    let result = compare("longer", &block(0.0, 0.0, 0.5e-6), &block(0.0, 0.0, 0.0));
    let problems = result.problems.join("\n");
    assert!(problems.contains("volume"), "{problems}");
    assert!(problems.contains("differ"), "{problems}");
}

#[test]
fn a_moved_shape_doesnt_match() {
    // Same size, 5 µm along y: only the bounding box and the difference can tell.
    let result = compare("moved", &block(0.0, 5e-6, 0.0), &block(0.0, 0.0, 0.0));
    assert!(!result.problems.iter().any(|p| p.starts_with("volume")), "{:?}", result.problems);
    let problems = result.problems.join("\n");
    assert!(problems.contains("bounding box"), "{problems}");
    assert!(problems.contains("differ"), "{problems}");
}

#[test]
fn different_topology_doesnt_match() {
    // A cylinder of the same volume as the block.
    let radius = (0.010f64 * 0.020 / std::f64::consts::PI).sqrt();
    let disc = occt::sketch_regions(&[occt::circle_edge(XY, radius).unwrap()], XY).unwrap();
    let cylinder = occt::prism(&disc[0], Vec3::new(0.0, 0.0, 0.005)).unwrap().shape();
    let problems = compare("cylinder", &cylinder, &block(0.0, 0.0, 0.0)).problems.join("\n");
    assert!(problems.contains("faces"), "{problems}");
    assert!(problems.contains("seam"), "{problems}");
}
