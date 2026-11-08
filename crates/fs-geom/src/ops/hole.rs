//! `@opHole`: hole tools revolved from a list of profiles, each placed along its axis relative to where the hole
//! meets the targets, and cut from the targets.

use std::rc::Rc;

use super::{boolean_bodies, owner_bodies};
use crate::model::{BodyType, FeatureId, Info, Model, NewBody, Origins, Tid};
use crate::query::{self, tid_string};
use crate::regen_error;
use crate::values::{self, add, cross, dot, field, map, norm, normalize, scale, sub};
use fs_interp::Value;
use fs_interp::eval::{R, error};
use occt::{Axis, BooleanKind, Frame, Shape, ShapeKind, Vec3};

/// One profile of a hole definition (`HoleProfile`).
struct Profile {
    reference: Rc<str>,
    before: bool,
    position: f64,
    radius: f64,
    name: Option<Rc<str>>,
}

/// Where the infinite hole cylinder meets one target, as distances along the axis.
#[derive(Clone, Copy)]
struct Extremes {
    first_entrance: f64,
    full_entrance: f64,
    first_exit: f64,
    full_exit: f64,
}

/// A reference's range along the axis and the target that defines it.
struct Reference {
    start: f64,
    end: f64,
    target: Option<usize>,
}

pub fn hole(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let hole_definition = field(definition, "holeDefinition");
    let mut profiles = Vec::new();
    for p in field(hole_definition, "profiles").as_array().map(|a| a.to_vec()).unwrap_or_default() {
        let reference: Rc<str> = field(&p, "positionReference").as_str().unwrap_or("AXIS_POINT").into();
        let matched = field(&p, "profileType").as_str() == Some("MATCHED");
        let position = if matched { 0.0 } else { values::length(field(&p, "position"), "opHole profile position")? };
        profiles.push(Profile {
            reference,
            before: !matched && field(&p, "beforeReference").as_bool() == Some(true),
            position,
            radius: values::length(field(&p, "radius"), "opHole profile radius")?,
            name: field(&p, "name").as_str().map(Rc::from),
        });
    }
    if profiles.is_empty() {
        return Err(error("@opHole: no profiles"));
    }
    let face_names: Vec<Option<Rc<str>>> = match field(hole_definition, "faceNames").as_array() {
        Some(names) => names.iter().map(|n| n.as_str().map(Rc::from)).collect(),
        None => vec![None; profiles.len()],
    };
    let axes: Vec<Axis> = match field(definition, "axes").as_array() {
        Some(items) => items.iter().map(|a| values::line(a, "opHole axes")).collect::<R<_>>()?,
        None => return Err(error("@opHole: axes must be an array of lines")),
    };
    let identities: Vec<Option<Tid>> = match field(definition, "identities").as_array() {
        Some(items) => {
            items.iter().map(|q| Ok(query::evaluate(model, q)?.first().map(|r| model.tid(*r)))).collect::<R<_>>()?
        }
        None => vec![None; axes.len()],
    };
    let bodies = |key: &str| -> R<Vec<usize>> {
        match field(definition, key) {
            v if v.is_undefined() => Ok(Vec::new()),
            v => Ok(owner_bodies(model, v)?
                .into_iter()
                .filter(|&b| model.bodies[b].body_type == BodyType::Solid)
                .collect()),
        }
    };
    let targets = bodies("targets")?;
    let excluded = bodies("targetsToExcludeFromSubtraction")?;
    let subtract = values::opt_bool(definition, "subtractFromTargets")?.unwrap_or(true);
    let keep_tools = values::opt_bool(definition, "keepTools")?.unwrap_or(false);
    let profile_names: Vec<Option<Rc<str>>> = profiles.iter().map(|p| p.name.clone()).collect();
    let max_radius = profiles.iter().map(|p| p.radius).fold(0.0, f64::max);

    let mut results = Vec::new();
    let mut tools = Vec::new();
    for (i, axis) in axes.iter().enumerate() {
        let direction = normalize(axis.direction).ok_or_else(|| error("@opHole: zero axis direction"))?;
        let axis = Axis { origin: axis.origin, direction };
        let extremes: Vec<(usize, Extremes)> = targets
            .iter()
            .filter_map(|&t| extremes(&model.bodies[t].shape, axis, max_radius).map(|e| (t, e)))
            .collect();
        let Some(placed) = place_profiles(&profiles, &extremes)? else {
            results.push(map(vec![("success", Value::bool(false))]));
            continue;
        };
        let identity = identities.get(i).copied().flatten();
        let Some(tool) = build_tool(axis, &placed) else {
            results.push(map(vec![("success", Value::bool(false))]));
            continue;
        };
        let labels = |s: &Shape| label(axis, &placed, &face_names, &profile_names, s);
        let created = Info::new(id.clone());
        let special = |s: &Shape| {
            labels(s).map(|name| {
                Rc::new(Info {
                    created_by: id.clone(),
                    sketch_entity: None,
                    cap: None,
                    dependency: None,
                    label: Some(name),
                    identity,
                })
            })
        };
        let body = model.add_body(
            NewBody::new(tool, BodyType::Solid, created.clone()),
            &Origins::default(),
            &created,
            &special,
        );
        tools.push(body);

        let first = extremes.iter().map(|(_, e)| e.first_entrance).fold(f64::INFINITY, f64::min);
        let deepest = placed.iter().filter(|p| p.1 > 0.0).map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let mut depths = Vec::new();
        for (t, e) in &extremes {
            let entry = map(vec![
                ("firstEntrance", Value::number(e.first_entrance)),
                ("fullEntrance", Value::number(e.full_entrance)),
                ("firstExit", Value::number(e.first_exit)),
                ("fullExit", Value::number(e.full_exit)),
            ]);
            depths.push((tid_string(model.bodies[*t].tid), entry));
        }
        let mut references = Vec::new();
        for p in &profiles {
            if references.iter().any(|(name, _): &(Rc<str>, Value)| *name == p.reference) {
                continue;
            }
            if let Some(r) = reference(&p.reference, &extremes)? {
                let target =
                    r.target.map(|t| Value::string(tid_string(model.bodies[t].tid))).unwrap_or(Value::UNDEFINED);
                let info = map(vec![
                    ("referenceRootStart", Value::number(r.start)),
                    ("referenceRootEnd", Value::number(r.end)),
                    ("target", target),
                ]);
                references.push((p.reference.clone(), info));
            }
        }
        let keyed = |entries: Vec<(String, Value)>| map(entries.iter().map(|(k, v)| (k.as_str(), v.clone())).collect());
        results.push(map(vec![
            ("success", Value::bool(true)),
            ("holeDepth", Value::number(if first.is_finite() { deepest - first } else { deepest })),
            ("targetToDepthExtremes", keyed(depths)),
            ("positionReferenceInfo", keyed(references.into_iter().map(|(k, v)| (k.to_string(), v)).collect())),
        ]));
    }
    if tools.is_empty() {
        return Err(regen_error("HOLE_NO_HITS"));
    }
    if subtract {
        let cut: Vec<usize> = targets.iter().copied().filter(|t| !excluded.contains(t)).collect();
        if !cut.is_empty() {
            boolean_bodies(model, id, BooleanKind::Subtraction, true, cut, tools.clone(), keep_tools)?;
        } else if !keep_tools {
            model.remove_bodies(&tools);
        }
    }
    Ok(Value::array(results))
}

