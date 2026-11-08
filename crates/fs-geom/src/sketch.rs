//! Sketches: `@newSketch` makes a [`Sketch`] object, `@sk*` builtins add entities to it, and `@skSolve` adds the
//! result to the context.
//!
//! Constraints are recorded but not solved: the engine only places entities where their definitions put them,
//! which is exact for sketches built in FeatureScript with explicit coordinates.
//!
//! Solving adds, as Onshape does (fixture batch 004, c012), a wire body for each curve (construction curves too,
//! though none counts as construction for `qConstructionFilter`), a sheet body for each enclosed region, and a
//! point body for each `skPoint` and each circle, arc or ellipse centre.

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
    Circle {
        center: P2,
        radius: f64,
    },
    Arc {
        start: P2,
        mid: P2,
        end: P2,
    },
    /// An ellipse, whole or (with `arc`) from one parameter to another; parameters have a period of 1, with 0 on
    /// the major axis and 0.25 on the minor.
    Ellipse {
        center: P2,
        major_axis: P2,
        major: f64,
        minor: f64,
        arc: Option<(f64, f64)>,
    },
    /// A cubic spline through points, periodic if the last point is the first; with optional end derivatives and
    /// point parameters.
    FitSpline {
        points: Vec<P2>,
        derivatives: Option<(P2, P2)>,
        parameters: Vec<f64>,
    },
    Bezier(Vec<P2>),
    Conic {
        start: P2,
        control: P2,
        end: P2,
        rho: f64,
    },
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

fn points(value: &Value, key: &str, what: &str) -> R<Vec<P2>> {
    let items = field(value, key).as_array().ok_or_else(|| error(format!("{what} must be an array of points")))?;
    items.iter().map(|p| values::vec2(p, what)).collect()
}

