//! A thin, safe layer over the system's OpenCascade (OCCT 7.9).
//!
//! The C++ side (`cpp/occt.cc`) wraps each modelling step in one function, so this crate only passes shapes,
//! lists of shapes and plain structs across. Shapes are values: cloning one is cheap and shares the topology, as
//! copying a `TopoDS_Shape` does.
//!
//! OCCT failures arrive as [`Error`]s holding OCCT's exception name and message.

use cxx::UniquePtr;
use std::fmt;

pub use ffi::{Axis, Box3, Conic, Distance, Frame, Transform, Vec3};

#[cxx::bridge(namespace = "occt")]
mod ffi {
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Vec3 {
        x: f64,
        y: f64,
        z: f64,
    }

    /// A point and a unit direction: an axis, or a point on a curve and the tangent there.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Axis {
        origin: Vec3,
        direction: Vec3,
    }

    /// The minimum distance between two shapes and the closest point on each.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Distance {
        distance: f64,
        a: Vec3,
        b: Vec3,
    }

    /// The curve an edge lies on: `kind` 0 a line (origin at the edge's start, `frame.normal` its direction),
    /// 1 a circle (`radius1`), 2 an ellipse (`radius1` major, `radius2` minor, `frame.x` along the major axis),
    /// 3 anything else.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Conic {
        kind: i32,
        frame: Frame,
        radius1: f64,
        radius2: f64,
    }

    /// A coordinate system: `normal` is z, `x` is x and y is `normal × x`.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Frame {
        origin: Vec3,
        normal: Vec3,
        x: Vec3,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Box3 {
        min: Vec3,
        max: Vec3,
        empty: bool,
    }

    /// The top three rows of a 4×4 rigid transform, row by row.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Transform {
        m: [f64; 12],
    }

    unsafe extern "C++" {
        include!("occt/cpp/occt.h");

        type Shape;
        type ShapeList;
        type Operation;

        fn shape_clone(s: &Shape) -> UniquePtr<Shape>;
        fn shape_is_null(s: &Shape) -> bool;
        fn shape_kind(s: &Shape) -> i32;
        fn shape_orientation(s: &Shape) -> i32;
        fn shape_is_same(a: &Shape, b: &Shape) -> bool;
        fn shape_hash(s: &Shape) -> u64;

        fn list_new() -> UniquePtr<ShapeList>;
        fn list_len(l: &ShapeList) -> usize;
        fn list_get(l: &ShapeList, i: usize) -> UniquePtr<Shape>;
        fn list_push(l: Pin<&mut ShapeList>, s: &Shape);

        fn sub_shapes(s: &Shape, kind: i32) -> Result<UniquePtr<ShapeList>>;
        fn ancestors(root: &Shape, s: &Shape, kind: i32) -> Result<UniquePtr<ShapeList>>;
        fn make_compound(l: &ShapeList) -> UniquePtr<Shape>;

        fn make_vertex(p: Vec3) -> Result<UniquePtr<Shape>>;
        fn make_line_edge(a: Vec3, b: Vec3) -> Result<UniquePtr<Shape>>;
        fn make_circle_edge(f: Frame, radius: f64) -> Result<UniquePtr<Shape>>;
        fn make_arc_edge(f: Frame, radius: f64, start: f64, end: f64) -> Result<UniquePtr<Shape>>;
        fn make_arc_through(a: Vec3, mid: Vec3, b: Vec3) -> Result<UniquePtr<Shape>>;
        fn make_plane_face(f: Frame, umin: f64, umax: f64, vmin: f64, vmax: f64) -> Result<UniquePtr<Shape>>;

        fn sketch_regions(edges: &ShapeList, f: Frame) -> Result<UniquePtr<ShapeList>>;
        fn merge_faces(faces: &ShapeList) -> Result<UniquePtr<Operation>>;
        fn prism(profile: &Shape, v: Vec3) -> Result<UniquePtr<Operation>>;
        fn boolean_op(
            kind: i32,
            objects: &ShapeList,
            tools: &ShapeList,
            unify: bool,
            fuzzy: f64,
        ) -> Result<UniquePtr<Operation>>;
        fn transform_op(s: &Shape, t: &Transform) -> Result<UniquePtr<Operation>>;
        fn revolve(profile: &Shape, axis: Axis, angle: f64) -> Result<UniquePtr<Operation>>;
        fn make_sphere(center: Vec3, radius: f64) -> Result<UniquePtr<Shape>>;
        fn split_op(objects: &ShapeList, tools: &ShapeList) -> Result<UniquePtr<Operation>>;

        fn operation_shape(op: &Operation) -> UniquePtr<Shape>;
        fn operation_modified(op: &Operation, s: &Shape) -> Result<UniquePtr<ShapeList>>;
        fn operation_generated(op: &Operation, s: &Shape) -> Result<UniquePtr<ShapeList>>;
        fn operation_is_deleted(op: &Operation, s: &Shape) -> Result<bool>;
        fn operation_first(op: &Operation) -> UniquePtr<ShapeList>;
        fn operation_last(op: &Operation) -> UniquePtr<ShapeList>;

        fn bounding_box(s: &Shape, precise: bool) -> Result<Box3>;
        fn bounding_box_in(s: &Shape, frame: Frame, precise: bool) -> Result<Box3>;
        fn shape_volume(s: &Shape) -> Result<f64>;
        fn shape_area(s: &Shape) -> Result<f64>;
        fn shape_length(s: &Shape) -> Result<f64>;
        fn shape_centroid(s: &Shape) -> Result<Vec3>;
        fn vertex_point(s: &Shape) -> Result<Vec3>;
        fn surface_type(face: &Shape) -> Result<i32>;
        fn face_plane(face: &Shape) -> Result<Frame>;
        fn curve_type(edge: &Shape) -> Result<i32>;
        fn edge_is_seam(root: &Shape, edge: &Shape) -> Result<bool>;
        fn shape_valid(s: &Shape) -> Result<bool>;
        fn visible_sub_shapes(s: &Shape, kind: i32) -> Result<UniquePtr<ShapeList>>;
        fn shape_axis(s: &Shape) -> Result<Axis>;
        fn edge_tangent(s: &Shape, t: f64, arc_length: bool) -> Result<Axis>;
        fn edge_fraction(s: &Shape, p: Vec3, arc_length: bool) -> Result<f64>;
        fn edge_fraction_extended(s: &Shape, p: Vec3) -> Result<f64>;
        fn shape_distance(a: &Shape, b: &Shape) -> Result<Distance>;
        fn curve_definition(s: &Shape) -> Result<Conic>;
        fn face_tangent_plane(s: &Shape, fu: f64, fv: f64) -> Result<Frame>;
        fn face_fraction(s: &Shape, p: Vec3) -> Result<Vec3>;
        fn face_uv_bounds(s: &Shape) -> Result<Box3>;
        fn extended_shape(s: &Shape, reach: f64) -> Result<UniquePtr<Shape>>;

        fn write_step(shapes: &ShapeList, names: &Vec<String>, path: &str) -> Result<()>;
        fn read_step(path: &str, names: &mut Vec<String>) -> Result<UniquePtr<ShapeList>>;
    }
}

