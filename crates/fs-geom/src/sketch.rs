//! Sketches: `@newSketch` makes a [`Sketch`] object, `@sk*` builtins add entities to it, and `@skSolve` adds the
//! result to the context.
//!
//! Constraints are recorded but not solved: the engine only places entities where their definitions put them,
//! which is exact for sketches built in FeatureScript with explicit coordinates.
//!
//! As std's sketch documentation says, solving adds the sketch's edges as wire bodies, its enclosed regions as
//! sheet bodies, and its free points (`skPoint`s and circle and arc centres) as point bodies.

use crate::model::{BodyType, FeatureId, Info, Model, NewBody, Origins};
use crate::values::{self, field, on_plane};
use crate::{GeomBuiltins, model_of, tags};
use fs_interp::Value;
use fs_interp::builtins::{arg, string};
use fs_interp::eval::{R, error};
use fs_interp::value::{Kind, Native, next_serial};
use occt::{Frame, Shape};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Sketch {
    serial: u64,
    context: Value,
    id: FeatureId,
    plane: Frame,
    entities: RefCell<Vec<SketchEntity>>,
    solved: std::cell::Cell<bool>,
}

impl Native for Sketch {
    fn type_name(&self) -> &'static str {
        "Sketch"
    }
    fn serial(&self) -> u64 {
        self.serial
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

type P2 = (f64, f64);

enum Geometry {
    Point(P2),
    Line(P2, P2),
    Circle { center: P2, radius: f64 },
    Arc { start: P2, mid: P2, end: P2 },
}

struct SketchEntity {
    id: Rc<str>,
    geometry: Geometry,
    construction: bool,
}

fn sketch_arg<'a>(args: &'a [Value], name: &str) -> R<&'a Sketch> {
    match &arg(args, 0, name)?.kind {
        Kind::Builtin(native) => native.as_any().downcast_ref::<Sketch>(),
        _ => None,
    }
    .ok_or_else(|| error(format!("@{name}: argument 1 must be a Sketch")))
}

fn point(value: &Value, key: &str, what: &str) -> R<P2> {
    values::vec2(field(value, key), what)
}

fn add(args: &[Value], name: &str, make: impl FnOnce(&Value) -> R<Geometry>) -> R<Rc<str>> {
    let sketch = sketch_arg(args, name)?;
    let id: Rc<str> = string(args, 1, name)?.into();
    let value = arg(args, 2, name)?;
    if sketch.solved.get() {
        return Err(error(format!("@{name}: the sketch has already been solved")));
    }
    let construction = field(value, "construction").as_bool().unwrap_or(false);
    let geometry = make(value)?;
    let mut entities = sketch.entities.borrow_mut();
    if entities.iter().any(|e| e.id == id) {
        return Err(error(format!("@{name}: sketch entity id {id:?} is already used")));
    }
    entities.push(SketchEntity { id: id.clone(), geometry, construction });
    Ok(id)
}

fn ids(base: &str, suffixes: &[(&str, &str)]) -> Value {
    values::map(suffixes.iter().map(|(key, suffix)| (*key, Value::string(format!("{base}.{suffix}")))).collect())
}

pub fn register(m: &mut GeomBuiltins) {
    m.insert("newSketch", |_, args| {
        arg(&args, 0, "newSketch")?;
        let context = args[0].clone();
        let id = values::feature_id(arg(&args, 1, "newSketch")?, "@newSketch id")?;
        let value = arg(&args, 2, "newSketch")?;
        let plane = values::plane(field(value, "sketchPlane"), "sketchPlane")?;
        Ok(Value::builtin(Rc::new(Sketch {
            serial: next_serial(),
            context,
            id,
            plane,
            entities: RefCell::new(Vec::new()),
            solved: std::cell::Cell::new(false),
        })))
    });
    m.insert("isSketch", |_, args| {
        let v = arg(&args, 0, "isSketch")?;
        Ok(Value::bool(matches!(&v.kind, Kind::Builtin(n) if n.as_any().is::<Sketch>())))
    });
    m.insert("skPoint", |_, args| {
        let id = add(&args, "skPoint", |v| Ok(Geometry::Point(point(v, "position", "skPoint position")?)))?;
        Ok(values::map(vec![("pointId", Value::string(id))]))
    });
    m.insert("skLineSegment", |_, args| {
        let id = add(&args, "skLineSegment", |v| {
            Ok(Geometry::Line(point(v, "start", "skLineSegment start")?, point(v, "end", "skLineSegment end")?))
        })?;
        Ok(ids(&id, &[("startId", "start"), ("endId", "end")]))
    });
    m.insert("skCircle", |_, args| {
        let id = add(&args, "skCircle", |v| {
            let center = point(v, "center", "skCircle center")?;
            let radius = values::length(field(v, "radius"), "skCircle radius")?;
            Ok(Geometry::Circle { center, radius })
        })?;
        Ok(ids(&id, &[("centerId", "center")]))
    });
    m.insert("skArc", |_, args| {
        let id = add(&args, "skArc", |v| {
            Ok(Geometry::Arc {
                start: point(v, "start", "skArc start")?,
                mid: point(v, "mid", "skArc mid")?,
                end: point(v, "end", "skArc end")?,
            })
        })?;
        Ok(ids(&id, &[("startId", "start"), ("endId", "end")]))
    });
    // Constraints are already satisfied by entities placed with explicit coordinates.
    m.insert("skConstraint", |_, args| {
        sketch_arg(&args, "skConstraint")?;
        Ok(Value::UNDEFINED)
    });
    m.insert("skSolve", |interp, args| {
        let sketch = sketch_arg(&args, "skSolve")?;
        if sketch.solved.replace(true) {
            return Err(error("@skSolve: the sketch has already been solved"));
        }
        tags(interp)?;
        model_of(&sketch.context, "skSolve", |model| solve(model, sketch))?;
        Ok(Value::UNDEFINED)
    });
}