/// Where the infinite hole cylinder (sampled along its axis and lines around it) enters and leaves a target.
/// Crossings wholly behind the axis point are ignored.
fn extremes(target: &Shape, axis: Axis, radius: f64) -> Option<Extremes> {
    let u = perpendicular(axis.direction);
    let v = cross(axis.direction, u);
    let mut origins = vec![axis.origin];
    if radius > 0.0 {
        for k in 0..16 {
            let a = k as f64 * std::f64::consts::TAU / 16.0;
            origins.push(add(axis.origin, add(scale(u, radius * a.cos()), scale(v, radius * a.sin()))));
        }
    }
    let mut entrances = Vec::new();
    let mut exits = Vec::new();
    for o in origins {
        let Ok(crossings) = target.line_crossings(o, axis.direction) else { continue };
        if crossings.len() % 2 != 0 {
            continue;
        }
        let inside: Vec<(f64, f64)> =
            crossings.chunks(2).map(|c| (c[0], c[1])).filter(|(_, end)| *end > 1e-9).collect();
        if let (Some(first), Some(last)) = (inside.first(), inside.last()) {
            entrances.push(first.0);
            exits.push(last.1);
        }
    }
    if entrances.is_empty() {
        return None;
    }
    let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(Extremes {
        first_entrance: min(&entrances),
        full_entrance: max(&entrances),
        first_exit: min(&exits),
        full_exit: max(&exits),
    })
}

