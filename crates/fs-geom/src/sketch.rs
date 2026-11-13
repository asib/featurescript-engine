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
    /// Its constraints' ids, each used once ("Duplicate id in sketch", batch 103 c003's first build).
    constraint_ids: RefCell<Vec<Rc<str>>>,
    /// Its entities' positions as Onshape's Part Studios write them (`skSetInitialGuess`): by entity id, an array of
    /// numbers whose layout depends on the entity (see [`from_guess`]).
    guess: RefCell<Option<Value>>,
    /// The lines its guess placed: Onshape's line is placed at their start, not their middle (batch 103).
    guessed_lines: RefCell<Vec<Rc<str>>>,
    /// Made by a Part Studio's own code (its tab's module), not a Feature Studio's: a spline's handles are drawn
    /// as construction lines from its ends rather than as points (batch 103 c006, c003).
    in_part_studio: bool,
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
    /// A B-spline as its poles, weights (empty if not rational), full knot vector and degree, over its knots'
    /// whole domain; `periodic` if Onshape flags it so though its knots aren't (a closed `skSpline`'s, batch 104).
    BSpline { poles: Vec<P2>, weights: Vec<f64>, knots: Vec<f64>, degree: usize, periodic: bool },
    /// An entity added without its geometry, as a Part Studio's sketch adds them, to come from the sketch's initial
    /// guess (batch 103).
    Pending(Pending),
    Conic {
        start: P2,
        control: P2,
        end: P2,
        rho: f64,
    },
}