/// An OCCT failure: the exception's type name and message.
#[derive(Clone, Debug, PartialEq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<cxx::Exception> for Error {
    fn from(e: cxx::Exception) -> Error {
        Error(e.what().to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// `TopAbs_ShapeEnum`, from the biggest kind of shape to the smallest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShapeKind {
    Compound = 0,
    CompSolid = 1,
    Solid = 2,
    Shell = 3,
    Face = 4,
    Wire = 5,
    Edge = 6,
    Vertex = 7,
}

impl ShapeKind {
    fn from_i32(kind: i32) -> Option<ShapeKind> {
        use ShapeKind::*;
        Some(match kind {
            0 => Compound,
            1 => CompSolid,
            2 => Solid,
            3 => Shell,
            4 => Face,
            5 => Wire,
            6 => Edge,
            7 => Vertex,
            _ => return None,
        })
    }
}

/// `GeomAbs_SurfaceType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceType {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    Bezier,
    BSpline,
    Revolution,
    Extrusion,
    Offset,
    Other,
}

/// `GeomAbs_CurveType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveType {
    Line,
    Circle,
    Ellipse,
    Hyperbola,
    Parabola,
    Bezier,
    BSpline,
    Offset,
    Other,
}

pub struct Shape(UniquePtr<ffi::Shape>);

impl Clone for Shape {
    fn clone(&self) -> Shape {
        Shape(ffi::shape_clone(&self.0))
    }
}

impl fmt::Debug for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Shape({:?} #{:x})", self.kind(), self.hash())
    }
}

impl Shape {
    fn new(ptr: UniquePtr<ffi::Shape>) -> Shape {
        Shape(ptr)
    }

