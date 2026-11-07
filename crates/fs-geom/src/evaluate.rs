//! Measurements (`@ev*`): the builtins that read a context's geometry. They return std's own types, with units.

use crate::model::{EntityRef, EntityType, Model};
use crate::values::{Tags, field};
use crate::{GeomBuiltins, model_mut, occt_error, query, regen_error, tags};
use fs_interp::Value;
use fs_interp::builtins::arg;
use fs_interp::eval::R;
use occt::{Box3, Vec3};

fn definition(args: &[Value], name: &str) -> R<Value> {
    Ok(arg(args, 1, name)?.clone())
}

/// Runs a measurement over what `key` in the definition finds.
fn measure(
    args: Vec<Value>,
    name: &str,
    key: &str,
    f: impl FnOnce(&Tags, &Model, Vec<EntityRef>) -> R<Value>,
    tags: &Tags,
) -> R<Value> {
    let definition = definition(&args, name)?;
    model_mut(&args, name, |model| {
        let found = query::evaluate(model, field(&definition, key))?;
        f(tags, model, found)
    })
}

fn sum(model: &Model, found: &[EntityRef], f: impl Fn(&occt::Shape) -> occt::Result<f64>) -> R<f64> {
    found.iter().map(|r| f(model.shape(*r)).map_err(occt_error)).sum()
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("evPlane", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evPlane",
            "face",
            |tags, model, found| {
                let face = found.first().filter(|r| r.kind == EntityType::Face);
                let frame = face.and_then(|r| model.shape(*r).face_plane().ok());
                frame.map(|f| tags.plane(f)).ok_or_else(|| regen_error("CANNOT_RESOLVE_PLANE"))
            },
            &tags,
        )
    });
    m.insert("evOwnerSketchPlane", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evOwnerSketchPlane",
            "entity",
            |tags, model, found| {
                let plane = found.first().and_then(|r| model.body(*r).sketch_plane);
                plane.map(|f| tags.plane(f)).ok_or_else(|| regen_error("CANNOT_RESOLVE_SKETCH_PLANE"))
            },
            &tags,
        )
    });
    m.insert("evVertexPoint", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evVertexPoint",
            "vertex",
            |tags, model, found| {
                let vertex =
                    found.first().filter(|r| r.kind == EntityType::Vertex || model.body(**r).vertices.len() == 1);
                let point = vertex.and_then(|r| {
                    let shape = match r.kind {
                        EntityType::Vertex => model.shape(*r).clone(),
                        _ => model.body(*r).vertices[0].shape.clone(),
                    };
                    shape.vertex_point().ok()
                });
                point.map(|p| tags.length_vector(p)).ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))
            },
            &tags,
        )
    });
    m.insert("evArea", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evArea",
            "faces",
            |tags, model, found| Ok(tags.with_units(sum(model, &found, |s| s.area())?, "meter", 2.0)),
            &tags,
        )
    });
    m.insert("evLength", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evLength",
            "edges",
            |tags, model, found| Ok(tags.length(sum(model, &found, |s| s.length())?)),
            &tags,
        )
    });
    m.insert("evVolume", |interp, args| {
        let tags = tags(interp)?;
        measure(
            args,
            "evVolume",
            "bodies",
            |tags, model, found| Ok(tags.with_units(sum(model, &found, |s| s.volume())?, "meter", 3.0)),
            &tags,
        )
    });
    m.insert("evBox", |interp, args| {
        let tags = tags(interp)?;
        let tight = definition(&args, "evBox")?.get("tight").and_then(Value::as_bool).unwrap_or(true);
        measure(
            args,
            "evBox",
            "topology",
            |tags, model, found| {
                let mut total: Option<Box3> = None;
                for r in &found {
                    let b = model.shape(*r).bounding_box(tight).map_err(occt_error)?;
                    if b.empty {
                        continue;
                    }
                    total = Some(match total {
                        None => b,
                        Some(t) => Box3 {
                            min: Vec3::new(t.min.x.min(b.min.x), t.min.y.min(b.min.y), t.min.z.min(b.min.z)),
                            max: Vec3::new(t.max.x.max(b.max.x), t.max.y.max(b.max.y), t.max.z.max(b.max.z)),
                            empty: false,
                        },
                    });
                }
                let total = total.ok_or_else(|| regen_error("CANNOT_RESOLVE_ENTITIES"))?;
                Ok(crate::values::map(vec![
                    ("minCorner", tags.length_vector(total.min)),
                    ("maxCorner", tags.length_vector(total.max)),
                ]))
            },
            &tags,
        )
    });
}