/// A `HolePositionReference`'s range along the axis, or `None` if no target defines it.
fn reference(name: &str, extremes: &[(usize, Extremes)]) -> R<Option<Reference>> {
    let pick = |key: fn(&Extremes) -> f64, furthest: bool| {
        extremes.iter().copied().reduce(|a, b| {
            let better = if furthest { key(&b.1) > key(&a.1) } else { key(&b.1) < key(&a.1) };
            if better { b } else { a }
        })
    };
    Ok(match name {
        "AXIS_POINT" => Some(Reference { start: 0.0, end: 0.0, target: None }),
        "TARGET_START" => pick(|e| e.first_entrance, false).map(|(t, e)| Reference {
            start: e.first_entrance,
            end: e.full_entrance,
            target: Some(t),
        }),
        "LAST_TARGET_START" => pick(|e| e.first_entrance, true).map(|(t, e)| Reference {
            start: e.first_entrance,
            end: e.full_entrance,
            target: Some(t),
        }),
        "LAST_TARGET_END" => pick(|e| e.full_exit, true).map(|(t, e)| Reference {
            start: e.first_exit,
            end: e.full_exit,
            target: Some(t),
        }),
        other => return Err(error(format!("@opHole: HolePositionReference.{other} isn't supported yet"))),
    })
}

/// Each profile's distance along the axis and radius, with each profile identical to the one before it dropped, and
/// the index of the profile it came from. `None` if a reference the profiles need meets no target.
fn place_profiles(profiles: &[Profile], extremes: &[(usize, Extremes)]) -> R<Option<Vec<(f64, f64, usize)>>> {
    let mut placed: Vec<(f64, f64, usize)> = Vec::new();
    for (k, p) in profiles.iter().enumerate() {
        let Some(r) = reference(&p.reference, extremes)? else { return Ok(None) };
        let at = if p.before { r.start } else { r.end } + p.position;
        if let Some(last) = placed.last()
            && (last.0 - at).abs() < 1e-9
            && (last.1 - p.radius).abs() < 1e-9
        {
            continue;
        }
        placed.push((at, p.radius, k));
    }
    Ok(Some(placed))
}

fn perpendicular(d: Vec3) -> Vec3 {
    let other = if d.x.abs() < 0.9 { Vec3 { x: 1.0, y: 0.0, z: 0.0 } } else { Vec3 { x: 0.0, y: 1.0, z: 0.0 } };
    normalize(cross(d, other)).unwrap_or(other)
}

/// The profile outline in a half plane through the axis: from the axis out to the first profile, through each
/// profile, back to the axis at the last (radius 0), and up the axis. Revolved a full turn.
fn build_tool(axis: Axis, placed: &[(f64, f64, usize)]) -> Option<Shape> {
    let u = perpendicular(axis.direction);
    let at = |t: f64, r: f64| add(axis.origin, add(scale(axis.direction, t), scale(u, r)));
    let mut points = vec![at(placed[0].0, 0.0)];
    points.extend(placed.iter().map(|&(t, r, _)| at(t, r)));
    points.push(points[0]);
    let mut edges = Vec::new();
    for pair in points.windows(2) {
        if norm(sub(pair[1], pair[0])) > 1e-12 {
            edges.push(occt::line_edge(pair[0], pair[1]).ok()?);
        }
    }
    let frame = Frame { origin: axis.origin, normal: cross(axis.direction, u), x: axis.direction };
    let face = occt::sketch_regions(&edges, frame).ok()?.into_iter().next()?;
    let op = occt::revolve(&face, axis, std::f64::consts::TAU).ok()?;
    op.shape().sub_shapes(ShapeKind::Solid).into_iter().next()
}

/// The name of a tool face (from `faceNames`: the face before each profile) or profile edge (the profile's name).
fn label(
    axis: Axis,
    placed: &[(f64, f64, usize)],
    face_names: &[Option<Rc<str>>],
    profile_names: &[Option<Rc<str>>],
    s: &Shape,
) -> Option<Rc<str>> {
    let local = |p: Vec3| {
        let rel = sub(p, axis.origin);
        let t = dot(rel, axis.direction);
        (t, norm(sub(rel, scale(axis.direction, t))))
    };
    match s.kind() {
        Some(ShapeKind::Face) => {
            let (t, r) = local(s.face_tangent_plane(0.5, 0.5).ok()?.origin);
            // The segments of the outline: the cap (to the first profile), then one to each later profile.
            let mut best: Option<(f64, usize)> = None;
            let mut previous = (placed[0].0, 0.0);
            for &(pt, pr, k) in placed {
                let d = segment_distance((t, r), previous, (pt, pr));
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, k));
                }
                previous = (pt, pr);
            }
            let (d, k) = best?;
            if d > 1e-6 {
                return None;
            }
            face_names.get(k).cloned().flatten()
        }
        // A profile's circle.
        Some(ShapeKind::Edge) => {
            let (t, r) = local(s.edge_tangent(0.5, false).ok()?.origin);
            let &(_, _, k) = placed.iter().find(|&&(pt, pr, _)| (pt - t).abs() < 1e-7 && (pr - r).abs() < 1e-7)?;
            profile_names.get(k).cloned().flatten()
        }
        _ => None,
    }
}

fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length2 = dx * dx + dy * dy;
    let s = if length2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (a.0 + s * dx, a.1 + s * dy);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}