    pub fn is_null(&self) -> bool {
        ffi::shape_is_null(&self.0)
    }

    /// The shape's kind, or `None` for a null shape.
    pub fn kind(&self) -> Option<ShapeKind> {
        ShapeKind::from_i32(ffi::shape_kind(&self.0))
    }

    /// Whether the shape is reversed relative to its underlying topology.
    pub fn is_reversed(&self) -> bool {
        ffi::shape_orientation(&self.0) == 1
    }

    /// The same topology in the same place, whatever the orientation.
    pub fn is_same(&self, other: &Shape) -> bool {
        ffi::shape_is_same(&self.0, &other.0)
    }

    /// A hash that agrees with [`Shape::is_same`].
    pub fn hash(&self) -> u64 {
        ffi::shape_hash(&self.0)
    }

    /// The distinct sub-shapes of one kind, in OCCT's exploration order.
    pub fn sub_shapes(&self, kind: ShapeKind) -> Vec<Shape> {
        from_list(&ffi::sub_shapes(&self.0, kind as i32).expect("valid shape kind"))
    }

    /// The edges or vertices Parasolid, and so Onshape, would give the shape: no seam or degenerate edges, and
    /// vertices only where an open edge ends or at a pole that isn't a sphere's (a cone's apex). Other kinds are
    /// the same as [`Shape::sub_shapes`].
    pub fn visible_sub_shapes(&self, kind: ShapeKind) -> Vec<Shape> {
        match kind {
            ShapeKind::Edge | ShapeKind::Vertex => {
                from_list(&ffi::visible_sub_shapes(&self.0, kind as i32).expect("edges or vertices"))
            }
            _ => self.sub_shapes(kind),
        }
    }

    /// The shapes of `kind` inside `self` that contain `part`, such as the faces next to an edge.
    pub fn ancestors(&self, part: &Shape, kind: ShapeKind) -> Vec<Shape> {
        from_list(&ffi::ancestors(&self.0, &part.0, kind as i32).expect("valid shape kind"))
    }

    pub fn bounding_box(&self, precise: bool) -> Result<Box3> {
        Ok(ffi::bounding_box(&self.0, precise)?)
    }

    /// The box in the coordinates of `frame`.
    pub fn bounding_box_in(&self, frame: Frame, precise: bool) -> Result<Box3> {
        Ok(ffi::bounding_box_in(&self.0, frame, precise)?)
    }

    pub fn volume(&self) -> Result<f64> {
        Ok(ffi::shape_volume(&self.0)?)
    }

    pub fn area(&self) -> Result<f64> {
        Ok(ffi::shape_area(&self.0)?)
    }

    pub fn length(&self) -> Result<f64> {
        Ok(ffi::shape_length(&self.0)?)
    }

    /// The centre of mass: by volume for solids, by area for faces and shells, by length for edges and wires.
    pub fn centroid(&self) -> Result<Vec3> {
        Ok(ffi::shape_centroid(&self.0)?)
    }

    pub fn vertex_point(&self) -> Result<Vec3> {
        Ok(ffi::vertex_point(&self.0)?)
    }

    pub fn surface_type(&self) -> Result<SurfaceType> {
        use SurfaceType::*;
        const TYPES: [SurfaceType; 11] =
            [Plane, Cylinder, Cone, Sphere, Torus, Bezier, BSpline, Revolution, Extrusion, Offset, Other];
        Ok(TYPES.get(ffi::surface_type(&self.0)? as usize).copied().unwrap_or(Other))
    }

    /// A planar face's plane, with the normal pointing out of the material.
    pub fn face_plane(&self) -> Result<Frame> {
        Ok(ffi::face_plane(&self.0)?)
    }

    pub fn curve_type(&self) -> Result<CurveType> {
        use CurveType::*;
        const TYPES: [CurveType; 9] = [Line, Circle, Ellipse, Hyperbola, Parabola, Bezier, BSpline, Offset, Other];
        Ok(TYPES.get(ffi::curve_type(&self.0)? as usize).copied().unwrap_or(Other))
    }

    /// Whether `edge` is the seam of one of this shape's closed faces, where a periodic face such as a cylinder's
    /// meets itself.
    pub fn is_seam(&self, edge: &Shape) -> bool {
        ffi::edge_is_seam(&self.0, &edge.0).unwrap_or(false)
    }

