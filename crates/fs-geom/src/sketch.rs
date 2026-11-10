//! Sketches: `@newSketch` makes a [`Sketch`] object, `@sk*` builtins add entities to it, and `@skSolve` adds the
//! result to the context.
//!
//! Constraints are solved only where the entities don't already meet them (sketches built in FeatureScript with
//! explicit coordinates do): from where the definitions put the entities, by damped Gauss–Newton over the points,
//! lines and arcs (a tear drop hem's profile, batch 029). Constraint types not listed in `residuals` are ignored.
//!
//! Solving adds, as Onshape does (fixture batch 004, c012), a wire body for each curve (construction curves too,
//! though none counts as construction for `qConstructionFilter`; their edges do), a sheet body for each enclosed region, and a
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
    constraints: RefCell<Vec<Value>>,
    solved: std::cell::Cell<bool>,
    /// Drawn in this sheet metal part's flat pattern (its plane is a flat face, `qCorrespondingInFlat`).
    flat_of: Option<crate::model::Tid>,
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
        derivatives: (Option<P2>, Option<P2>),
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
        let reference = field(value, "planeReference");
        let flat_of = if reference.is_undefined() {
            None
        } else {
            crate::with_model(&context, |model| crate::query::flat_part(model, reference)).transpose()?.flatten()
        };
        Ok(Value::builtin(Rc::new(Sketch {
            serial: next_serial(),
            context,
            id,
            plane,
            entities: RefCell::new(Vec::new()),
            constraints: RefCell::new(Vec::new()),
            solved: std::cell::Cell::new(false),
            flat_of,
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
    // An image placed in the sketch: Onshape bounds it with four construction lines, as wide as the corners are
    // apart and as tall as the image's shape makes that, from the lower left corner (batch 094).
    m.insert("skImage", |_, args| {
        let sketch = sketch_arg(&args, "skImage")?;
        let id: Rc<str> = string(&args, 1, "skImage")?.into();
        let value = arg(&args, 2, "skImage")?;
        if sketch.solved.get() {
            return Err(error("@skImage: the sketch has already been solved"));
        }
        let first = point(value, "firstCorner", "skImage firstCorner")?;
        let second = point(value, "secondCorner", "skImage secondCorner")?;
        let pixels = |key: &str| {
            field(value, key)
                .as_number()
                .filter(|n| *n > 0.0)
                .ok_or_else(|| error(format!("@skImage: blobInfo has no {key}: it must be an image's BLOB_DATA")))
        };
        let (width, height) = (pixels("imageWidth")?, pixels("imageHeight")?);
        let (x0, y0) = (first.0.min(second.0), first.1.min(second.1));
        let x1 = x0 + (second.0 - first.0).abs();
        let y1 = y0 + (x1 - x0) * height / width;
        let mut entities = sketch.entities.borrow_mut();
        let sides = [
            ("bottom", (x0, y0), (x1, y0)),
            ("right", (x1, y0), (x1, y1)),
            ("top", (x1, y1), (x0, y1)),
            ("left", (x0, y1), (x0, y0)),
        ];
        for (side, a, b) in sides {
            let side_id: Rc<str> = format!("{id}.{side}").into();
            if entities.iter().any(|e| e.id == side_id) {
                return Err(error(format!("@skImage: sketch entity id {side_id:?} is already used")));
            }
            entities.push(SketchEntity { id: side_id, geometry: Geometry::Line(a, b), construction: true });
        }
        Ok(values::map(vec![("imageId", Value::string(id))]))
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
            // Either end's derivative, or neither (Onshape takes one alone, batch 066 c005).
            let derivatives = (derivative("startDerivative")?, derivative("endDerivative")?);
            let parameters = match field(v, "parameters") {
                p if p.is_undefined() => Vec::new(),
                p => values::numbers(p, "skFitSpline parameters")?,
            };
            Ok(Geometry::FitSpline { points, derivatives, parameters })
        })?;
        Ok(ids(&id, &[("startId", "start"), ("endId", "end")]))
    });
    // A Bézier comes with its control polygon, construction lines between its points, each a wire body as the
    // curve is (batch 071 c008: a curve made of the sketch's edges is closed by them).
    m.insert("skBezier", |_, args| {
        let id = add(&args, "skBezier", |v| Ok(Geometry::Bezier(points(v, "points", "skBezier points")?)))?;
        let sketch = sketch_arg(&args, "skBezier")?;
        let poles = match sketch.entities.borrow().last().map(|e| &e.geometry) {
            Some(Geometry::Bezier(poles)) => poles.clone(),
            _ => Vec::new(),
        };
        for (k, w) in poles.windows(2).enumerate() {
            let line_id: Rc<str> = format!("{id}.polygon{k}").into();
            sketch.entities.borrow_mut().push(SketchEntity { id: line_id, geometry: Geometry::Line(w[0], w[1]), construction: true });
        }
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
    m.insert("skConstraint", |_, args| {
        let sketch = sketch_arg(&args, "skConstraint")?;
        sketch.constraints.borrow_mut().push(arg(&args, 2, "skConstraint")?.clone());
        Ok(Value::UNDEFINED)
    });
    m.insert("skSolve", |interp, args| {
        let sketch = sketch_arg(&args, "skSolve")?;
        if sketch.solved.replace(true) {
            return Err(error("@skSolve: the sketch has already been solved"));
        }
        tags(interp)?;
        constraints::solve(&mut sketch.entities.borrow_mut(), &sketch.constraints.borrow())?;
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
    // Each open curve's ends, start then end (an arc's counterclockwise about the normal, as Onshape has them).
    let mut ends: Vec<(Rc<str>, occt::Vec3, occt::Vec3)> = Vec::new();
    let mut points: Vec<(Rc<str>, occt::Vec3, bool)> = Vec::new();
    for entity in sketch.entities.borrow().iter() {
        let edge = match &entity.geometry {
            Geometry::Point(p) => {
                points.push((entity.id.clone(), world(*p), entity.construction));
                continue;
            }
            // A zero-length line, such as the tip of std's fCone with no top radius, adds nothing.
            Geometry::Line(a, b) if (a.0 - b.0).hypot(a.1 - b.1) < 1e-12 => continue,
            Geometry::Line(a, b) => {
                ends.push((entity.id.clone(), world(*a), world(*b)));
                occt::line_edge(world(*a), world(*b)).map_err(occt_error)?
            }
            Geometry::Circle { center, radius } => {
                points.push((format!("{}.center", entity.id).into(), world(*center), entity.construction));
                occt::circle_edge(circle_frame(&plane, *center), *radius).map_err(occt_error)?
            }
            Geometry::Arc { start, mid, end } => {
                if let Some((center, _)) = circle_through(*start, *mid, *end) {
                    points.push((format!("{}.center", entity.id).into(), world(center), entity.construction));
                }
                // Onshape's arcs run counterclockwise about the sketch normal, whichever end is the start
                // (batch 010, c006).
                let clockwise = (mid.0 - start.0) * (end.1 - mid.1) - (mid.1 - start.1) * (end.0 - mid.0) < 0.0;
                let (a, b) = if clockwise { (*end, *start) } else { (*start, *end) };
                ends.push((entity.id.clone(), world(a), world(b)));
                occt::arc_through(world(a), world(*mid), world(b)).map_err(occt_error)?
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
                    (Some(a), Some(b)) => crate::splines::Ends::Clamped(direction(*a), direction(*b)),
                    (Some(a), None) => crate::splines::Ends::StartClamped(direction(*a)),
                    (None, Some(b)) => crate::splines::Ends::EndClamped(direction(*b)),
                    (None, None) => crate::splines::Ends::Natural,
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
    // The curves' wire bodies come first, then the regions' sheets, as Onshape lists them (batch 091).
    let shapes: Vec<Shape> = edges.iter().filter(|(_, _, c)| !c).map(|(_, e, _)| e.clone()).collect();
    // Curves: a wire body each; a construction curve's edge and vertices are construction.
    for (id, edge, construction) in edges {
        let info = Rc::new(Info {
            created_by: sketch.id.clone(),
            sketch_entity: Some(id.clone()),
            cap: None,
            dependency: None,
            construction,
            ..Default::default()
        });
        let new = NewBody {
            sketch_plane: Some(plane),
            flat_of: sketch.flat_of,
            ..NewBody::new(edge, BodyType::Wire, info.clone())
        };
        // Its end vertices are "<id>.start" and "<id>.end".
        let named_ends: Vec<(occt::Vec3, Rc<Info>)> = ends
            .iter()
            .filter(|(curve, _, _)| *curve == id)
            .flat_map(|(curve, a, b)| {
                let name = |suffix: &str| {
                    Rc::new(Info {
                        created_by: sketch.id.clone(),
                        sketch_entity: Some(format!("{curve}.{suffix}").into()),
                        construction,
                        ..Default::default()
                    })
                };
                [(*a, name("start")), (*b, name("end"))]
            })
            .collect();
        let special = |shape: &Shape| -> Option<Rc<Info>> {
            let p = shape.vertex_point().ok()?;
            named_ends.iter().find(|(q, _)| values::norm(values::sub(p, *q)) < 1e-12).map(|(_, info)| info.clone())
        };
        model.add_body(new, &none, &info, &special);
    }
    // Regions, from the edges that aren't construction: a sheet body for each set of regions joined by shared edges
    // (a ring and the square inside it are one sheet; frame relies on it).
    let regions = occt::sketch_regions(&shapes, plane).map_err(occt_error)?;
    let sheets = if regions.len() > 1 {
        occt::connected_shells(&occt::compound(&regions)).map_err(occt_error)?
    } else {
        regions
    };
    for sheet in sheets {
        let new = NewBody {
            sketch_plane: Some(plane),
            flat_of: sketch.flat_of,
            ..NewBody::new(sheet, BodyType::Sheet, created.clone())
        };
        model.add_body(new, &none, &created, &|_| None);
    }
    for (id, p, construction) in points {
        let info = Rc::new(Info {
            created_by: sketch.id.clone(),
            sketch_entity: Some(id),
            cap: None,
            dependency: None,
            ..Default::default()
        });
        let shape = occt::vertex(p).map_err(occt_error)?;
        let new = NewBody {
            construction,
            sketch_plane: Some(plane),
            flat_of: sketch.flat_of,
            ..NewBody::new(shape, BodyType::Point, info.clone())
        };
        model.add_body(new, &none, &info, &|_| None);
    }
    Ok(())
}

mod constraints {
    //! Solving a sketch's constraints: see the module's documentation.
    use super::{Geometry, P2, SketchEntity, circle_through};
    use crate::values::field;
    use fs_interp::Value;
    use fs_interp::eval::R;
    use std::f64::consts::TAU;

    /// Where an entity's unknowns start in the vector, and their kind.
    #[derive(Clone, Copy)]
    enum Slot {
        Point(usize),
        Line(usize),
        /// Centre, radius, and start and end angles (the end past the start the way the arc runs).
        Arc(usize),
    }

    fn slots(entities: &[SketchEntity]) -> (Vec<Option<Slot>>, Vec<f64>) {
        let mut x = Vec::new();
        let mut out = Vec::new();
        for e in entities {
            let slot = match &e.geometry {
                Geometry::Point(p) => {
                    x.extend([p.0, p.1]);
                    Some(Slot::Point(x.len() - 2))
                }
                Geometry::Line(a, b) => {
                    x.extend([a.0, a.1, b.0, b.1]);
                    Some(Slot::Line(x.len() - 4))
                }
                // An arc's start and end run counterclockwise, whichever way it was given, as in Onshape (std's
                // hem relies on it).
                Geometry::Arc { start, mid, end } => match circle_through(*start, *mid, *end) {
                    Some((c, r)) => {
                        let ccw = (mid.0 - start.0) * (end.1 - mid.1) - (mid.1 - start.1) * (end.0 - mid.0) > 0.0;
                        let (from, to) = if ccw { (*start, *end) } else { (*end, *start) };
                        let a0 = (from.1 - c.1).atan2(from.0 - c.0);
                        let mut a1 = (to.1 - c.1).atan2(to.0 - c.0);
                        if a1 <= a0 {
                            a1 += TAU;
                        }
                        x.extend([c.0, c.1, r, a0, a1]);
                        Some(Slot::Arc(x.len() - 5))
                    }
                    None => None,
                },
                _ => None,
            };
            out.push(slot);
        }
        (out, x)
    }

    /// A reference to part of the sketch: a whole entity, or one of its points.
    enum Part {
        Entity(Slot),
        Point(Slot, &'static str),
    }

    fn part(entities: &[SketchEntity], slots: &[Option<Slot>], name: &str) -> Option<Part> {
        let (base, suffix) = match name.rsplit_once('.') {
            Some((b, s @ ("start" | "end" | "center"))) => (b, Some(s)),
            _ => (name, None),
        };
        let index = entities.iter().position(|e| &*e.id == base)?;
        let slot = slots[index]?;
        Some(match (suffix, slot) {
            (None, Slot::Point(_)) => Part::Point(slot, "point"),
            (None, _) => Part::Entity(slot),
            (Some(s), _) => Part::Point(
                slot,
                match s {
                    "start" => "start",
                    "end" => "end",
                    _ => "center",
                },
            ),
        })
    }

    fn point(x: &[f64], slot: Slot, which: &str) -> Option<P2> {
        match (slot, which) {
            (Slot::Point(i), _) => Some((x[i], x[i + 1])),
            (Slot::Line(i), "start") => Some((x[i], x[i + 1])),
            (Slot::Line(i), "end") => Some((x[i + 2], x[i + 3])),
            (Slot::Arc(i), "center") => Some((x[i], x[i + 1])),
            (Slot::Arc(i), "start") => Some((x[i] + x[i + 2] * x[i + 3].cos(), x[i + 1] + x[i + 2] * x[i + 3].sin())),
            (Slot::Arc(i), "end") => Some((x[i] + x[i + 2] * x[i + 4].cos(), x[i + 1] + x[i + 2] * x[i + 4].sin())),
            _ => None,
        }
    }

    fn line(x: &[f64], slot: Slot) -> Option<(P2, P2)> {
        match slot {
            Slot::Line(i) => Some(((x[i], x[i + 1]), (x[i + 2], x[i + 3]))),
            _ => None,
        }
    }

    /// The perpendicular distance from `p` to the line through `a` and `b`, signed.
    fn to_line(p: P2, a: P2, b: P2) -> f64 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy).max(1e-300);
        ((p.0 - a.0) * dy - (p.1 - a.1) * dx) / len
    }

    /// One constraint's residuals at `x` (each zero when it holds), given its values at the start `x0` for fixes.
    fn residuals(entities: &[SketchEntity], slots: &[Option<Slot>], c: &Value, x: &[f64], x0: &[f64]) -> Vec<f64> {
        let kind = field(c, "constraintType").as_str().unwrap_or("");
        let first = field(c, "localFirst").as_str().and_then(|n| part(entities, slots, n));
        let second = field(c, "localSecond").as_str().and_then(|n| part(entities, slots, n));
        let length = |key: &str| crate::values::length(field(c, key), "constraint length").ok();
        match (kind, first, second) {
            ("FIX", Some(Part::Point(s, w)), None) => match (point(x, s, w), point(x0, s, w)) {
                (Some(p), Some(q)) => vec![p.0 - q.0, p.1 - q.1],
                _ => vec![],
            },
            // A fixed arc keeps its centre and radius; its ends may still move round it.
            ("FIX", Some(Part::Entity(Slot::Arc(i))), None) => {
                vec![x[i] - x0[i], x[i + 1] - x0[i + 1], x[i + 2] - x0[i + 2]]
            }
            ("FIX", Some(Part::Entity(Slot::Line(i))), None) => (i..i + 4).map(|k| x[k] - x0[k]).collect(),
            ("COINCIDENT", Some(Part::Point(s1, w1)), Some(Part::Point(s2, w2))) => {
                match (point(x, s1, w1), point(x, s2, w2)) {
                    (Some(p), Some(q)) => vec![p.0 - q.0, p.1 - q.1],
                    _ => vec![],
                }
            }
            ("COINCIDENT", Some(Part::Point(s, w)), Some(Part::Entity(e)))
            | ("COINCIDENT", Some(Part::Entity(e)), Some(Part::Point(s, w))) => {
                let Some(p) = point(x, s, w) else { return vec![] };
                match e {
                    Slot::Line(_) => line(x, e).map(|(a, b)| vec![to_line(p, a, b)]).unwrap_or_default(),
                    Slot::Arc(i) => vec![(p.0 - x[i]).hypot(p.1 - x[i + 1]) - x[i + 2]],
                    Slot::Point(_) => vec![],
                }
            }
            ("TANGENT", Some(Part::Entity(Slot::Arc(i))), Some(Part::Entity(l)))
            | ("TANGENT", Some(Part::Entity(l)), Some(Part::Entity(Slot::Arc(i)))) => {
                line(x, l).map(|(a, b)| vec![to_line((x[i], x[i + 1]), a, b).abs() - x[i + 2]]).unwrap_or_default()
            }
            ("PERPENDICULAR" | "PARALLEL", Some(Part::Entity(l1)), Some(Part::Entity(l2))) => {
                match (line(x, l1), line(x, l2)) {
                    (Some((a, b)), Some((p, q))) => {
                        let (d1, d2) = ((b.0 - a.0, b.1 - a.1), (q.0 - p.0, q.1 - p.1));
                        let n = d1.0.hypot(d1.1) * d2.0.hypot(d2.1);
                        if n < 1e-300 {
                            return vec![];
                        }
                        vec![if kind == "PERPENDICULAR" {
                            (d1.0 * d2.0 + d1.1 * d2.1) / n
                        } else {
                            (d1.0 * d2.1 - d1.1 * d2.0) / n
                        }]
                    }
                    _ => vec![],
                }
            }
            ("HORIZONTAL" | "VERTICAL", Some(Part::Entity(l)), None) => match line(x, l) {
                Some((a, b)) => vec![if kind == "HORIZONTAL" { b.1 - a.1 } else { b.0 - a.0 }],
                None => vec![],
            },
            ("DISTANCE", Some(Part::Point(s1, w1)), Some(Part::Point(s2, w2))) => {
                let (Some(p), Some(q), Some(l)) = (point(x, s1, w1), point(x, s2, w2), length("length")) else {
                    return vec![];
                };
                let aligned = field(c, "alignment").as_str();
                let along = |d: f64| match aligned {
                    Some("ALIGNED") => d - l,
                    Some("ANTI_ALIGNED") => -d - l,
                    _ => d.abs() - l,
                };
                match field(c, "direction").as_str() {
                    Some("HORIZONTAL") => vec![along(q.0 - p.0)],
                    Some("VERTICAL") => vec![along(q.1 - p.1)],
                    // Smooth at the solution: a zero distance makes the points coincide.
                    _ if l < 1e-12 => vec![q.0 - p.0, q.1 - p.1],
                    _ => vec![((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2) - l * l) / (2.0 * l)],
                }
            }
            _ => vec![],
        }
    }

    fn all(
        entities: &[SketchEntity],
        slots: &[Option<Slot>],
        constraints: &[Value],
        x: &[f64],
        x0: &[f64],
    ) -> Vec<f64> {
        constraints.iter().flat_map(|c| residuals(entities, slots, c, x, x0)).collect()
    }

    /// Moves the entities to meet the constraints, if they don't already.
    pub fn solve(entities: &mut [SketchEntity], constraints: &[Value]) -> R<()> {
        if constraints.is_empty() {
            return Ok(());
        }
        let (slots, x0) = slots(entities);
        let norm = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut x = x0.clone();
        let mut r = all(entities, &slots, constraints, &x, &x0);
        if r.iter().all(|v| v.abs() < 1e-10) {
            return Ok(());
        }
        let n = x.len();
        let mut lambda = 1e-6;
        for _ in 0..200 {
            if norm(&r) < 1e-13 {
                break;
            }
            // The Jacobian by forward differences.
            let h = 1e-10;
            let mut jac = vec![vec![0.0; n]; r.len()];
            for k in 0..n {
                let mut xk = x.clone();
                xk[k] += h;
                let rk = all(entities, &slots, constraints, &xk, &x0);
                for (row, (a, b)) in jac.iter_mut().zip(rk.iter().zip(r.iter())) {
                    row[k] = (a - b) / h;
                }
            }
            // (JᵀJ + λI) δ = −Jᵀr
            let mut a = vec![vec![0.0; n]; n];
            let mut g = vec![0.0; n];
            for (row, ri) in jac.iter().zip(r.iter()) {
                for i in 0..n {
                    if row[i] == 0.0 {
                        continue;
                    }
                    g[i] -= row[i] * ri;
                    for j in 0..n {
                        a[i][j] += row[i] * row[j];
                    }
                }
            }
            let mut improved = false;
            for _ in 0..30 {
                let mut m = a.clone();
                for (i, row) in m.iter_mut().enumerate() {
                    row[i] += lambda * (1.0 + row[i]);
                }
                let Some(step) = crate::splines::solve(m, g.clone()) else {
                    lambda *= 10.0;
                    continue;
                };
                let trial: Vec<f64> = x.iter().zip(step.iter()).map(|(a, b)| a + b).collect();
                let rt = all(entities, &slots, constraints, &trial, &x0);
                if norm(&rt) < norm(&r) {
                    x = trial;
                    r = rt;
                    lambda = (lambda / 10.0).max(1e-12);
                    improved = true;
                    break;
                }
                lambda *= 10.0;
            }
            if !improved {
                break;
            }
        }
        // Write the solved geometry back.
        for (e, slot) in entities.iter_mut().zip(slots.iter()) {
            match (slot, &mut e.geometry) {
                (Some(Slot::Point(i)), Geometry::Point(p)) => *p = (x[*i], x[*i + 1]),
                (Some(Slot::Line(i)), Geometry::Line(a, b)) => {
                    *a = (x[*i], x[*i + 1]);
                    *b = (x[*i + 2], x[*i + 3]);
                }
                (Some(Slot::Arc(i)), Geometry::Arc { start, mid, end }) => {
                    let (cx, cy, rr, a0, a1) = (x[*i], x[*i + 1], x[*i + 2], x[*i + 3], x[*i + 4]);
                    let at = |a: f64| (cx + rr * a.cos(), cy + rr * a.sin());
                    *start = at(a0);
                    *mid = at((a0 + a1) / 2.0);
                    *end = at(a1);
                }
                _ => {}
            }
        }
        Ok(())
    }
}
