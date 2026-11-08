//! `@opRevolve`: revolving faces about an axis into solids.

use super::ShapeSet;
use crate::model::{BodyType, Cap, EntityType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field};
use crate::{occt_error, query, regen_error};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Shape, ShapeKind};
use std::f64::consts::TAU;
use std::rc::Rc;

/// An angle normalized to [0, 2π).
fn normalized(angle: f64) -> f64 {
    let a = angle.rem_euclid(TAU);
    if (TAU - a).abs() < 1e-12 { 0.0 } else { a }
}

pub fn revolve(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let found = query::evaluate(model, field(definition, "entities"))?;
    let faces: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Face).map(|r| model.shape(*r).clone()).collect();
    let edges: Vec<Shape> =
        found.iter().filter(|r| r.kind == EntityType::Edge).map(|r| model.shape(*r).clone()).collect();
    if faces.is_empty() && edges.is_empty() {
        return Err(regen_error("REVOLVE_SELECT_FACES"));
    }
    let axis = values::line(field(definition, "axis"), "opRevolve axis")?;
    let (start, sweep) = sweep_angles(definition)?;
    let full = sweep >= TAU - 1e-12;

    // Faces revolve into solids; edges, joined into wires, into sheets.
    let solid = !faces.is_empty();
    let profile = if !solid {
        occt::compound(&occt::connect_edges(&edges).map_err(occt_error)?)
    } else if faces.len() == 1 {
        faces[0].clone()
    } else {
        occt::merge_faces(&faces).map_err(occt_error)?.shape()
    };
    let profile = if start != 0.0 && !full {
        occt::transform(&profile, &rotation(axis, start)).map_err(occt_error)?.shape()
    } else {
        profile
    };
    let op = occt::revolve(&profile, axis, sweep).map_err(|_| regen_error("REVOLVE_FAILED"))?;
    let (mut start_cap, mut end_cap) = (ShapeSet::default(), ShapeSet::default());
    for face in op.first() {
        start_cap.insert_all(&face);
    }
    for face in op.last() {
        end_cap.insert_all(&face);
    }
    let created = Info::new(id.clone());
    let start_info =
        Rc::new(Info { created_by: id.clone(), sketch_entity: None, cap: Some(Cap::Start), dependency: None });
    let end_info = Rc::new(Info { created_by: id.clone(), sketch_entity: None, cap: Some(Cap::End), dependency: None });
    let special = |s: &Shape| {
        if end_cap.contains(s) {
            Some(end_info.clone())
        } else if start_cap.contains(s) {
            Some(start_info.clone())
        } else {
            None
        }
    };
    let (kind, body_type) =
        if solid { (ShapeKind::Solid, BodyType::Solid) } else { (ShapeKind::Shell, BodyType::Sheet) };
    let pieces = op.shape().sub_shapes(kind);
    if pieces.is_empty() {
        return Err(regen_error("REVOLVE_FAILED"));
    }
    for piece in pieces {
        model.add_body(NewBody::new(piece, body_type, created.clone()), &Origins::default(), &created, &special);
    }
    Ok(Value::UNDEFINED)
}

/// Where the revolve starts (radians from the profile, right-handed about the axis) and how far it sweeps.
///
/// std's revolve feature passes blind bounds as `startBoundAngle` and `endBoundAngle`, both measured forward from
/// the profile: the sweep runs forward from the start to the end, so a symmetric revolve of `a` starts at
/// `2π - a/2`. Older callers pass `angleForward` and `angleBack`: from `-angleBack` to `angleForward`, a full turn
/// when they're equal.
fn sweep_angles(definition: &Value) -> R<(f64, f64)> {
    let angle = |key: &str| -> R<Option<f64>> {
        match field(definition, key) {
            v if v.is_undefined() => Ok(None),
            v => values::angle(v, key).map(Some),
        }
    };
    if let Some(forward) = angle("angleForward")? {
        let forward = normalized(forward);
        let back = normalized(angle("angleBack")?.unwrap_or(0.0));
        let sweep = forward + back;
        if (forward - back).abs() < 1e-12 || sweep >= TAU - 1e-12 {
            return Ok((0.0, TAU));
        }
        return Ok((-back, sweep));
    }
    for key in ["endBound", "startBound"] {
        match field(definition, key).as_str() {
            None | Some("BLIND") => {}
            Some(other) => return Err(error(format!("@opRevolve: {key} {other} isn't supported yet"))),
        }
    }
    let end = angle("endBoundAngle")?.ok_or_else(|| error("@opRevolve: no endBoundAngle"))?;
    let mut start = angle("startBoundAngle")?.unwrap_or(0.0);
    if values::opt_bool(definition, "startOppositeDirection")?.unwrap_or(false) {
        start = -start;
    }
    let mut sweep = (end - start).rem_euclid(TAU);
    if sweep < 1e-12 {
        // Zero or a whole number of turns: a full revolve when the ends differ by a turn.
        sweep = if (end - start).abs() > 1e-12 { TAU } else { 0.0 };
    }
    if sweep == 0.0 {
        return Err(regen_error("REVOLVE_FAILED"));
    }
    Ok((start, sweep))
}

/// A rotation by `angle` radians, right-handed, about an axis.
pub fn rotation(axis: occt::Axis, angle: f64) -> occt::Transform {
    let (x, y, z) = (axis.direction.x, axis.direction.y, axis.direction.z);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    let r = [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ];
    let o = axis.origin;
    let mut m = [0.0; 12];
    for i in 0..3 {
        for j in 0..3 {
            m[i * 4 + j] = r[i][j];
        }
        // The origin stays put: translation = o - R o.
        let ro = r[i][0] * o.x + r[i][1] * o.y + r[i][2] * o.z;
        m[i * 4 + 3] = [o.x, o.y, o.z][i] - ro;
    }
    occt::Transform { m }
}
