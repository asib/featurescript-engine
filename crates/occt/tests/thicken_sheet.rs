//! Thickening sheets as sheet metal does: an L of two walls, its corner rounded, thickened to one side.

use occt::{Shape, ShapeKind, Vec3};

fn l_sheet() -> Shape {
    let a = occt::line_edge(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.030, 0.0, 0.0)).unwrap();
    let b = occt::line_edge(Vec3::new(0.030, 0.0, 0.0), Vec3::new(0.030, 0.020, 0.0)).unwrap();
    let wire = occt::connect_edges(&[a, b]).unwrap().remove(0);
    occt::prism(&wire, Vec3::new(0.0, 0.0, 0.025)).unwrap().shape()
}

#[test]
fn thickens_a_rounded_l() {
    let sheet = l_sheet();
    let corner: Vec<Shape> = sheet
        .sub_shapes(ShapeKind::Edge)
        .into_iter()
        .filter(|e| {
            let p = e.edge_tangent(0.5, false).unwrap().origin;
            (p.x - 0.030).abs() < 1e-9 && p.y.abs() < 1e-9
        })
        .collect();
    assert_eq!(corner.len(), 1);
    let rounded = occt::fillet(&sheet, &corner, 0.002).unwrap().shape();
    for (t1, t2) in [(0.0015, 0.0), (0.0, 0.0015), (0.00075, 0.00075)] {
        let solid = occt::thicken_sheet(&rounded, t1, t2).unwrap();
        let faces = solid.sub_shapes(ShapeKind::Face).len();
        let volume = solid.volume().unwrap();
        eprintln!("t1 {t1} t2 {t2}: valid {} faces {faces} volume {volume}", solid.is_valid());
        assert!(solid.is_valid());
        assert_eq!(faces, 14);
    }
}

#[test]
fn union_of_sheets_reports_the_shared_edge() {
    use occt::{BooleanKind, Frame};
    let plate = occt::plane_face(
        Frame { origin: Vec3::new(0.0, 0.0, 0.0), normal: Vec3::new(0.0, 0.0, 1.0), x: Vec3::new(1.0, 0.0, 0.0) },
        0.0,
        0.04,
        0.0,
        0.03,
    )
    .unwrap();
    let wall = occt::plane_face(
        Frame { origin: Vec3::new(0.04, 0.0, 0.0), normal: Vec3::new(1.0, 0.0, 0.0), x: Vec3::new(0.0, 0.0, 1.0) },
        0.0,
        0.015,
        0.0,
        0.03,
    )
    .unwrap();
    let on_line = |e: &Shape| {
        let p = e.edge_tangent(0.5, false).unwrap().origin;
        (p.x - 0.04).abs() < 1e-9 && p.z.abs() < 1e-9
    };
    let wall_edge = wall.sub_shapes(ShapeKind::Edge).into_iter().find(|e| on_line(e)).expect("the wall's edge");
    let plate_edge = plate.sub_shapes(ShapeKind::Edge).into_iter().find(|e| on_line(e)).expect("the plate's edge");
    let op = occt::boolean(BooleanKind::Union, std::slice::from_ref(&plate), std::slice::from_ref(&wall), true).unwrap();
    let result = op.shape();
    let in_result = |e: &Shape| result.sub_shapes(ShapeKind::Edge).iter().any(|r| r.is_same(e));
    eprintln!(
        "plate edge kept {} modified {} deleted {}; wall edge kept {} modified {} deleted {}",
        in_result(&plate_edge),
        op.modified(&plate_edge).len(),
        op.is_deleted(&plate_edge),
        in_result(&wall_edge),
        op.modified(&wall_edge).len(),
        op.is_deleted(&wall_edge)
    );
}
