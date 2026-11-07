//! Converting between FeatureScript values and geometry: lengths, vectors, planes, ids and std's tagged types.
//!
//! Builtins take lengths either as `ValueWithUnits` maps or as plain numbers in metres (std strips units before
//! some calls), and return std's own types, so `@evPlane` returns a map tagged `Plane` whose origin is a
//! `Vector` of `ValueWithUnits`.

use fs_interp::Value;
use fs_interp::eval::{R, error};
use fs_interp::types::{self, TypeId};
use fs_interp::value::ValueMap;
use occt::{Frame, Vec3};
use std::rc::Rc;

/// std's type tags, looked up by name once std has loaded.
pub struct Tags {
    pub value_with_units: TypeId,
    pub unit_spec: TypeId,
    pub vector: TypeId,
    pub plane: TypeId,
    pub line: TypeId,
    pub query: TypeId,
    pub id: TypeId,
    pub box3d: TypeId,
    pub query_type: TypeId,
    pub entity_type: TypeId,
    pub body_type: TypeId,
}

impl Tags {
    pub fn find() -> R<Tags> {
        let find = |name: &str| types::find(name).ok_or_else(|| error(format!("std type {name} isn't loaded")));
        Ok(Tags {
            value_with_units: find("ValueWithUnits")?,
            unit_spec: find("UnitSpec")?,
            vector: find("Vector")?,
            plane: find("Plane")?,
            line: find("Line")?,
            query: find("Query")?,
            id: find("Id")?,
            box3d: find("Box3d")?,
            query_type: find("QueryType")?,
            entity_type: find("EntityType")?,
            body_type: find("BodyType")?,
        })
    }

    pub fn length(&self, metres: f64) -> Value {
        self.with_units(metres, "meter", 1.0)
    }

    pub fn with_units(&self, value: f64, unit: &str, power: f64) -> Value {
        let unit = map(vec![(unit, Value::number(power))]).with_tag(Some(self.unit_spec));
        map(vec![("value", Value::number(value)), ("unit", unit)]).with_tag(Some(self.value_with_units))
    }

    pub fn length_vector(&self, v: Vec3) -> Value {
        Value::array(vec![self.length(v.x), self.length(v.y), self.length(v.z)]).with_tag(Some(self.vector))
    }

    pub fn unit_vector(&self, v: Vec3) -> Value {
        Value::array(vec![Value::number(v.x), Value::number(v.y), Value::number(v.z)]).with_tag(Some(self.vector))
    }

    pub fn plane(&self, frame: Frame) -> Value {
        map(vec![
            ("origin", self.length_vector(frame.origin)),
            ("normal", self.unit_vector(frame.normal)),
            ("x", self.unit_vector(frame.x)),
        ])
        .with_tag(Some(self.plane))
    }

    pub fn line(&self, origin: Vec3, direction: Vec3) -> Value {
        map(vec![("origin", self.length_vector(origin)), ("direction", self.unit_vector(direction))])
            .with_tag(Some(self.line))
    }

    pub fn enum_value(&self, tag: TypeId, name: &str) -> Value {
        Value::string(name).with_tag(Some(tag))
    }

    pub fn transient_query(&self, transient_id: &str) -> Value {
        map(vec![
            ("queryType", self.enum_value(self.query_type, "TRANSIENT")),
            ("transientId", Value::string(transient_id)),
        ])
        .with_tag(Some(self.query))
    }
}

pub fn map(entries: Vec<(&str, Value)>) -> Value {
    Value::map(entries.into_iter().map(|(k, v)| (Value::string(k), v)).collect::<ValueMap>())
}

/// A field of a definition map, or `undefined`.
pub fn field<'a>(definition: &'a Value, name: &str) -> &'a Value {
    definition.get(name).unwrap_or(&Value::UNDEFINED)
}

pub fn opt_bool(definition: &Value, name: &str) -> R<Option<bool>> {
    match field(definition, name) {
        v if v.is_undefined() => Ok(None),
        v => v.as_bool().map(Some).ok_or_else(|| error(format!("{name} must be a boolean"))),
    }
}

/// A number with the given unit, in SI: `ValueWithUnits` maps are checked; plain numbers are taken as SI.
fn with_unit(v: &Value, unit: &str, what: &str) -> R<f64> {
    if let Some(n) = v.as_number() {
        return Ok(n);
    }
    let value = v.get("value").and_then(Value::as_number);
    let unit_map = v.get("unit").and_then(Value::as_map);
    match (value, unit_map) {
        (Some(value), Some(units)) => {
            let ok = units.len() == 1 && units.get(&Value::string(unit)).and_then(Value::as_number) == Some(1.0);
            if ok { Ok(value) } else { Err(error(format!("{what} has the wrong units"))) }
        }
        _ => Err(error(format!("{what} must be a {unit} value, not {}", v.type_description()))),
    }
}

/// A length in metres.
pub fn length(v: &Value, what: &str) -> R<f64> {
    with_unit(v, "meter", what)
}

/// An angle in radians.
pub fn angle(v: &Value, what: &str) -> R<f64> {
    with_unit(v, "radian", what)
}

fn components(v: &Value, n: usize, what: &str) -> R<Vec<f64>> {
    let items = v.as_array().ok_or_else(|| error(format!("{what} must be a vector")))?;
    if items.len() != n {
        return Err(error(format!("{what} must have {n} components")));
    }
    items.iter().map(|c| c.as_number().map_or_else(|| length(c, what), Ok)).collect()
}

/// A 3D vector of lengths (in metres) or of plain numbers.
pub fn vec3(v: &Value, what: &str) -> R<Vec3> {
    let c = components(v, 3, what)?;
    Ok(Vec3::new(c[0], c[1], c[2]))
}

/// A 2D vector of lengths (in metres) or of plain numbers.
pub fn vec2(v: &Value, what: &str) -> R<(f64, f64)> {
    let c = components(v, 2, what)?;
    Ok((c[0], c[1]))
}

/// A `Plane` map.
pub fn plane(v: &Value, what: &str) -> R<Frame> {
    let origin = vec3(field(v, "origin"), what)?;
    let normal = normalize(vec3(field(v, "normal"), what)?).ok_or_else(|| error(format!("{what}: zero normal")))?;
    let x = normalize(vec3(field(v, "x"), what)?).ok_or_else(|| error(format!("{what}: zero x direction")))?;
    Ok(Frame { origin, normal, x })
}

/// An `Id` (array of strings) as its parts.
pub fn feature_id(v: &Value, what: &str) -> R<Rc<[Rc<str>]>> {
    let items = v.as_array().ok_or_else(|| error(format!("{what} must be an Id")))?;
    items.iter().map(|part| part.as_str().map(Rc::from).ok_or_else(|| error(format!("{what} must be an Id")))).collect()
}

pub fn id_value(tags: &Tags, id: &[Rc<str>]) -> Value {
    Value::array(id.iter().map(|part| Value::string(part.clone())).collect()).with_tag(Some(tags.id))
}

// ----- Vector maths -----

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

pub fn scale(a: Vec3, s: f64) -> Vec3 {
    Vec3::new(a.x * s, a.y * s, a.z * s)
}

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

pub fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

pub fn normalize(a: Vec3) -> Option<Vec3> {
    let n = norm(a);
    (n > 1e-300).then(|| scale(a, 1.0 / n))
}

/// The point at plane coordinates (u, v) of a frame.
pub fn on_plane(frame: &Frame, u: f64, v: f64) -> Vec3 {
    let y = cross(frame.normal, frame.x);
    add(frame.origin, add(scale(frame.x, u), scale(y, v)))
}