    /// The axis of a line, circle or arc edge, or of a cylinder, cone, sphere, torus or revolved face. A line's
    /// axis starts at its first point.
    pub fn axis(&self) -> Result<Axis> {
        Ok(ffi::shape_axis(&self.0)?)
    }

    /// The point at fraction `t` (0 to 1) along an edge, by arc length or curve parameter, and the unit tangent
    /// there in the edge's direction.
    pub fn edge_tangent(&self, t: f64, arc_length: bool) -> Result<Axis> {
        Ok(ffi::edge_tangent(&self.0, t, arc_length)?)
    }

    /// The fraction (0 to 1) along an edge of the point on it closest to `p`.
    pub fn edge_fraction(&self, p: Vec3, arc_length: bool) -> Result<f64> {
        Ok(ffi::edge_fraction(&self.0, p, arc_length)?)
    }

    /// Like [`Shape::edge_fraction`], but along the edge's whole underlying curve, so the fraction can be below 0
    /// or above 1.
    pub fn edge_fraction_extended(&self, p: Vec3) -> Result<f64> {
        Ok(ffi::edge_fraction_extended(&self.0, p)?)
    }

    /// The minimum distance to another shape, and the closest points.
    pub fn distance(&self, other: &Shape) -> Result<Distance> {
        Ok(ffi::shape_distance(&self.0, &other.0)?)
    }

    /// The line, circle or ellipse an edge lies on.
    pub fn curve_definition(&self) -> Result<Conic> {
        Ok(ffi::curve_definition(&self.0)?)
    }

    /// The plane tangent to a face at fractions of its parameter-space bounding box.
    pub fn face_tangent_plane(&self, fu: f64, fv: f64) -> Result<Frame> {
        Ok(ffi::face_tangent_plane(&self.0, fu, fv)?)
    }

    /// Where on a face (as fractions of its parameter-space bounding box) the point closest to `p` is.
    pub fn face_fraction(&self, p: Vec3) -> Result<(f64, f64)> {
        let f = ffi::face_fraction(&self.0, p)?;
        Ok((f.x, f.y))
    }

    /// A face's parameter-space bounding box, as ((u0, v0), (u1, v1)).
    pub fn face_uv_bounds(&self) -> Result<((f64, f64), (f64, f64))> {
        let b = ffi::face_uv_bounds(&self.0)?;
        Ok(((b.min.x, b.min.y), (b.max.x, b.max.y)))
    }

    /// An edge or face extended to its underlying line, plane or closed curve.
    pub fn extended(&self, reach: f64) -> Result<Shape> {
        Ok(Shape::new(ffi::extended_shape(&self.0, reach)?))
    }

    /// Whether OCCT's checker finds the shape valid.
    pub fn is_valid(&self) -> bool {
        ffi::shape_valid(&self.0).unwrap_or(false)
    }
}

fn to_list(shapes: &[Shape]) -> UniquePtr<ffi::ShapeList> {
    let mut list = ffi::list_new();
    for s in shapes {
        ffi::list_push(list.pin_mut(), &s.0);
    }
    list
}

fn from_list(list: &ffi::ShapeList) -> Vec<Shape> {
    (0..ffi::list_len(list)).map(|i| Shape::new(ffi::list_get(list, i))).collect()
}

pub fn compound(shapes: &[Shape]) -> Shape {
    Shape::new(ffi::make_compound(&to_list(shapes)))
}

pub fn vertex(p: Vec3) -> Result<Shape> {
    Ok(Shape::new(ffi::make_vertex(p)?))
}

pub fn line_edge(a: Vec3, b: Vec3) -> Result<Shape> {
    Ok(Shape::new(ffi::make_line_edge(a, b)?))
}

pub fn circle_edge(frame: Frame, radius: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::make_circle_edge(frame, radius)?))
}

/// An arc of the circle centred on `frame`'s origin, from angle `start` to `end` (radians, from `frame.x`).
pub fn arc_edge(frame: Frame, radius: f64, start: f64, end: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::make_arc_edge(frame, radius, start, end)?))
}

pub fn arc_through(a: Vec3, mid: Vec3, b: Vec3) -> Result<Shape> {
    Ok(Shape::new(ffi::make_arc_through(a, mid, b)?))
}

/// A rectangular face on `frame`'s plane, bounded in the frame's x and y coordinates.
pub fn plane_face(frame: Frame, umin: f64, umax: f64, vmin: f64, vmax: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::make_plane_face(frame, umin, umax, vmin, vmax)?))
}