/// What a pending entity is, to read its initial guess as.
#[derive(Clone, Copy, Debug)]
enum Pending {
    Point,
    Line,
    Circle,
    Arc,
    Bezier,
    /// A spline through points (`skInterpolatedSpline`, closed; `skInterpolatedSplineSegment`, open), with handles
    /// at its ends drawn in the sketch if `handles`.
    Interpolated { closed: bool, handles: bool },
    /// A spline by its poles and knots (`skSpline`, closed; `skSplineSegment`, open), as "Use" of a spline edge
    /// makes them (batch 104).
    Spline { closed: bool },
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
    m.insert("newSketch", |interp, args| {
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
            constraint_ids: RefCell::new(Vec::new()),
            guess: RefCell::new(None),
            guessed_lines: RefCell::new(Vec::new()),
            in_part_studio: interp.calling_module_kind() == Some(fs_interp::module::ModuleKind::PartStudio),
            solved: std::cell::Cell::new(false),
            flat_of,
        })))
    });
    m.insert("isSketch", |_, args| {
        let v = arg(&args, 0, "isSketch")?;
        Ok(Value::bool(matches!(&v.kind, Kind::Builtin(n) if n.as_any().is::<Sketch>())))
    });
    m.insert("skPoint", |_, args| {
        let id = add(&args, "skPoint", |v| {
            if unplaced(v, &["position"]) {
                return Ok(Geometry::Pending(Pending::Point));
            }
            Ok(Geometry::Point(point(v, "position", "skPoint position")?))
        })?;
        Ok(values::map(vec![("pointId", Value::string(id))]))
    });
    m.insert("skLineSegment", |_, args| {
        let id = add(&args, "skLineSegment", |v| {
            if unplaced(v, &["start", "end"]) {
                return Ok(Geometry::Pending(Pending::Line));
            }
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
    // Text as Onshape's font engine draws it (batch 091, exactly): the font's outlines (fs-geom/fonts) as FreeType
    // loads them unhinted at 100 pixels to the em, in 64ths of a pixel (font units × 6400 / units per em, rounded
    // half away from zero; TrueType's implied points midway between those), the box's height 4608 of them (0.72
    // em), the baseline on its bottom, each glyph after the last by its rounded advance (no kerning); lines and
    // quadratic Béziers, and a construction frame as tall as the box, a quarter of a space (rounded down) wider
    // than the ink, with a line down its middle.
    m.insert("skText", |_, args| {
        let sketch = sketch_arg(&args, "skText")?;
        let id: Rc<str> = string(&args, 1, "skText")?.into();
        let value = arg(&args, 2, "skText")?;
        if sketch.solved.get() {
            return Err(error("@skText: the sketch has already been solved"));
        }
        let text = field(value, "text").as_str().ok_or_else(|| error("@skText: text must be a string"))?.to_string();
        let font_name = field(value, "fontName").as_str().unwrap_or("OpenSans-Regular.ttf");
        let font = crate::font::by_name(font_name)
            .ok_or_else(|| error(format!("@skText: the font {font_name} isn't one the engine has (fs-geom/fonts)")))?;
        for key in ["mirrorHorizontal", "mirrorVertical"] {
            if field(value, key).as_bool() == Some(true) {
                return Err(error(format!("@skText: {key} isn't supported yet")));
            }
        }
        let first = point(value, "firstCorner", "skText firstCorner")?;
        let second = point(value, "secondCorner", "skText secondCorner")?;
        let (x0, y0) = (first.0.min(second.0), first.1.min(second.1));
        let height = (second.1 - first.1).abs();
        if height <= 0.0 {
            return Err(error("@skText: the box has no height"));
        }
        // Font units to 64ths of a pixel at 100 pixels to the em, and those to lengths.
        let scale = 6400.0 / font.units_per_em;
        let ft = |u: f64| ((u * scale).abs() + 0.5).floor().copysign(u);
        let length = height / 4608.0;
        let mut glyphs = Vec::new();
        let (mut pen, mut ink) = (0.0, None::<f64>);
        for c in text.chars() {
            let index = font.glyph_index(c);
            let mut glyph = font.glyph(index);
            for contour in &mut glyph.contours {
                for point in contour.iter_mut() {
                    *point = (ft(point.0) + pen, ft(point.1), point.2);
                }
            }
            if !glyph.contours.is_empty() {
                let right = pen + ft(glyph.x_max);
                ink = Some(ink.map_or(right, |x: f64| x.max(right)));
            }
            glyphs.push(glyph);
            pen += ft(font.advance(index));
        }
        let space = ft(font.advance(font.glyph_index(' ')));
        let width = (ink.unwrap_or(0.0) + (space / 4.0).floor()) * length;
        let mut entities = sketch.entities.borrow_mut();
        let mut add = |name: String, geometry: Geometry, construction: bool| -> R<()> {
            let entity_id: Rc<str> = format!("{id}.{name}").into();
            if entities.iter().any(|e| e.id == entity_id) {
                return Err(error(format!("@skText: sketch entity id {entity_id:?} is already used")));
            }
            entities.push(SketchEntity { id: entity_id, geometry, construction });
            Ok(())
        };
        let (x1, y1) = (x0 + width, y0 + height);
        let frame = [
            ("bottom", (x0, y0), (x1, y0)),
            ("top", (x0, y1), (x1, y1)),
            ("left", (x0, y0), (x0, y1)),
            ("middle", (x0 + width / 2.0, y0), (x0 + width / 2.0, y1)),
            ("right", (x1, y0), (x1, y1)),
        ];
        for (name, a, b) in frame {
            add(format!("frame.{name}"), Geometry::Line(a, b), true)?;
        }
        for (i, glyph) in glyphs.iter().enumerate() {
            for (k, contour) in glyph.contours.iter().enumerate() {
                for (j, piece) in crate::font::segments(contour).into_iter().enumerate() {
                    let points: Vec<P2> = piece.iter().map(|&(x, y)| (x0 + x * length, y0 + y * length)).collect();
                    if points.windows(2).all(|w| w[0] == w[1]) {
                        continue;
                    }
                    let geometry =
                        if points.len() == 2 { Geometry::Line(points[0], points[1]) } else { Geometry::Bezier(points) };
                    add(format!("{i}.{k}.{j}"), geometry, false)?;
                }
            }
        }
        Ok(Value::UNDEFINED)
    });
    m.insert("skCircle", |_, args| {
        let id = add(&args, "skCircle", |v| {
            if unplaced(v, &["center", "radius"]) {
                return Ok(Geometry::Pending(Pending::Circle));
            }
            let center = point(v, "center", "skCircle center")?;
            let radius = values::length(field(v, "radius"), "skCircle radius")?;
            Ok(Geometry::Circle { center, radius })
        })?;
        Ok(ids(&id, &[("centerId", "center")]))
    });
    m.insert("skArc", |_, args| {
        let id = add(&args, "skArc", |v| {
            if unplaced(v, &["start", "mid", "end"]) {
                return Ok(Geometry::Pending(Pending::Arc));
            }
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
        let id = add(&args, "skBezier", |v| {
            if unplaced(v, &["points"]) {
                return Ok(Geometry::Pending(Pending::Bezier));
            }
            Ok(Geometry::Bezier(points(v, "points", "skBezier points")?))
        })?;
        let sketch = sketch_arg(&args, "skBezier")?;
        // Its ends snap to its first and last control points, constraints of its own (a Part Studio's Bézier, given
        // its points, then fails to add its own "<id>.startSnap": batch 103 c003's first build).
        if !matches!(sketch.entities.borrow().last().map(|e| &e.geometry), Some(Geometry::Pending(_))) {
            let mut ids = sketch.constraint_ids.borrow_mut();
            ids.extend([format!("{id}.startSnap").into(), format!("{id}.endSnap").into()]);
        }
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
    // Splines through points, as a Part Studio's sketch adds them: their points (and an open one's end derivatives)
    // come from the initial guess.
    m.insert("skInterpolatedSpline", |_, args| {
        add(&args, "skInterpolatedSpline", |_| {
            Ok(Geometry::Pending(Pending::Interpolated { closed: true, handles: false }))
        })?;
        Ok(Value::UNDEFINED)
    });
    m.insert("skInterpolatedSplineSegment", |_, args| {
        add(&args, "skInterpolatedSplineSegment", |v| {
            let handles = field(v, ".hasHandlesInSketch").as_bool().unwrap_or(false);
            Ok(Geometry::Pending(Pending::Interpolated { closed: false, handles }))
        })?;
        Ok(Value::UNDEFINED)
    });
    m.insert("skSpline", |_, args| {
        add(&args, "skSpline", |_| Ok(Geometry::Pending(Pending::Spline { closed: true })))?;
        Ok(Value::UNDEFINED)
    });
    m.insert("skSplineSegment", |_, args| {
        add(&args, "skSplineSegment", |_| Ok(Geometry::Pending(Pending::Spline { closed: false })))?;
        Ok(Value::UNDEFINED)
    });
    m.insert("skSetInitialGuess", |_, args| {
        let sketch = sketch_arg(&args, "skSetInitialGuess")?;
        let guess = arg(&args, 1, "skSetInitialGuess")?;
        if guess.as_map().is_none() {
            return Err(error("@skSetInitialGuess: the initial guess must be a map"));
        }
        *sketch.guess.borrow_mut() = Some(guess.clone());
        Ok(Value::UNDEFINED)
    });
    m.insert("skConstraint", |_, args| {
        let sketch = sketch_arg(&args, "skConstraint")?;
        let id: Rc<str> = string(&args, 1, "skConstraint")?.into();
        if sketch.constraint_ids.borrow().contains(&id) {
            return Err(error(format!("@skConstraint: Duplicate id in sketch: {id}, sketch = ")));
        }
        sketch.constraint_ids.borrow_mut().push(id);
        sketch.constraints.borrow_mut().push(arg(&args, 2, "skConstraint")?.clone());
        Ok(Value::UNDEFINED)
    });
    m.insert("skSolve", |interp, args| {
        let sketch = sketch_arg(&args, "skSolve")?;
        if sketch.solved.replace(true) {
            return Err(error("@skSolve: the sketch has already been solved"));
        }
        tags(interp)?;
        // An entity both placed by its own fields and given a guess fails the solve, which then adds nothing
        // ("Solve for sketch fails", batch 103: a Bézier given points with Onshape's guess for it, even the same).
        if !place_from_guess(sketch)? {
            return Ok(Value::UNDEFINED);
        }
        constraints::solve(&mut sketch.entities.borrow_mut(), &sketch.constraints.borrow())?;
        model_of(&sketch.context, "skSolve", |model| solve(model, sketch))?;
        Ok(Value::UNDEFINED)
    });
}

/// Whether a sketch call gives none of its entity's geometric fields (a Part Studio's sketch, whose geometry is in
/// its initial guess).
fn unplaced(value: &Value, keys: &[&str]) -> bool {
    keys.iter().all(|k| field(value, k).is_undefined())
}

/// Places the sketch's pending entities from its initial guess. False if an entity placed by its own fields has a
/// guess too: the solve fails.
fn place_from_guess(sketch: &Sketch) -> R<bool> {
    let guess = sketch.guess.borrow();
    let mut entities = sketch.entities.borrow_mut();
    let mut placed = Vec::with_capacity(entities.len());
    for mut entity in entities.drain(..) {
        let numbers = match guess.as_ref().map(|g| field(g, &entity.id)) {
            Some(v) if !v.is_undefined() => values::numbers(v, "initial guess")?,
            _ => {
                placed.push(entity);
                continue;
            }
        };
        let Geometry::Pending(kind) = entity.geometry else { return Ok(false) };
        let (geometry, extras) = from_guess(&entity.id, kind, &numbers, sketch.in_part_studio)?;
        if matches!(geometry, Geometry::Line(..)) {
            sketch.guessed_lines.borrow_mut().push(entity.id.clone());
        }
        entity.geometry = geometry;
        // Its points and construction lines come before it, as Onshape lists their bodies (c003, c006).
        placed.extend(extras);
        placed.push(entity);
    }
    *entities = placed;
    Ok(true)
}

/// An entity's geometry from its initial guess (batch 103: Onshape's "Show code" of a sketch drawn in its UI, and
/// the curves its Part Studio built from it), and the points and construction lines that come with it:
///
/// - a point: x, y;
/// - a line: its midpoint, unit direction, and start and end parameters along it (the half length either way);
/// - a circle: its centre, x direction, radius and -1;
/// - an arc: a circle's and its start and end angles from that x, counterclockwise;
/// - a Bézier: 1, rational, periodic, its degree and its points' count, the points, its knot vector (clamped at 0
///   and 1) and its parameter range; with each control point a construction point, and its control polygon;
/// - a spline by its poles (batch 104): periodic, rational, its degree and its poles' count, the poles, its weights
///   if rational, its full knot vector, and for an open one its parameter range (a closed one's knots are unclamped
///   with its degree's multiplicity at 0 and 1, its first pole repeated last);
/// - a spline through points: periodic, twice its points' count, the points (closed: nothing more), and for an open
///   one its end derivatives along a [0, 1] parameter, its handles' points (the second and second-last poles of
///   the cubic its centripetal parameters make with those derivatives) and its parameter range; with each point a
///   point, and with handles drawn in the sketch, each handle: in a Part Studio's sketch a construction line from
///   its end (c006), in a Feature Studio's a point (c003).
fn from_guess(id: &Rc<str>, kind: Pending, g: &[f64], in_part_studio: bool) -> R<(Geometry, Vec<SketchEntity>)> {
    let bad = || error(format!("@skSolve: the initial guess for {id:?} ({kind:?}) isn't one the engine reads: {g:?}"));
    let at = |i: usize| g.get(i).copied().ok_or_else(bad);
    let pt = |i: usize| -> R<P2> { Ok((at(i)?, at(i + 1)?)) };
    let sub = |name: String, geometry: Geometry, construction: bool| SketchEntity { id: name.into(), geometry, construction };
    let mut extras = Vec::new();
    let geometry = match kind {
        Pending::Point => Geometry::Point(pt(0)?),
        Pending::Line => {
            let (m, d, t0, t1) = (pt(0)?, pt(2)?, at(4)?, at(5)?);
            Geometry::Line((m.0 + t0 * d.0, m.1 + t0 * d.1), (m.0 + t1 * d.0, m.1 + t1 * d.1))
        }
        Pending::Circle => Geometry::Circle { center: pt(0)?, radius: at(4)? },
        Pending::Arc => {
            let (c, x, r, a0, a1) = (pt(0)?, pt(2)?, at(4)?, at(6)?, at(7)?);
            let y = (-x.1, x.0);
            let on = |a: f64| (c.0 + r * (a.cos() * x.0 + a.sin() * y.0), c.1 + r * (a.cos() * x.1 + a.sin() * y.1));
            Geometry::Arc { start: on(a0), mid: on((a0 + a1) / 2.0), end: on(a1) }
        }
        Pending::Bezier => {
            let (rational, degree, count) = (at(1)? != 0.0, at(3)?, at(4)? as usize);
            if rational || degree as usize + 1 != count {
                return Err(bad());
            }
            let poles: Vec<P2> = (0..count).map(|k| pt(5 + 2 * k)).collect::<R<_>>()?;
            for (k, p) in poles.iter().enumerate() {
                extras.push(sub(format!("{id}.{k}.internal"), Geometry::Point(*p), true));
            }
            for (k, w) in poles.windows(2).enumerate() {
                extras.push(sub(format!("{id}.polygon{k}"), Geometry::Line(w[0], w[1]), true));
            }
            Geometry::Bezier(poles)
        }
        Pending::Spline { closed } => {
            let (rational, degree, count) = (at(1)? != 0.0, at(2)? as usize, at(3)? as usize);
            if (at(0)? != 0.0) != closed || degree == 0 || count <= degree {
                return Err(bad());
            }
            let poles: Vec<P2> = (0..count).map(|k| pt(4 + 2 * k)).collect::<R<_>>()?;
            let mut next = 4 + 2 * count;
            let weights: Vec<f64> = if rational {
                next += count;
                (0..count).map(|k| at(4 + 2 * count + k)).collect::<R<_>>()?
            } else {
                Vec::new()
            };
            let knots: Vec<f64> = (0..count + degree + 1).map(|k| at(next + k)).collect::<R<_>>()?;
            next += count + degree + 1;
            // An open one's parameter range is its knots' whole domain (the only one seen).
            if !closed && (at(next)? - knots[degree]).abs() + (at(next + 1)? - knots[count]).abs() > 1e-12 {
                return Err(bad());
            }
            Geometry::BSpline { poles, weights, knots, degree, periodic: closed }
        }
        Pending::Interpolated { closed, handles } => {
            let count = at(1)? as usize / 2;
            if count < 2 || (at(0)? != 0.0) != closed {
                return Err(bad());
            }
            let mut points: Vec<P2> = (0..count).map(|k| pt(2 + 2 * k)).collect::<R<_>>()?;
            for (k, p) in points.iter().enumerate() {
                extras.push(sub(format!("{id}.{k}.internal"), Geometry::Point(*p), false));
            }
            let base = 2 + 2 * count;
            let derivatives = if closed || g.len() < base + 8 {
                (None, None)
            } else {
                if handles {
                    let ends = [(0, points[0], pt(base + 4)?), (count - 1, points[count - 1], pt(base + 6)?)];
                    for (k, end, handle) in ends {
                        let name = format!("{id}.{k}.handle");
                        extras.push(if in_part_studio {
                            sub(name, Geometry::Line(end, handle), true)
                        } else {
                            sub(name, Geometry::Point(handle), false)
                        });
                    }
                }
                (Some(pt(base)?), Some(pt(base + 2)?))
            };
            if closed {
                points.push(points[0]);
            }
            Geometry::FitSpline { points, derivatives, parameters: Vec::new() }
        }
    };
    Ok((geometry, extras))
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
    // Curves Onshape flags periodic though OCCT's can't be (`Info::flagged_periodic`).
    let mut flagged: Vec<Rc<str>> = Vec::new();
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
            Geometry::BSpline { poles, weights, knots, degree, periodic } => {
                if *periodic {
                    flagged.push(entity.id.clone());
                }
                let poles: Vec<occt::Vec3> = poles.iter().map(|p| world(*p)).collect();
                occt::bspline_edge(&poles, weights, knots, *degree as i32, false).map_err(occt_error)?
            }
            Geometry::Pending(kind) => {
                return Err(error(format!("@skSolve: sketch entity {:?} ({kind:?}) has no geometry and no initial guess", entity.id)));
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
    // Curves: a wire body each, closed curves' after the open ones' (batch 103 c003, c006); a construction curve's
    // edge and vertices are construction.
    let (open, closed): (Vec<_>, Vec<_>) = edges.into_iter().partition(|(_, e, _)| !e.is_closed_edge());
    let guessed_lines = sketch.guessed_lines.borrow();
    for (id, edge, construction) in open.into_iter().chain(closed) {
        let line_origin = guessed_lines
            .contains(&id)
            .then(|| ends.iter().find(|(curve, _, _)| *curve == id).map(|(_, a, _)| *a))
            .flatten();
        let info = Rc::new(Info {
            created_by: sketch.id.clone(),
            sketch_entity: Some(id.clone()),
            cap: None,
            dependency: None,
            construction,
            line_origin,
            flagged_periodic: flagged.contains(&id),
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
        /// Where its middle started, its direction (an angle), its length and the length it started with: its start
        /// is the first less half the second along the direction, so a change of length moves its end.
        Line(usize),
        /// Centre, radius, and start and end angles (the end past the start the way the arc runs).
        Arc(usize),
        /// Centre and radius.
        Circle(usize),
    }

    /// Each entity's slot, the unknowns as the sketch starts, each unknown's scale (so that moving it by one is
    /// moving a point by one), and the order in which Onshape's solver lets them move (batch 136): the groups it
    /// holds hardest first, each entity's direction, length or radius, oldest entity first, then each entity's place,
    /// oldest first. A line's end moves when its length changes, its middle stays when it turns.
    fn slots(entities: &[SketchEntity], fixed: bool) -> (Vec<Option<Slot>>, Vec<f64>, Vec<f64>, Vec<Vec<usize>>) {
        let mut x = Vec::new();
        let mut scale = Vec::new();
        let mut out = Vec::new();
        // Where lines sit holds before where circles, arcs and points do: a circle drawn off a line it's held
        // tangent to (or a line's end held on it) moves its centre, whichever was drawn first (batch 150 c002–c004,
        // c006).
        let (mut stiff, mut place, mut line_place): (Vec<Vec<usize>>, Vec<Vec<usize>>, Vec<Vec<usize>>) =
            (Vec::new(), Vec::new(), Vec::new());
        for e in entities {
            let i = x.len();
            let slot = match &e.geometry {
                Geometry::Point(p) => {
                    x.extend([p.0, p.1]);
                    scale.extend([1.0, 1.0]);
                    place.push(vec![i, i + 1]);
                    Some(Slot::Point(i))
                }
                Geometry::Line(a, b) => {
                    let length = (b.0 - a.0).hypot(b.1 - a.1);
                    x.extend([(a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, (b.1 - a.1).atan2(b.0 - a.0), length, length]);
                    scale.extend([1.0, 1.0, (length / 2.0).max(1e-9), 1.0, 1.0]);
                    stiff.push(vec![i + 2, i + 3]);
                    line_place.push(vec![i, i + 1]);
                    Some(Slot::Line(i))
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
                        scale.extend([1.0, 1.0, 1.0, r, r]);
                        stiff.push(vec![i + 2]);
                        place.push(vec![i, i + 1, i + 3, i + 4]);
                        Some(Slot::Arc(i))
                    }
                    None => None,
                },
                Geometry::Circle { center, radius } => {
                    x.extend([center.0, center.1, *radius]);
                    scale.extend([1.0, 1.0, 1.0]);
                    stiff.push(vec![i + 2]);
                    place.push(vec![i, i + 1]);
                    Some(Slot::Circle(i))
                }
                _ => None,
            };
            out.push(slot);
        }
        // A sketch with something fixed holds its lines' places, oldest first, before its circles' and points' (std's
        // tear-drop hem, batch 029; a mirror across a fixed axis, batch 135 c019).
        if fixed {
            stiff.extend(line_place);
            stiff.extend(place);
            return (out, x, scale, stiff);
        }
        // With nothing fixed, of the lines Onshape keeps where it was the one starting first, by x and then y (batch 155: the lower of
        // two lines held tangent to circles, the left of two turned a quarter turn, whichever was drawn or constrained
        // first or drawn further off; batch 150 c007's line through the origin): all of it holds hardest. The others
        // keep their lengths, give way after the circles and points, turning (about their middles) only where sliding
        // won't do, then sliding along their main axis, and last across it (a mostly level line moves straight up or
        // down, batch 154 c006; two lines' ends held together, the second slides, batch 150 c007).
        let lines: Vec<usize> =
            out.iter().filter_map(|s| match s { Some(Slot::Line(i)) => Some(*i), _ => None }).collect();
        let start = |i: usize| (x[i] - x[i + 4] / 2.0 * x[i + 2].cos(), x[i + 1] - x[i + 4] / 2.0 * x[i + 2].sin());
        let anchor = lines.iter().copied().min_by(|&a, &b| {
            let (pa, pb) = (start(a), start(b));
            if (pa.0 - pb.0).abs() > 1e-9 { pa.0.total_cmp(&pb.0) } else { pa.1.total_cmp(&pb.1) }
        });
        let mut groups: Vec<Vec<usize>> = Vec::new();
        if let Some(a) = anchor {
            groups.push(vec![a, a + 1, a + 2, a + 3]);
        }
        let is_anchor = |g: &Vec<usize>| anchor.is_some_and(|a| g.contains(&a) || g.contains(&(a + 2)));
        // (Two circles held tangent to the held line and to another: the circles move the shortest way onto the held
        // line, the other turns about its middle onto them, batch 154 c003.)
        let mut turns = Vec::new();
        for g in stiff.into_iter().filter(|g| !is_anchor(g)) {
            match lines.iter().find(|&&i| g.contains(&(i + 2))) {
                Some(&i) => {
                    groups.push(vec![i + 3]);
                    turns.push(vec![i + 2]);
                }
                None => groups.push(g),
            }
        }
        let (mut along, mut across) = (Vec::new(), Vec::new());
        for g in line_place.into_iter().filter(|g| !is_anchor(g)) {
            let i = g[0];
            let level = x[i + 2].cos().abs() >= x[i + 2].sin().abs();
            along.push(vec![if level { i } else { i + 1 }]);
            across.push(vec![if level { i + 1 } else { i }]);
        }
        groups.extend(place);
        groups.extend(turns);
        groups.extend(along);
        groups.extend(across);
        (out, x, scale, groups)
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
            (Slot::Line(_), "start") => line(x, slot).map(|(a, _)| a),
            (Slot::Line(_), "end") => line(x, slot).map(|(_, b)| b),
            (Slot::Arc(i) | Slot::Circle(i), "center") => Some((x[i], x[i + 1])),
            (Slot::Arc(i), "start") => Some((x[i] + x[i + 2] * x[i + 3].cos(), x[i + 1] + x[i + 2] * x[i + 3].sin())),
            (Slot::Arc(i), "end") => Some((x[i] + x[i + 2] * x[i + 4].cos(), x[i + 1] + x[i + 2] * x[i + 4].sin())),
            _ => None,
        }
    }

    fn line(x: &[f64], slot: Slot) -> Option<(P2, P2)> {
        match slot {
            Slot::Line(i) => {
                let ((s, c), h, l) = (x[i + 2].sin_cos(), x[i + 4] / 2.0, x[i + 3]);
                let start = (x[i] - h * c, x[i + 1] - h * s);
                Some((start, (start.0 + l * c, start.1 + l * s)))
            }
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
        let named = |key: &str| field(c, key).as_str().and_then(|n| part(entities, slots, n));
        // A midpoint given as three points, as a sketch drawn in Onshape writes it.
        if kind == "MIDPOINT" {
            if let (Some(Part::Point(s, w)), Some(Part::Point(s1, w1)), Some(Part::Point(s2, w2))) =
                (named("localMidpoint"), named("localEntity1"), named("localEntity2"))
            {
                return match (point(x, s, w), point(x, s1, w1), point(x, s2, w2)) {
                    (Some(p), Some(a), Some(b)) => vec![p.0 - (a.0 + b.0) / 2.0, p.1 - (a.1 + b.1) / 2.0],
                    _ => vec![],
                };
            }
        }
        // The second the first's mirror image across a line: a point the other's reflection; a line on the other's
        // reflection, wherever along it (Onshape's lines are mirrored as lines: batch 135 c019).
        if kind == "MIRROR" {
            let Some(Part::Entity(axis @ Slot::Line(_))) = named("localMirror") else { return vec![] };
            let Some((a, b)) = line(x, axis) else { return vec![] };
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let n2 = (dx * dx + dy * dy).max(1e-300);
            let reflect = |p: P2| {
                let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / n2;
                let foot = (a.0 + t * dx, a.1 + t * dy);
                (2.0 * foot.0 - p.0, 2.0 * foot.1 - p.1)
            };
            let pairs: Vec<(P2, P2)> = match (first, second) {
                (Some(Part::Point(s1, w1)), Some(Part::Point(s2, w2))) => {
                    point(x, s1, w1).zip(point(x, s2, w2)).into_iter().collect()
                }
                (Some(Part::Entity(l1 @ Slot::Line(_))), Some(Part::Entity(l2 @ Slot::Line(_)))) => {
                    return match (line(x, l1), line(x, l2)) {
                        (Some((p, q)), Some((r, t))) => {
                            let (p, q) = (reflect(p), reflect(q));
                            vec![to_line(r, p, q), to_line(t, p, q)]
                        }
                        _ => vec![],
                    };
                }
                (Some(Part::Entity(Slot::Arc(i) | Slot::Circle(i))), Some(Part::Entity(Slot::Arc(j) | Slot::Circle(j)))) => {
                    let m = reflect((x[i], x[i + 1]));
                    return vec![x[j] - m.0, x[j + 1] - m.1, x[j + 2] - x[i + 2]];
                }
                _ => vec![],
            };
            return pairs
                .into_iter()
                .flat_map(|(p, q)| {
                    let m = reflect(p);
                    [q.0 - m.0, q.1 - m.1]
                })
                .collect();
        }
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
            ("FIX", Some(Part::Entity(Slot::Circle(i))), None) => (i..i + 3).map(|k| x[k] - x0[k]).collect(),
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
                    Slot::Arc(i) | Slot::Circle(i) => vec![(p.0 - x[i]).hypot(p.1 - x[i + 1]) - x[i + 2]],
                    Slot::Point(_) => vec![],
                }
            }
            ("TANGENT", Some(Part::Entity(Slot::Arc(i) | Slot::Circle(i))), Some(Part::Entity(l @ Slot::Line(_))))
            | ("TANGENT", Some(Part::Entity(l @ Slot::Line(_))), Some(Part::Entity(Slot::Arc(i) | Slot::Circle(i)))) => {
                line(x, l).map(|(a, b)| vec![to_line((x[i], x[i + 1]), a, b).abs() - x[i + 2]]).unwrap_or_default()
            }
            // Round curves touching: outside each other or one inside the other, as they start.
            ("TANGENT", Some(Part::Entity(Slot::Arc(i) | Slot::Circle(i))), Some(Part::Entity(Slot::Arc(j) | Slot::Circle(j)))) => {
                let gap = |x: &[f64]| (x[i] - x[j]).hypot(x[i + 1] - x[j + 1]);
                let (outside, inside) = (x0[i + 2] + x0[j + 2], (x0[i + 2] - x0[j + 2]).abs());
                if (gap(x0) - outside).abs() <= (gap(x0) - inside).abs() {
                    vec![gap(x) - x[i + 2] - x[j + 2]]
                } else {
                    vec![gap(x) - (x[i + 2] - x[j + 2]).abs()]
                }
            }
            ("LENGTH", Some(Part::Entity(l)), None) => match (line(x, l), length("length")) {
                (Some((a, b)), Some(d)) => vec![match field(c, "direction").as_str() {
                    Some("HORIZONTAL") => (b.0 - a.0).abs() - d,
                    Some("VERTICAL") => (b.1 - a.1).abs() - d,
                    _ => (b.0 - a.0).hypot(b.1 - a.1) - d,
                }],
                _ => vec![],
            },
            ("RADIUS" | "DIAMETER", Some(Part::Entity(Slot::Arc(i) | Slot::Circle(i))), None) => match length("length") {
                Some(d) => vec![if kind == "RADIUS" { x[i + 2] - d } else { 2.0 * x[i + 2] - d }],
                None => vec![],
            },
            ("EQUAL", Some(Part::Entity(e1)), Some(Part::Entity(e2))) => match (e1, e2) {
                (Slot::Line(_), Slot::Line(_)) => match (line(x, e1), line(x, e2)) {
                    (Some((a, b)), Some((p, q))) => vec![(b.0 - a.0).hypot(b.1 - a.1) - (q.0 - p.0).hypot(q.1 - p.1)],
                    _ => vec![],
                },
                (Slot::Arc(i) | Slot::Circle(i), Slot::Arc(j) | Slot::Circle(j)) => vec![x[i + 2] - x[j + 2]],
                _ => vec![],
            },
            ("CONCENTRIC", Some(a), Some(b)) => {
                let centre = |p: &Part| match *p {
                    Part::Entity(Slot::Arc(i) | Slot::Circle(i)) => Some((x[i], x[i + 1])),
                    Part::Point(s, w) => point(x, s, w),
                    _ => None,
                };
                match (centre(&a), centre(&b)) {
                    (Some(p), Some(q)) => vec![p.0 - q.0, p.1 - q.1],
                    _ => vec![],
                }
            }
            // A line normal to a round curve: on a line through its centre.
            ("NORMAL", Some(Part::Entity(l @ Slot::Line(_))), Some(Part::Entity(Slot::Arc(i) | Slot::Circle(i))))
            | ("NORMAL", Some(Part::Entity(Slot::Arc(i) | Slot::Circle(i))), Some(Part::Entity(l @ Slot::Line(_)))) => {
                line(x, l).map(|(a, b)| vec![to_line((x[i], x[i + 1]), a, b)]).unwrap_or_default()
            }
            // Between lines (parallel ones): the second's middle that far from the first's line.
            ("DISTANCE", Some(Part::Entity(l1 @ Slot::Line(_))), Some(Part::Entity(l2 @ Slot::Line(_)))) => {
                match (line(x, l1), line(x, l2), length("length")) {
                    (Some((a, b)), Some((p, q)), Some(d)) => {
                        vec![to_line(((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0), a, b).abs() - d]
                    }
                    _ => vec![],
                }
            }
            ("DISTANCE", Some(Part::Point(s, w)), Some(Part::Entity(l @ Slot::Line(_))))
            | ("DISTANCE", Some(Part::Entity(l @ Slot::Line(_))), Some(Part::Point(s, w))) => {
                match (point(x, s, w), line(x, l), length("length")) {
                    (Some(p), Some((a, b)), Some(d)) => vec![to_line(p, a, b).abs() - d],
                    _ => vec![],
                }
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
    /// A small symmetric matrix's eigenvalues and eigenvectors (as columns), by Jacobi rotations.
    fn eigen(mut a: Vec<Vec<f64>>) -> (Vec<f64>, Vec<Vec<f64>>) {
        let n = a.len();
        let mut v: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
        for _ in 0..100 {
            let off: f64 = (0..n).flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j))).map(|(i, j)| a[i][j] * a[i][j]).sum();
            let diag: f64 = (0..n).map(|i| a[i][i] * a[i][i]).sum();
            if off <= 1e-30 * diag.max(1e-300) {
                break;
            }
            for p in 0..n {
                for q in p + 1..n {
                    if a[p][q].abs() < 1e-300 {
                        continue;
                    }
                    let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                    let t = if theta >= 0.0 { 1.0 } else { -1.0 } / (theta.abs() + (theta * theta + 1.0).sqrt());
                    let c = 1.0 / (t * t + 1.0).sqrt();
                    let s = t * c;
                    for k in 0..n {
                        let (kp, kq) = (a[k][p], a[k][q]);
                        a[k][p] = c * kp - s * kq;
                        a[k][q] = s * kp + c * kq;
                    }
                    for k in 0..n {
                        let (pk, qk) = (a[p][k], a[q][k]);
                        a[p][k] = c * pk - s * qk;
                        a[q][k] = s * pk + c * qk;
                    }
                    for row in v.iter_mut() {
                        let (kp, kq) = (row[p], row[q]);
                        row[p] = c * kp - s * kq;
                        row[q] = s * kp + c * kq;
                    }
                }
            }
        }
        ((0..n).map(|i| a[i][i]).collect(), v)
    }

    /// The least-norm solution of the columns `cols` (each of length m) times x = b, in the least-squares sense,
    /// directions in which the columns' Gram matrix is under `floor` left out.
    fn least_norm(cols: &[Vec<f64>], b: &[f64], floor: f64) -> Vec<f64> {
        let k = cols.len();
        let gram: Vec<Vec<f64>> =
            (0..k).map(|i| (0..k).map(|j| cols[i].iter().zip(&cols[j]).map(|(a, b)| a * b).sum()).collect()).collect();
        let atb: Vec<f64> = cols.iter().map(|c| c.iter().zip(b).map(|(a, b)| a * b).sum()).collect();
        let (values, vectors) = eigen(gram);
        // Directions the columns all but miss are left out: a rank-one Gram matrix's other eigenvalue comes out of
        // rounding a little above nothing, and dividing by it would send the step off along it (a circle held
        // tangent to a line, sent sliding along it, batch 154 c001).
        let top = values.iter().fold(0.0f64, |acc, v| acc.max(*v));
        let mut x = vec![0.0; k];
        for (j, &l) in values.iter().enumerate() {
            if l <= floor || l <= 1e-12 * top || l <= 1e-300 {
                continue;
            }
            let coefficient = (0..k).map(|i| vectors[i][j] * atb[i]).sum::<f64>() / l;
            for i in 0..k {
                x[i] += coefficient * vectors[i][j];
            }
        }
        x
    }

    /// The step `d` with `jac d = b` moving each group in turn (the first first) as little as the later ones let it,
    /// each unknown measured in units of its `scale`.
    fn lexicographic(jac: &[Vec<f64>], b: &[f64], groups: &[Vec<usize>], scale: &[f64]) -> Vec<f64> {
        let m = jac.len();
        let column = |k: usize| -> Vec<f64> { (0..m).map(|i| jac[i][k] / scale[k]).collect() };
        let mut b = b.to_vec();
        let mut d = vec![0.0; scale.len()];
        // What a group's columns leave once the later groups' are taken out counts only above rounding of the
        // columns themselves: where the later groups can do all a group does, it doesn't move.
        let floor = 1e-20
            * groups
                .iter()
                .flatten()
                .map(|&k| column(k).iter().map(|v| v * v).sum::<f64>())
                .fold(0.0f64, f64::max);
        for (g, group) in groups.iter().enumerate() {
            let rest: Vec<Vec<f64>> = groups[g + 1..].iter().flatten().map(|&k| column(k)).collect();
            // The part of a vector the later groups can't make: its component outside their columns' span.
            let basis: Vec<Vec<f64>> = if rest.is_empty() || m == 0 {
                Vec::new()
            } else {
                let outer: Vec<Vec<f64>> =
                    (0..m).map(|i| (0..m).map(|j| rest.iter().map(|c| c[i] * c[j]).sum()).collect()).collect();
                let (values, vectors) = eigen(outer);
                let top = values.iter().fold(0.0f64, |acc, v| acc.max(v.abs()));
                (0..m)
                    .filter(|&j| values[j] > 1e-12 * top && values[j] > 1e-300)
                    .map(|j| (0..m).map(|i| vectors[i][j]).collect())
                    .collect()
            };
            let outside = |y: &[f64]| -> Vec<f64> {
                let mut out = y.to_vec();
                for u in &basis {
                    let dot: f64 = u.iter().zip(y).map(|(a, b)| a * b).sum();
                    for (o, ui) in out.iter_mut().zip(u) {
                        *o -= dot * ui;
                    }
                }
                out
            };
            let cols: Vec<Vec<f64>> = group.iter().map(|&k| outside(&column(k))).collect();
            let z = least_norm(&cols, &outside(&b), floor);
            for (&k, zk) in group.iter().zip(&z) {
                d[k] = zk / scale[k];
                for (bi, row) in b.iter_mut().zip(jac) {
                    *bi -= row[k] * d[k];
                }
            }
        }
        d
    }

    pub fn solve(entities: &mut [SketchEntity], constraints: &[Value]) -> R<()> {
        if constraints.is_empty() {
            return Ok(());
        }
        let fixed = constraints.iter().any(|c| field(c, "constraintType").as_str() == Some("FIX"));
        let (slots, x0, scale, groups) = slots(entities, fixed);
        let norm = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut x = x0.clone();
        let mut r = all(entities, &slots, constraints, &x, &x0);
        if r.iter().all(|v| v.abs() < 1e-10) {
            return Ok(());
        }
        let n = x.len();
        // The solution moving the groups Onshape holds hardest least, in turn: at each step, the start projected onto
        // the constraints linearised where the solution has got to, group by group, backed off while it doesn't
        // bring the residuals down; then plain Gauss–Newton steps to finish it off, moving it by no more than the
        // residuals.
        let all_groups = vec![(0..n).collect::<Vec<usize>>()];
        let mut polish = false;
        for _ in 0..100 {
            if norm(&r) < 1e-13 {
                break;
            }
            // The Jacobian by central differences.
            let h = 1e-8;
            let mut jac = vec![vec![0.0; n]; r.len()];
            for k in 0..n {
                let (mut up, mut down) = (x.clone(), x.clone());
                up[k] += h;
                down[k] -= h;
                let (ru, rd) = (all(entities, &slots, constraints, &up, &x0), all(entities, &slots, constraints, &down, &x0));
                for (row, (a, b)) in jac.iter_mut().zip(ru.iter().zip(rd.iter())) {
                    row[k] = (a - b) / (2.0 * h);
                }
            }
            let anchor: Vec<f64> = if polish { x.clone() } else { x0.clone() };
            let rhs: Vec<f64> = (0..r.len())
                .map(|i| (0..n).map(|k| jac[i][k] * (x[k] - anchor[k])).sum::<f64>() - r[i])
                .collect();
            let step = lexicographic(&jac, &rhs, if polish { &all_groups } else { &groups }, &scale);
            let target: Vec<f64> = (0..n).map(|k| anchor[k] + step[k]).collect();
            let mut t = 1.0;
            let mut improved = false;
            for _ in 0..30 {
                let trial: Vec<f64> = (0..n).map(|k| x[k] + t * (target[k] - x[k])).collect();
                let rt = all(entities, &slots, constraints, &trial, &x0);
                if norm(&rt) < norm(&r) {
                    x = trial;
                    r = rt;
                    improved = true;
                    break;
                }
                t /= 2.0;
            }
            if !improved {
                if polish {
                    break;
                }
                polish = true;
            }
        }
        // Write the solved geometry back.
        for (e, slot) in entities.iter_mut().zip(slots.iter()) {
            match (slot, &mut e.geometry) {
                (Some(Slot::Point(i)), Geometry::Point(p)) => *p = (x[*i], x[*i + 1]),
                (Some(slot @ Slot::Line(_)), Geometry::Line(a, b)) => {
                    if let Some((p, q)) = line(&x, *slot) {
                        (*a, *b) = (p, q);
                    }
                }
                (Some(Slot::Arc(i)), Geometry::Arc { start, mid, end }) => {
                    let (cx, cy, rr, a0, a1) = (x[*i], x[*i + 1], x[*i + 2], x[*i + 3], x[*i + 4]);
                    let at = |a: f64| (cx + rr * a.cos(), cy + rr * a.sin());
                    *start = at(a0);
                    *mid = at((a0 + a1) / 2.0);
                    *end = at(a1);
                }
                (Some(Slot::Circle(i)), Geometry::Circle { center, radius }) => {
                    *center = (x[*i], x[*i + 1]);
                    *radius = x[*i + 2];
                }
                _ => {}
            }
        }
        Ok(())
    }
}
