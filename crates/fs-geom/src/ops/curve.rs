//! Curve operations: `@opHelix`, `@opFitSpline` and `@opCreateBSplineCurve`.

use crate::model::{BodyType, FeatureId, Info, Model, NewBody, Origins};
use crate::regen_error;
use crate::splines::{self, Ends};
use crate::values::{self, field};
use fs_interp::Value;
use fs_interp::eval::{R, error};

fn add_wire(model: &mut Model, id: FeatureId, edge: occt::Shape) {
    let info = Info::new(id);
    model.add_body(NewBody::new(edge, BodyType::Wire, info.clone()), &Origins::default(), &info, &|_| None);
}

pub fn helix(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let direction = values::vec3(field(definition, "direction"), "opHelix direction")?;
    let origin = values::vec3(field(definition, "axisStart"), "opHelix axisStart")?;
    let start = values::vec3(field(definition, "startPoint"), "opHelix startPoint")?;
    let interval = values::numbers(field(definition, "interval"), "opHelix interval")?;
    let [t0, t1] = interval[..] else { return Err(error("@opHelix: interval must have two numbers")) };
    let clockwise = values::opt_bool(definition, "clockwise")?.unwrap_or(false);
    let pitch = values::length(field(definition, "helicalPitch"), "opHelix helicalPitch")?;
    let spiral = match field(definition, "spiralPitch") {
        v if v.is_undefined() => 0.0,
        v => values::length(v, "opHelix spiralPitch")?,
    };
    // `clockwise: false` turns left-handed about `direction` (batch 004, c043; whole turns can't tell): the
    // opposite of occt's flag.
    let edge = occt::helix_edge(origin, direction, start, t0, t1, !clockwise, pitch, spiral)
        .map_err(|_| regen_error("HELIX_FAILED"))?;
    add_wire(model, id, edge);
    Ok(Value::UNDEFINED)
}

pub fn fit_spline(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    for key in ["start2ndDerivative", "end2ndDerivative", "derivatives"] {
        if !field(definition, key).is_undefined() {
            return Err(error(format!("@opFitSpline: {key} isn't supported yet")));
        }
    }
    if values::opt_bool(definition, "hasTargetLength")?.unwrap_or(false) {
        return Err(error("@opFitSpline: target lengths aren't supported yet"));
    }
    let items = field(definition, "points").as_array().ok_or_else(|| error("@opFitSpline: points must be an array"))?;
    let mut points: Vec<occt::Vec3> = items.iter().map(|p| values::vec3(p, "opFitSpline point")).collect::<R<_>>()?;
    let closed = points.len() > 2 && values::norm(values::sub(points[0], *points.last().expect("points"))) < 1e-12;
    if closed {
        points.pop();
    }
    let derivative = |key: &str| -> R<Option<occt::Vec3>> {
        match field(definition, key) {
            v if v.is_undefined() => Ok(None),
            v => values::vec3(v, key).map(Some),
        }
    };
    let ends = match (closed, derivative("startDerivative")?, derivative("endDerivative")?) {
        (true, _, _) => Ends::Periodic,
        (false, Some(a), Some(b)) => Ends::Clamped(a, b),
        (false, None, None) => Ends::Natural,
        _ => return Err(error("@opFitSpline: one end derivative without the other isn't supported yet")),
    };
    let parameters = match field(definition, "parameters") {
        v if v.is_undefined() => None,
        v => Some(values::numbers(v, "opFitSpline parameters")?),
    };
    let edge = splines::spline(&points, ends, parameters).map_err(|_| regen_error("FIT_SPLINE_FAILED"))?;
    add_wire(model, id, edge);
    Ok(Value::UNDEFINED)
}

/// `@opCreateBSplineCurve`: a wire body on a B-spline given as std's `BSplineCurve`.
pub fn bspline_curve(model: &mut Model, id: FeatureId, definition: &Value) -> R<Value> {
    let curve = field(definition, "bSplineCurve");
    let degree = field(curve, "degree").as_number().ok_or_else(|| error("@opCreateBSplineCurve: no degree"))? as i32;
    if field(curve, "dimension").as_number() != Some(3.0) {
        return Err(error("@opCreateBSplineCurve: only 3D curves are supported"));
    }
    let poles: Vec<occt::Vec3> = field(curve, "controlPoints")
        .as_array()
        .map(|a| a.iter().map(|p| values::vec3(p, "opCreateBSplineCurve control point")).collect::<R<_>>())
        .transpose()?
        .unwrap_or_default();
    let weights = if field(curve, "isRational").as_bool() == Some(true) {
        values::numbers(field(curve, "weights"), "opCreateBSplineCurve weights")?
    } else {
        Vec::new()
    };
    let knots = values::numbers(field(curve, "knots"), "opCreateBSplineCurve knots")?;
    let periodic = field(curve, "isPeriodic").as_bool() == Some(true);
    let edge = occt::bspline_edge(&poles, &weights, &knots, degree, periodic).map_err(crate::occt_error)?;
    add_wire(model, id, edge);
    Ok(Value::UNDEFINED)
}