fn circle_frame(plane: &Frame, center: P2) -> Frame {
    Frame { origin: on_plane(plane, center.0, center.1), normal: plane.normal, x: plane.x }
}

/// The circle through three points, as (centre, radius), or `None` if they're collinear.
fn circle_through(a: P2, b: P2, c: P2) -> Option<(P2, f64)> {
    let d = 2.0 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if d.abs() < 1e-18 {
        return None;
    }
    let (a2, b2, c2) = (a.0 * a.0 + a.1 * a.1, b.0 * b.0 + b.1 * b.1, c.0 * c.0 + c.1 * c.1);
    let x = (a2 * (b.1 - c.1) + b2 * (c.1 - a.1) + c2 * (a.1 - b.1)) / d;
    let y = (a2 * (c.0 - b.0) + b2 * (a.0 - c.0) + c2 * (b.0 - a.0)) / d;
    Some(((x, y), ((a.0 - x).powi(2) + (a.1 - y).powi(2)).sqrt()))
}

fn occt_error(e: occt::Error) -> fs_interp::eval::Flow {
    error(format!("sketch: {e}"))
}

fn solve(model: &mut Model, sketch: &Sketch) -> R<()> {
    let plane = sketch.plane;
    let world = |p: P2| on_plane(&plane, p.0, p.1);
    let mut edges: Vec<(Rc<str>, Shape)> = Vec::new();
    let mut construction_edges: Vec<(Rc<str>, Shape)> = Vec::new();
    let mut points: Vec<(Rc<str>, occt::Vec3, bool)> = Vec::new();
    for entity in sketch.entities.borrow().iter() {
        let edge = match entity.geometry {
            Geometry::Point(p) => {
                points.push((entity.id.clone(), world(p), entity.construction));
                continue;
            }
            Geometry::Line(a, b) => occt::line_edge(world(a), world(b)).map_err(occt_error)?,
            Geometry::Circle { center, radius } => {
                points.push((format!("{}.center", entity.id).into(), world(center), entity.construction));
                occt::circle_edge(circle_frame(&plane, center), radius).map_err(occt_error)?
            }
            Geometry::Arc { start, mid, end } => {
                if let Some((center, _)) = circle_through(start, mid, end) {
                    points.push((format!("{}.center", entity.id).into(), world(center), entity.construction));
                }
                occt::arc_through(world(start), world(mid), world(end)).map_err(occt_error)?
            }
        };
        let list = if entity.construction { &mut construction_edges } else { &mut edges };
        list.push((entity.id.clone(), edge));
    }

    let created = Info::new(sketch.id.clone());
    let none = Origins::default();
    let shapes: Vec<Shape> = edges.iter().map(|(_, e)| e.clone()).collect();
    let regions = occt::sketch_regions(&shapes, plane).map_err(occt_error)?;
    if !regions.is_empty() {
        let shape = if regions.len() == 1 { regions[0].clone() } else { occt::compound(&regions) };
        let new = NewBody { sketch_plane: Some(plane), ..NewBody::new(shape, BodyType::Sheet, created.clone()) };
        model.add_body(new, &none, &created, &|_| None);
    }
    for (list, construction) in [(&edges, false), (&construction_edges, true)] {
        if list.is_empty() {
            continue;
        }
        let shapes: Vec<Shape> = list.iter().map(|(_, e)| e.clone()).collect();
        let shape = if shapes.len() == 1 { shapes[0].clone() } else { occt::compound(&shapes) };
        let sketch_entity = |s: &Shape| {
            let (id, _) = list.iter().find(|(_, e)| e.is_same(s))?;
            Some(Rc::new(Info { created_by: sketch.id.clone(), sketch_entity: Some(id.clone()), cap: None }))
        };
        let new =
            NewBody { construction, sketch_plane: Some(plane), ..NewBody::new(shape, BodyType::Wire, created.clone()) };
        model.add_body(new, &none, &created, &sketch_entity);
    }
    for (id, p, construction) in points {
        let info = Rc::new(Info { created_by: sketch.id.clone(), sketch_entity: Some(id), cap: None });
        let shape = occt::vertex(p).map_err(occt_error)?;
        let new =
            NewBody { construction, sketch_plane: Some(plane), ..NewBody::new(shape, BodyType::Point, info.clone()) };
        model.add_body(new, &none, &info, &|_| None);
    }
    Ok(())
}
