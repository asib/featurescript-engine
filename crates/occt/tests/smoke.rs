use occt::{BooleanKind, Frame, ShapeKind, SurfaceType, Vec3};

const XY: Frame =
    Frame { origin: Vec3::new(0.0, 0.0, 0.0), normal: Vec3::new(0.0, 0.0, 1.0), x: Vec3::new(1.0, 0.0, 0.0) };

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * (1.0 + b.abs())
}

fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<occt::Shape> {
    let p = |x, y| Vec3::new(x, y, 0.0);
    vec![
        occt::line_edge(p(x0, y0), p(x1, y0)).unwrap(),
        occt::line_edge(p(x1, y0), p(x1, y1)).unwrap(),
        occt::line_edge(p(x1, y1), p(x0, y1)).unwrap(),
        occt::line_edge(p(x0, y1), p(x0, y0)).unwrap(),
    ]
}

#[test]
fn extrude_a_rectangle() {
    let regions = occt::sketch_regions(&rectangle(0.0, 0.0, 2.0, 3.0), XY).unwrap();
    assert_eq!(regions.len(), 1);
    assert!(close(regions[0].area().unwrap(), 6.0));
    let plane = regions[0].face_plane().unwrap();
    assert!(close(plane.normal.z, 1.0), "{plane:?}");

    let op = occt::prism(&regions[0], Vec3::new(0.0, 0.0, 4.0)).unwrap();
    let solid = op.shape();
    assert_eq!(solid.kind(), Some(ShapeKind::Solid));
    assert!(close(solid.volume().unwrap(), 24.0));
    assert_eq!(solid.sub_shapes(ShapeKind::Face).len(), 6);
    assert_eq!(solid.sub_shapes(ShapeKind::Edge).len(), 12);
    assert_eq!(solid.sub_shapes(ShapeKind::Vertex).len(), 8);
    assert!(solid.is_valid());

    // Each profile edge sweeps one side face; the caps are recorded separately.
    for edge in regions[0].sub_shapes(ShapeKind::Edge) {
        let side = op.generated(&edge);
        assert_eq!(side.len(), 1);
        assert_eq!(side[0].kind(), Some(ShapeKind::Face));
    }
    let (first, last) = (op.first(), op.last());
    assert_eq!((first.len(), last.len()), (1, 1));
    assert!(close(last[0].centroid().unwrap().z, 4.0));
    assert!(close(last[0].face_plane().unwrap().normal.z, 1.0));
    let bottom = solid.sub_shapes(ShapeKind::Face).into_iter().find(|f| f.is_same(&first[0])).unwrap();
    assert!(close(bottom.face_plane().unwrap().normal.z, -1.0), "bottom cap faces down");
}

#[test]
fn regions_with_holes_and_overlaps() {
    let mut edges = rectangle(-2.0, -2.0, 2.0, 2.0);
    edges.push(occt::circle_edge(XY, 1.0).unwrap());
    let regions = occt::sketch_regions(&edges, XY).unwrap();
    let mut areas: Vec<f64> = regions.iter().map(|r| r.area().unwrap()).collect();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas.len(), 2);
    assert!((areas[0] - std::f64::consts::PI).abs() < 1e-6, "{areas:?}");
    assert!((areas[1] - (16.0 - std::f64::consts::PI)).abs() < 1e-6, "{areas:?}");

    let mut edges = rectangle(0.0, 0.0, 2.0, 2.0);
    edges.extend(rectangle(1.0, 1.0, 3.0, 3.0));
    let regions = occt::sketch_regions(&edges, XY).unwrap();
    assert_eq!(regions.len(), 3);
    let merged = occt::merge_faces(&regions).unwrap().shape();
    let faces = merged.sub_shapes(ShapeKind::Face);
    assert_eq!(faces.len(), 1);
    assert!(close(faces[0].area().unwrap(), 7.0));
    assert_eq!(faces[0].sub_shapes(ShapeKind::Edge).len(), 8);
}

#[test]
fn boolean_history() {
    let a = occt::prism(&occt::plane_face(XY, 0.0, 2.0, 0.0, 2.0).unwrap(), Vec3::new(0.0, 0.0, 1.0)).unwrap().shape();
    let b = occt::prism(&occt::plane_face(XY, 1.0, 3.0, 0.0, 2.0).unwrap(), Vec3::new(0.0, 0.0, 1.0)).unwrap().shape();
    let fused = occt::boolean(BooleanKind::Union, std::slice::from_ref(&a), std::slice::from_ref(&b), true).unwrap();
    let solid = fused.shape();
    assert!(close(solid.volume().unwrap(), 6.0));
    // Unified: one box, not two boxes glued together.
    assert_eq!(solid.sub_shapes(ShapeKind::Face).len(), 6);
    let top_a = a.sub_shapes(ShapeKind::Face).into_iter().find(|f| close(f.centroid().unwrap().z, 1.0)).unwrap();
    let images = fused.modified(&top_a);
    assert_eq!(images.len(), 1);
    assert!(close(images[0].area().unwrap(), 6.0));

    let cut = occt::boolean(BooleanKind::Subtraction, &[a], &[b], false).unwrap().shape();
    assert!(close(cut.volume().unwrap(), 2.0));
    assert!(cut.sub_shapes(ShapeKind::Face).iter().all(|f| f.surface_type().unwrap() == SurfaceType::Plane));
}