fn number_or(value: &Value, key: &str, default: f64) -> R<f64> {
    match field(value, key) {
        v if v.is_undefined() => Ok(default),
        v => v.as_number().ok_or_else(|| error(format!("{key} must be a number"))),
    }
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
    m.insert("skEllipse", |_, args| {
        let id = add(&args, "skEllipse", |v| ellipse(v, None))?;
        Ok(ids(&id, &[("centerId", "center")]))
    });
    m.insert("skEllipticalArc", |_, args| {
        let id = add(&args, "skEllipticalArc", |v| {
            let arc = (number_or(v, "startParameter", 0.0)?, number_or(v, "endParameter", 1.0)?);
            ellipse(v, Some(arc))
        })?;
        Ok(ids(&id, &[("startId", "start"), ("endId", "end"), ("centerId", "center")]))
    });
    m.insert("skFitSpline", |_, args| {
        let id = add(&args, "skFitSpline", |v| {
            let points = points(v, "points", "skFitSpline points")?;
            let derivative = |key: &str| -> R<Option<P2>> {
                match field(v, key) {
                    d if d.is_undefined() => Ok(None),
                    d => values::vec2(d, key).map(Some),
                }
            };
            let derivatives = match (derivative("startDerivative")?, derivative("endDerivative")?) {
                (Some(a), Some(b)) => Some((a, b)),
                (None, None) => None,
                _ => return Err(error("@skFitSpline: give both end derivatives or neither")),
            };
            let parameters = match field(v, "parameters") {
                p if p.is_undefined() => Vec::new(),
                p => values::numbers(p, "skFitSpline parameters")?,
            };
            Ok(Geometry::FitSpline { points, derivatives, parameters })
        })?;
        Ok(ids(&id, &[("startId", "start"), ("endId", "end")]))
    });
    m.insert("skBezier", |_, args| {
        let id = add(&args, "skBezier", |v| Ok(Geometry::Bezier(points(v, "points", "skBezier points")?)))?;
        Ok(ids(&id, &[("startId", "start"), ("endId", "end")]))
    });
    m.insert("skConicSegment", |_, args| {
        let id = add(&args, "skConicSegment", |v| {
            if !field(v, "lowParameter").is_undefined() || !field(v, "highParameter").is_undefined() {
                return Err(error("@skConicSegment: trimmed conics aren't supported yet"));
            }
            Ok(Geometry::Conic {
                start: point(v, "start", "skConicSegment start")?,
                control: point(v, "controlPoint", "skConicSegment controlPoint")?,
                end: point(v, "end", "skConicSegment end")?,
                rho: number_or(v, "rho", 0.5)?,
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

/// An ellipse's definition. `majorAxis` is a unit direction, or a point on the major axis.
fn ellipse(v: &Value, arc: Option<(f64, f64)>) -> R<Geometry> {
    let center = match field(v, "center") {
        c if c.is_undefined() => (0.0, 0.0),
        c => values::vec2(c, "ellipse center")?,
    };
    let axis = field(v, "majorAxis");
    let major_axis = if axis.is_undefined() {
        (1.0, 0.0)
    } else if axis.as_array().is_some_and(|a| a.iter().all(|x| x.as_number().is_some())) {
        values::vec2(axis, "ellipse majorAxis")?
    } else {
        let p = values::vec2(axis, "ellipse majorAxis")?;
        (p.0 - center.0, p.1 - center.1)
    };
    let length = major_axis.0.hypot(major_axis.1);
    if length == 0.0 {
        return Err(error("ellipse majorAxis is zero"));
    }
    Ok(Geometry::Ellipse {
        center,
        major_axis: (major_axis.0 / length, major_axis.1 / length),
        major: values::length(field(v, "majorRadius"), "ellipse majorRadius")?,
        minor: values::length(field(v, "minorRadius"), "ellipse minorRadius")?,
        arc,
    })
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
    let direction =
        |d: P2| values::add(values::scale(plane.x, d.0), values::scale(values::cross(plane.normal, plane.x), d.1));
    let mut edges: Vec<(Rc<str>, Shape, bool)> = Vec::new();
    let mut points: Vec<(Rc<str>, occt::Vec3, bool)> = Vec::new();
    for entity in sketch.entities.borrow().iter() {
        let edge = match &entity.geometry {
            Geometry::Point(p) => {
                points.push((entity.id.clone(), world(*p), entity.construction));
                continue;
            }
            // A zero-length line, such as the tip of std's fCone with no top radius, adds nothing.
            Geometry::Line(a, b) if (a.0 - b.0).hypot(a.1 - b.1) < 1e-12 => continue,
            Geometry::Line(a, b) => occt::line_edge(world(*a), world(*b)).map_err(occt_error)?,
            Geometry::Circle { center, radius } => {
                points.push((format!("{}.center", entity.id).into(), world(*center), entity.construction));
                occt::circle_edge(circle_frame(&plane, *center), *radius).map_err(occt_error)?
            }
            Geometry::Arc { start, mid, end } => {
                if let Some((center, _)) = circle_through(*start, *mid, *end) {
                    points.push((format!("{}.center", entity.id).into(), world(center), entity.construction));
                }
                occt::arc_through(world(*start), world(*mid), world(*end)).map_err(occt_error)?
            }
            Geometry::Ellipse { center, major_axis, major, minor, arc } => {
                points.push((format!("{}.center", entity.id).into(), world(*center), entity.construction));
                let frame = Frame { origin: world(*center), normal: plane.normal, x: direction(*major_axis) };
                let arc = arc.map(|(a, b)| (a * std::f64::consts::TAU, b * std::f64::consts::TAU));
                occt::ellipse_edge(frame, *major, *minor, arc).map_err(occt_error)?
            }
            Geometry::FitSpline { points: through, derivatives, parameters } => {
                let mut through: Vec<occt::Vec3> = through.iter().map(|p| world(*p)).collect();
                let closed = through.len() > 2
                    && values::norm(values::sub(through[0], *through.last().expect("points"))) < 1e-12;
                if closed {
                    through.pop();
                }
                let ends = match derivatives {
                    _ if closed => crate::splines::Ends::Periodic,
                    Some((a, b)) => crate::splines::Ends::Clamped(direction(*a), direction(*b)),
                    None => crate::splines::Ends::Natural,
                };
                let parameters = (!parameters.is_empty()).then(|| parameters.clone());
                crate::splines::spline(&through, ends, parameters)?
            }
            Geometry::Bezier(poles) => {
                let poles: Vec<occt::Vec3> = poles.iter().map(|p| world(*p)).collect();
                occt::bezier_edge(&poles).map_err(occt_error)?
            }
            Geometry::Conic { start, control, end, rho } => {
                let w = rho / (1.0 - rho);
                occt::conic_edge(world(*start), world(*control), world(*end), w).map_err(occt_error)?
            }
        };
        edges.push((entity.id.clone(), edge, entity.construction));
    }

    let created = Info::new(sketch.id.clone());
    let none = Origins::default();
    // Regions, from the edges that aren't construction: a sheet body each.
    let shapes: Vec<Shape> = edges.iter().filter(|(_, _, c)| !c).map(|(_, e, _)| e.clone()).collect();
    for region in occt::sketch_regions(&shapes, plane).map_err(occt_error)? {
        let new = NewBody { sketch_plane: Some(plane), ..NewBody::new(region, BodyType::Sheet, created.clone()) };
        model.add_body(new, &none, &created, &|_| None);
    }
    // Curves: a wire body each.
    for (id, edge, _) in edges {
        let info = Rc::new(Info { created_by: sketch.id.clone(), sketch_entity: Some(id), cap: None });
        let new = NewBody { sketch_plane: Some(plane), ..NewBody::new(edge, BodyType::Wire, info.clone()) };
        model.add_body(new, &none, &info, &|_| None);
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
