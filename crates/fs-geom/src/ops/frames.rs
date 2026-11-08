//! The coordinate systems Onshape gives cylinders and cones (`evSurfaceDefinition`), which depend on how they were
//! made (batch 007, c001–c006).

use crate::values::{add, cross, dot, normalize, scale, sub};
use occt::{Frame, Shape, Vec3};

/// A circle of a profile: its centre, normal, x direction and radius.
pub(crate) struct Circle {
    centre: Vec3,
    normal: Vec3,
    x: Vec3,
    radius: f64,
}

/// The circles among some profiles' edges.
pub(crate) fn circles(profiles: &[Shape]) -> Vec<Circle> {
    let mut out = Vec::new();
    for profile in profiles {
        for edge in profile.sub_shapes(occt::ShapeKind::Edge) {
            if edge.curve_type().ok() != Some(occt::CurveType::Circle) {
                continue;
            }
            if let Ok(c) = edge.curve_definition() {
                out.push(Circle { centre: c.frame.origin, normal: c.frame.normal, x: c.frame.x, radius: c.radius1 });
            }
        }
    }
    out
}

/// A cylinder an extrude swept along `direction` from one of `circles`: its origin is on the axis at the face's far
/// end, z points back along the extrude, and x is the circle's y × z. A sheet's is the other way about: its origin
/// at the near end, z along the extrude (a quarter cylinder swept from an arc, batch 046).
pub(crate) fn extruded(face: &Shape, circles: &[Circle], direction: Vec3, sheet: bool) -> Option<Frame> {
    let s = face.surface_definition().ok()?;
    if s.kind != 1 {
        return None;
    }
    let (p, a) = (s.frame.origin, s.frame.normal);
    let circle = circles.iter().find(|c| {
        let rel = sub(c.centre, p);
        let off_axis = sub(rel, scale(a, dot(rel, a)));
        (c.radius - s.radius1).abs() < 1e-9
            && dot(off_axis, off_axis) < 1e-18
            && crate::values::norm(cross(c.normal, a)) < 1e-9
    })?;
    let z = if sheet { direction } else { scale(direction, -1.0) };
    let x = normalize(cross(cross(circle.normal, circle.x), z))?;
    let local = Frame { origin: circle.centre, normal: direction, x: crate::values::canonical_x(direction) };
    let bounds = face.bounding_box_in(local, true).ok()?;
    let along = if sheet { bounds.min.z } else { bounds.max.z };
    Some(Frame { origin: add(circle.centre, scale(direction, along)), normal: z, x })
}

/// A cylinder or cone revolved about `axis` from a profile through `profile_point`. Both have z against the axis. A
/// cylinder's origin is the axis's and its x points from the profile to the axis; a cone's origin stays at its apex
/// and its y points from the axis to the profile (one case each so far).
pub(crate) fn revolved(face: &Shape, axis: occt::Axis, profile_point: Vec3) -> Option<Frame> {
    let s = face.surface_definition().ok()?;
    let rel = sub(profile_point, axis.origin);
    let radial = normalize(sub(rel, scale(axis.direction, dot(rel, axis.direction))))?;
    let z = scale(axis.direction, -1.0);
    match s.kind {
        1 => Some(Frame { origin: axis.origin, normal: z, x: scale(radial, -1.0) }),
        2 => Some(Frame { origin: s.frame.origin, normal: z, x: cross(axis.direction, radial) }),
        _ => None,
    }
}
