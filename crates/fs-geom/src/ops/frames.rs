//! The coordinate systems Onshape gives cylinders and cones (`evSurfaceDefinition`), which depend on how they were
//! made (batch 007, c001–c006).

use crate::values::{add, cross, dot, normalize, scale, sub};
use occt::{Frame, Shape, Vec3};

/// A circle of a profile: its centre, normal and radius.
pub(crate) struct Circle {
    centre: Vec3,
    normal: Vec3,
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
                out.push(Circle { centre: c.frame.origin, normal: c.frame.normal, radius: c.radius1 });
            }
        }
    }
    out
}

/// The x axis Onshape gives an extruded cylinder or a revolved torus with z along `z`: y × z, y being the world's y
/// axis square to z, or the world's x if z is along y.
fn world_y_cross(z: Vec3) -> Option<Vec3> {
    let world_y = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    let y = normalize(sub(world_y, scale(z, dot(world_y, z))))
        .filter(|_| crate::values::norm(cross(world_y, z)) > 1e-9)
        .unwrap_or(Vec3 { x: 1.0, y: 0.0, z: 0.0 });
    normalize(cross(y, z))
}

/// A cylinder an extrude swept along `direction` from one of `circles`: its origin is on the axis at the face's far
/// end, z points back along the extrude, and x is y × z, y being the world's y axis square to the axis however the
/// circle was drawn (batch 060: cylinders about x sketched with their x along y or z both have x along +z; batch
/// 061: tilted axes), or the world's x if the axis is along y (batch 061 c004). A sheet's is the other way about:
/// its origin at the near end, z along the extrude (a quarter cylinder swept from an arc, batch 046).
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
    let x = world_y_cross(z)?;
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
        // A torus: z against the axis and x as an extruded cylinder's (batch 085: about z, x along -x; about x, along
        // +z), its centre the origin.
        4 => Some(Frame { origin: s.frame.origin, normal: z, x: world_y_cross(z)? }),
        _ => None,
    }
}

/// For a torus face with Onshape's coordinate system `frame`, the face fractions OCCT's own parameters give for
/// fractions of Onshape's (batch 085): Onshape's u runs from its x axis towards z × x, OCCT's from its own x towards
/// its y with z the other way, so u turns back; and Onshape's v starts at the tube's inner equator and runs the
/// other way round it from OCCT's, which starts at the outer one. Round a whole period each runs from its start.
pub(crate) fn torus_fractions(face: &Shape, frame: Frame) -> Option<impl Fn(f64, f64) -> (f64, f64)> {
    use std::f64::consts::{PI, TAU};
    let s = face.surface_definition().ok()?;
    if s.kind != 4 {
        return None;
    }
    let ((u0, v0), (u1, v1)) = face.face_uv_bounds().ok()?;
    let (xo, zo) = (s.frame.x, s.frame.normal);
    // OCCT's frame may be left-handed (a revolve's about x, batch 085 c004): y is where u runs from x.
    let mid = face.face_tangent_plane(0.5, 0.5).ok()?;
    let rel = sub(mid.origin, s.frame.origin);
    let radial = sub(rel, scale(zo, dot(rel, zo)));
    let yo = if dot(mid.x, cross(zo, radial)) >= 0.0 { cross(zo, xo) } else { cross(xo, zo) };
    // Onshape's z against the way OCCT's u turns: u runs back; with it, on.
    let flipped = dot(frame.normal, cross(xo, yo)) < 0.0;
    let theta = dot(frame.x, yo).atan2(dot(frame.x, xo));
    let (su, sv) = (u1 - u0, v1 - v0);
    let wrap = |x: f64| x.rem_euclid(TAU);
    // The tube's height runs along OCCT's z as r sin v, along Onshape's as -r sin v: v turns back where they agree.
    let v_flipped = dot(frame.normal, zo) < 0.0;
    let whole = |span: f64| span >= TAU - 1e-9;
    // Onshape's ranges: the images of OCCT's, from Onshape's zero round a whole period.
    let lo_u = if whole(su) {
        0.0
    } else if flipped {
        wrap(theta - u1)
    } else {
        wrap(u0 - theta)
    };
    let lo_v = if whole(sv) {
        0.0
    } else if v_flipped {
        wrap(PI - v1)
    } else {
        wrap(v0 - PI)
    };
    Some(move |fu: f64, fv: f64| {
        let (un, vn) = (lo_u + fu * su, lo_v + fv * sv);
        let uo = if flipped { theta - un } else { theta + un };
        let vo = if v_flipped { PI - vn } else { PI + vn };
        (wrap(uo - u0) / su, wrap(vo - v0) / sv)
    })
}