/// The regions that edges lying on `frame`'s plane enclose, as faces with the frame's normal.
pub fn sketch_regions(edges: &[Shape], frame: Frame) -> Result<Vec<Shape>> {
    let regions = ffi::sketch_regions(&to_list(edges), frame)?;
    Ok(from_list(&regions))
}

/// A modelling step's result and the history of its inputs' vertices, edges, faces and solids.
pub struct Operation(UniquePtr<ffi::Operation>);

impl Operation {
    pub fn shape(&self) -> Shape {
        Shape::new(ffi::operation_shape(&self.0))
    }

    /// What an input shape became, if the step changed it.
    pub fn modified(&self, input: &Shape) -> Vec<Shape> {
        ffi::operation_modified(&self.0, &input.0).map(|l| from_list(&l)).unwrap_or_default()
    }

    /// New shapes made from an input shape, such as the side face an extruded edge sweeps.
    pub fn generated(&self, input: &Shape) -> Vec<Shape> {
        ffi::operation_generated(&self.0, &input.0).map(|l| from_list(&l)).unwrap_or_default()
    }

    pub fn is_deleted(&self, input: &Shape) -> bool {
        ffi::operation_is_deleted(&self.0, &input.0).unwrap_or(false)
    }

    /// For an extrude, the cap at the start of each profile face.
    pub fn first(&self) -> Vec<Shape> {
        from_list(&ffi::operation_first(&self.0))
    }

    /// For an extrude, the cap at the end of each profile face.
    pub fn last(&self) -> Vec<Shape> {
        from_list(&ffi::operation_last(&self.0))
    }
}

/// Merges faces that share edges and lie on the same surface.
pub fn merge_faces(faces: &[Shape]) -> Result<Operation> {
    Ok(Operation(ffi::merge_faces(&to_list(faces))?))
}

/// Sweeps a profile along `v`.
pub fn prism(profile: &Shape, v: Vec3) -> Result<Operation> {
    Ok(Operation(ffi::prism(&profile.0, v)?))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanKind {
    Union = 0,
    Subtraction = 1,
    Intersection = 2,
}

/// A boolean of `objects` with `tools`. With `unify`, faces and edges left on one surface or curve are merged.
pub fn boolean(kind: BooleanKind, objects: &[Shape], tools: &[Shape], unify: bool) -> Result<Operation> {
    boolean_fuzzy(kind, objects, tools, unify, 0.0)
}

/// A boolean that treats geometry closer than `fuzzy` as coincident, for shapes that should match but come from
/// different modellers.
pub fn boolean_fuzzy(
    kind: BooleanKind,
    objects: &[Shape],
    tools: &[Shape],
    unify: bool,
    fuzzy: f64,
) -> Result<Operation> {
    Ok(Operation(ffi::boolean_op(kind as i32, &to_list(objects), &to_list(tools), unify, fuzzy)?))
}

pub fn transform(shape: &Shape, t: &Transform) -> Result<Operation> {
    Ok(Operation(ffi::transform_op(&shape.0, t)?))
}

/// Revolves a profile `angle` radians, right-handed, about an axis. Below a full turn, [`Operation::first`] and
/// [`Operation::last`] are the faces at the start and end of the sweep.
pub fn revolve(profile: &Shape, axis: Axis, angle: f64) -> Result<Operation> {
    Ok(Operation(ffi::revolve(&profile.0, axis, angle)?))
}

pub fn sphere(center: Vec3, radius: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::make_sphere(center, radius)?))
}

/// Splits `objects` with `tools` (faces, sheets or solids); the result holds every piece.
pub fn split(objects: &[Shape], tools: &[Shape]) -> Result<Operation> {
    Ok(Operation(ffi::split_op(&to_list(objects), &to_list(tools))?))
}

/// Writes shapes to an AP242 STEP file in metres, naming each one's product.
pub fn write_step(shapes: &[Shape], names: &[String], path: &str) -> Result<()> {
    Ok(ffi::write_step(&to_list(shapes), &names.to_vec(), path)?)
}

/// Reads every manifold solid of a STEP file, in metres, with the name written on it.
pub fn read_step(path: &str) -> Result<Vec<(String, Shape)>> {
    let mut names = Vec::new();
    let list = ffi::read_step(path, &mut names)?;
    let shapes = from_list(&list);
    Ok(names.into_iter().zip(shapes).collect())
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }
}