/// Onshape's parameters (as fractions of its face's ranges) at a point on a whole torus, cylinder or cone with
/// Onshape's coordinate system `frame` (batch 087 c009, c010: ray hits): u round from x towards z × x; on a torus
/// v from the tube's inner equator, on a cylinder or cone OCCT's along it. `None` for other faces and for faces
/// not all the way round.
pub(crate) fn revolved_fractions_at(face: &Shape, frame: Frame, p: Vec3) -> Option<(f64, f64)> {
    use std::f64::consts::TAU;
    let s = face.surface_definition().ok()?;
    let ((u0, v0), (u1, v1)) = face.face_uv_bounds().ok()?;
    if u1 - u0 < TAU - 1e-9 {
        return None;
    }
    // u starts at the world's y × z, as a torus's x lies (a cone's coordinate system has another x, but its
    // parameter starts there too: batch 087 c010); a cylinder keeps OCCT's.
    if !matches!(s.kind, 2 | 4) {
        return None;
    }
    let x = world_y_cross(frame.normal)?;
    let d = sub(p, frame.origin);
    let y = cross(frame.normal, x);
    let u = dot(d, y).atan2(dot(d, x)).rem_euclid(TAU) / TAU;
    match s.kind {
        4 => {
            if v1 - v0 < TAU - 1e-9 {
                return None;
            }
            let (major, minor) = (s.radius1, s.radius2);
            let radial = (dot(d, x).powi(2) + dot(d, y).powi(2)).sqrt();
            let v = (-dot(d, frame.normal) / minor).atan2((major - radial) / minor).rem_euclid(TAU) / TAU;
            Some((u, v))
        }
        _ => {
            // v along Onshape's z, which may run against OCCT's.
            let v = face.face_fraction(p).ok()?.1;
            Some((u, if dot(frame.normal, s.frame.normal) > 0.0 { v } else { 1.0 - v }))
        }
    }
}

/// For a whole cone with Onshape's coordinate system `frame`, the face fractions OCCT's parameters give for fractions
/// of Onshape's (batch 087 c010): u round from the world's y × z towards z × that, v along Onshape's z (from the
/// top of a revolved frustum, its z against the axis).
pub(crate) fn cone_fractions(face: &Shape, frame: Frame) -> Option<impl Fn(f64, f64) -> (f64, f64)> {
    use std::f64::consts::TAU;
    let s = face.surface_definition().ok()?;
    if s.kind != 2 {
        return None;
    }
    let ((u0, _), (u1, _)) = face.face_uv_bounds().ok()?;
    if u1 - u0 < TAU - 1e-9 {
        return None;
    }
    let (xo, zo) = (s.frame.x, s.frame.normal);
    let mid = face.face_tangent_plane(0.5, 0.5).ok()?;
    let rel = sub(mid.origin, s.frame.origin);
    let radial = sub(rel, scale(zo, dot(rel, zo)));
    let yo = if dot(mid.x, cross(zo, radial)) >= 0.0 { cross(zo, xo) } else { cross(xo, zo) };
    let x = world_y_cross(frame.normal)?;
    let y = cross(frame.normal, x);
    let along = dot(frame.normal, zo) > 0.0;
    Some(move |fu: f64, fv: f64| {
        let (sin, cos) = (TAU * fu).sin_cos();
        let dir = add(scale(x, cos), scale(y, sin));
        let theta = dot(dir, yo).atan2(dot(dir, xo));
        ((theta - u0).rem_euclid(TAU) / (u1 - u0), if along { fv } else { 1.0 - fv })
    })
}
