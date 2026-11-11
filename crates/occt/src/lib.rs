//! A thin, safe layer over the system's OpenCascade (OCCT 7.9).
//!
//! The C++ side (`cpp/occt.cc`) wraps each modelling step in one function, so this crate only passes shapes,
//! lists of shapes and plain structs across. Shapes are values: cloning one is cheap and shares the topology, as
//! copying a `TopoDS_Shape` does.
//!
//! OCCT failures arrive as [`Error`]s holding OCCT's exception name and message.

use cxx::UniquePtr;
use std::fmt;

pub use ffi::{Axis, BSplineData, BSplineSurfaceData, Box3, Conic, Distance, Frame, Transform, Vec3};

// Some operations take many plain values (wrapping takes two frames, the destination and a radius).
#[allow(clippy::too_many_arguments)]
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

    /// A B-spline curve as std's `BSplineCurve` gives it.
    #[derive(Clone, Debug, PartialEq)]
    struct BSplineData {
        degree: i32,
        periodic: bool,
        rational: bool,
        poles: Vec<Vec3>,
        /// Empty unless rational.
        weights: Vec<f64>,
        /// The full knot vector, knots repeated by their multiplicity; a periodic curve's has `poles + 2 degree + 1`.
        knots: Vec<f64>,
    }

    /// A B-spline surface as std's `BSplineSurface` gives it: `poles` row by row, `nu` rows (along u) of `nv`.
    #[derive(Clone, Debug, PartialEq)]
    struct BSplineSurfaceData {
        u_degree: i32,
        v_degree: i32,
        u_periodic: bool,
        v_periodic: bool,
        rational: bool,
        nu: i32,
        nv: i32,
        poles: Vec<Vec3>,
        /// Empty unless rational, else as `poles`.
        weights: Vec<f64>,
        /// The full knot vectors, knots repeated by their multiplicity.
        u_knots: Vec<f64>,
        v_knots: Vec<f64>,
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
        fn shape_reversed(s: &Shape) -> UniquePtr<Shape>;
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
        fn make_ellipse_edge(
            f: Frame,
            major: f64,
            minor: f64,
            whole: bool,
            start: f64,
            end: f64,
        ) -> Result<UniquePtr<Shape>>;
        fn make_interpolated_edge(
            points: &[Vec3],
            derivatives: &[Vec3],
            parameters: &[f64],
            closed: bool,
        ) -> Result<UniquePtr<Shape>>;
        fn make_bezier_edge(points: &[Vec3]) -> Result<UniquePtr<Shape>>;
        fn make_bspline_edge(
            poles: &[Vec3],
            weights: &[f64],
            knots: &[f64],
            degree: i32,
            periodic: bool,
        ) -> Result<UniquePtr<Shape>>;
        fn make_conic_edge(a: Vec3, c: Vec3, b: Vec3, w: f64) -> Result<UniquePtr<Shape>>;
        fn thicken_sheet(input: &Shape, t1: f64, t2: f64) -> Result<UniquePtr<Shape>>;
        fn orient_sheet(input: &Shape) -> Result<UniquePtr<Shape>>;
        fn nurbs_converted(input: &Shape) -> Result<UniquePtr<Shape>>;
        fn face_from_uv(face: &Shape, uv: &[f64], counts: &[u32]) -> Result<UniquePtr<Shape>>;
        fn make_bspline_surface_face(
            poles: &[Vec3],
            nu: i32,
            nv: i32,
            knots_u: &[f64],
            knots_v: &[f64],
            degree_u: i32,
            degree_v: i32,
        ) -> Result<UniquePtr<Shape>>;
        #[allow(clippy::too_many_arguments)]
        fn make_rational_bspline_surface_face(
            poles: &[Vec3],
            weights: &[f64],
            nu: i32,
            nv: i32,
            knots_u: &[f64],
            knots_v: &[f64],
            degree_u: i32,
            degree_v: i32,
        ) -> Result<UniquePtr<Shape>>;
        fn connect_edges(edges: &ShapeList) -> Result<UniquePtr<ShapeList>>;
        fn offset_wire_region(wire: &Shape, normal: Vec3, o1: f64, o2: f64) -> Result<UniquePtr<Shape>>;
        #[allow(clippy::too_many_arguments)]
        fn make_helix_edge(
            origin: Vec3,
            direction: Vec3,
            start: Vec3,
            t0: f64,
            t1: f64,
            clockwise: bool,
            pitch: f64,
            spiral: f64,
        ) -> Result<UniquePtr<Shape>>;
        fn sweep_op(profile: &Shape, path: &Shape) -> Result<UniquePtr<Operation>>;
        fn sweep_controlled_op(
            profile: &Shape,
            path: &Shape,
            mode: i32,
            direction: Vec3,
        ) -> Result<UniquePtr<Operation>>;
        fn loft_op(wires: &ShapeList, solid: bool) -> Result<UniquePtr<Operation>>;
        fn loft_op_matching(wires: &ShapeList, solid: bool, check: bool) -> Result<UniquePtr<Operation>>;
        fn loft_pieces_op(wires: &ShapeList, counts: &[u32], solid: bool) -> Result<UniquePtr<Operation>>;
        fn loft_clamped_op(
            wires: &ShapeList,
            solid: bool,
            start_dir: Vec3,
            start_mag: f64,
            start_tangent: bool,
            start_faces: &ShapeList,
            start_curvature: bool,
            end_dir: Vec3,
            end_mag: f64,
            end_tangent: bool,
            end_faces: &ShapeList,
            end_curvature: bool,
        ) -> Result<UniquePtr<Operation>>;
        fn outer_wire(face: &Shape) -> Result<UniquePtr<Shape>>;
        fn section_edges(a: &ShapeList, b: &ShapeList) -> Result<UniquePtr<ShapeList>>;
        fn fill_edges(edges: &ShapeList) -> Result<UniquePtr<Shape>>;
        fn enclose_shapes(shapes: &ShapeList) -> Result<UniquePtr<ShapeList>>;
        fn extract_faces(faces: &ShapeList, offset: f64) -> Result<UniquePtr<ShapeList>>;
        fn extend_planar_sheet(sheet: &Shape, edges: &ShapeList, distance: f64) -> Result<UniquePtr<Shape>>;
        fn extend_planar_face(sheet: &Shape, edges: &ShapeList, distances: &[f64]) -> Result<UniquePtr<Shape>>;
        fn sew_faces(faces: &ShapeList, tolerance: f64) -> Result<UniquePtr<Shape>>;
        fn cylinder_face(frame: Frame, radius: f64, half_height: f64) -> Result<UniquePtr<Shape>>;
        fn surface_patch(face: &Shape, u0: f64, u1: f64, v0: f64, v1: f64) -> Result<UniquePtr<Shape>>;
        fn face_corners_2d(face: &Shape, vertices: &ShapeList, size: f64, chamfer: bool) -> Result<UniquePtr<Shape>>;
        fn face_corner_2d(
            face: &Shape,
            vertex: &Shape,
            kind: i32,
            d1: f64,
            d2: f64,
            rho: f64,
        ) -> Result<UniquePtr<Shape>>;
        fn iso_curves(face: &Shape, along_u: bool, t: f64) -> Result<UniquePtr<ShapeList>>;
        fn face_periodic(face: &Shape, along_u: bool) -> Result<bool>;
        fn join_edges(wire: &Shape) -> Result<UniquePtr<Shape>>;
        fn ordered_wire(edges: &ShapeList) -> Result<UniquePtr<Shape>>;
        fn edge_on_face(face: &Shape, uv: &[f64]) -> Result<UniquePtr<Shape>>;
        fn closed_edge_on_face(face: &Shape, uv: &[f64]) -> Result<UniquePtr<Shape>>;
        fn outline_on_plane(tools: &ShapeList, plane: Frame) -> Result<UniquePtr<Shape>>;
        fn edge_between(edge: &Shape, f0: f64, f1: f64, reverse: bool) -> Result<UniquePtr<Shape>>;
        fn project_edges(edges: &ShapeList, faces: &ShapeList, direction: Vec3) -> Result<UniquePtr<ShapeList>>;
        fn offset_edge_on_planar_face(edge: &Shape, face: &Shape, distance: f64) -> Result<UniquePtr<Shape>>;

        fn sketch_regions(edges: &ShapeList, f: Frame) -> Result<UniquePtr<ShapeList>>;
        fn merge_faces(faces: &ShapeList) -> Result<UniquePtr<Operation>>;
        fn merge_edges(s: &Shape) -> Result<UniquePtr<Operation>>;
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
        fn fillet_op(solid: &Shape, edges: &ShapeList, radius: f64) -> Result<UniquePtr<Operation>>;
        fn chamfer_op(
            solid: &Shape,
            edges: &ShapeList,
            faces: &ShapeList,
            kind: i32,
            d1: f64,
            d2: f64,
        ) -> Result<UniquePtr<Operation>>;
        fn variable_fillet_op(
            solid: &Shape,
            edges: &ShapeList,
            p0: &[Vec3],
            r0: &[f64],
            r1: &[f64],
            smooth: bool,
        ) -> Result<UniquePtr<Operation>>;
        fn shell_op(solid: &Shape, faces: &ShapeList, thickness: f64) -> Result<UniquePtr<Operation>>;
        fn offset_shape(s: &Shape, distance: f64) -> Result<UniquePtr<Shape>>;
        fn draft_angles_op(
            solid: &Shape,
            faces: &ShapeList,
            pull: Vec3,
            angles: &[f64],
            neutrals: &[Frame],
        ) -> Result<UniquePtr<Operation>>;
        fn draft_op(
            solid: &Shape,
            faces: &ShapeList,
            pull: Vec3,
            angle: f64,
            neutrals: &[Frame],
        ) -> Result<UniquePtr<Operation>>;
        fn thicken_op(sheet: &Shape, t1: f64, t2: f64) -> Result<UniquePtr<Operation>>;
        fn offset_faces_op(solid: &Shape, faces: &ShapeList, distance: f64) -> Result<UniquePtr<Operation>>;
        fn delete_faces_op(solid: &Shape, faces: &ShapeList) -> Result<UniquePtr<Operation>>;
        fn remove_faces_open(solid: &Shape, faces: &ShapeList) -> Result<UniquePtr<Shape>>;
        fn solid_from(s: &Shape) -> Result<UniquePtr<Shape>>;
        fn replace_planar_face_op(solid: &Shape, face: &Shape, target: Frame) -> Result<UniquePtr<Operation>>;
        fn replace_face_with_surface_op(
            solid: &Shape,
            face: &Shape,
            template_face: &Shape,
            offset: f64,
        ) -> Result<UniquePtr<Operation>>;

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
        fn shape_principal_moments(s: &Shape) -> Result<Vec3>;
        fn shape_mass_properties(s: &Shape, dimension: i32) -> Result<Vec<f64>>;
        fn vertex_point(s: &Shape) -> Result<Vec3>;
        fn surface_type(face: &Shape) -> Result<i32>;
        fn face_plane(face: &Shape) -> Result<Frame>;
        fn curve_type(edge: &Shape) -> Result<i32>;
        fn edge_is_seam(root: &Shape, edge: &Shape) -> Result<bool>;
        fn shape_valid(s: &Shape) -> Result<bool>;
        fn shape_max_tolerance(s: &Shape) -> Result<f64>;
        fn visible_sub_shapes(s: &Shape, kind: i32) -> Result<UniquePtr<ShapeList>>;
        fn hidden_edge_pieces(s: &Shape) -> Result<UniquePtr<ShapeList>>;
        fn shape_axis(s: &Shape) -> Result<Axis>;
        fn edge_tangent(s: &Shape, t: f64, arc_length: bool) -> Result<Axis>;
        fn edge_fraction(s: &Shape, p: Vec3, arc_length: bool) -> Result<f64>;
        fn edge_fraction_extended(s: &Shape, p: Vec3) -> Result<f64>;
        fn shape_distance(a: &Shape, b: &Shape) -> Result<Distance>;
        fn line_crossings(s: &Shape, origin: Vec3, direction: Vec3) -> Result<Vec<f64>>;
        fn face_inner_point(s: &Shape) -> Result<Vec3>;
        fn edge_curvature(s: &Shape, t: f64, arc_length: bool) -> Result<Conic>;
        fn edge_curvature_derivative(s: &Shape, t: f64, arc_length: bool) -> Result<Vec3>;
        fn full_round_op(
            solid: &Shape,
            side1: &Shape,
            side2: &Shape,
            centre: &ShapeList,
        ) -> Result<UniquePtr<Operation>>;
        fn edge_convexity(body: &Shape, edge: &Shape, by_curvature: bool) -> Result<i32>;
        fn face_feature_solid(faces: &ShapeList) -> Result<UniquePtr<Shape>>;
        fn connected_shells(s: &Shape) -> Result<UniquePtr<ShapeList>>;
        fn unwrap_from_cylinder(
            faces: &ShapeList,
            source: &Shape,
            source_anchor: Vec3,
            source_direction: Vec3,
            destination_anchor: Vec3,
            destination_direction: Vec3,
            destination_normal: Vec3,
        ) -> Result<UniquePtr<Operation>>;
        fn wrap_onto_cylinder(
            faces: &ShapeList,
            source_anchor: Vec3,
            source_direction: Vec3,
            source_normal: Vec3,
            destination: &Shape,
            destination_anchor: Vec3,
            destination_direction: Vec3,
            arc_radius: f64,
        ) -> Result<UniquePtr<Operation>>;
        fn face_blend_op(
            side1: &ShapeList,
            reverse1: bool,
            side2: &ShapeList,
            reverse2: bool,
            radius: f64,
        ) -> Result<UniquePtr<Operation>>;
        fn split_faces_op(body: &Shape, faces: &ShapeList, tools: &ShapeList) -> Result<UniquePtr<Operation>>;
        fn merge_faces_across_op(body: &Shape, removable: &ShapeList) -> Result<UniquePtr<Operation>>;
        fn split_faces_paired_op(
            body: &Shape,
            faces: &ShapeList,
            tools: &ShapeList,
            owners: &[i32],
        ) -> Result<UniquePtr<Operation>>;
        fn split_by_bands_op(
            body: &Shape,
            face: &Shape,
            first: &[Vec3],
            second: &[Vec3],
        ) -> Result<UniquePtr<Operation>>;
        fn split_by_open_bands_op(
            body: &Shape,
            face: &Shape,
            first: &[Vec3],
            second: &[Vec3],
        ) -> Result<UniquePtr<Operation>>;
        fn split_face_by_edges_op(body: &Shape, face: &Shape, edges: &ShapeList) -> Result<UniquePtr<Operation>>;
        fn point_in_solid(solid: &Shape, p: Vec3) -> Result<bool>;
        fn curve_definition(s: &Shape) -> Result<Conic>;
        fn edge_bspline(s: &Shape, tolerance: f64, force_cubic: bool, force_non_rational: bool) -> Result<BSplineData>;
        fn face_bspline(s: &Shape) -> Result<BSplineSurfaceData>;
        fn surface_definition(s: &Shape) -> Result<Conic>;
        fn face_tangent_plane(s: &Shape, fu: f64, fv: f64) -> Result<Frame>;
        fn face_curvatures(s: &Shape, fu: f64, fv: f64) -> Result<Conic>;
        fn face_fraction(s: &Shape, p: Vec3) -> Result<Vec3>;
        fn face_uv_bounds(s: &Shape) -> Result<Box3>;
        fn edge_end_vertices(s: &Shape) -> Result<UniquePtr<ShapeList>>;
        fn edges_tangent_at(a: &Shape, b: &Shape, vertex: &Shape, angle_tolerance: f64) -> Result<bool>;
        fn wire_smooth(wire: &Shape, angle_tolerance: f64) -> Result<bool>;
        fn faces_tangent_along(edge: &Shape, a: &Shape, b: &Shape, angle_tolerance: f64) -> Result<bool>;
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
    /// The same shape with its orientation reversed (a face's normal flipped).
    pub fn reversed(&self) -> Shape {
        Shape::new(ffi::shape_reversed(&self.0))
    }

    /// TopAbs orientation: 0 forward, 1 reversed, 2 internal, 3 external.
    pub fn orientation(&self) -> i32 {
        ffi::shape_orientation(&self.0)
    }

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

    /// The pieces of edges OCCT split at a seam vertex that [`Shape::visible_sub_shapes`] hides behind the piece it
    /// shows: (the piece shown, a hidden piece).
    pub fn hidden_edge_pieces(&self) -> Vec<(Shape, Shape)> {
        let list = from_list(&ffi::hidden_edge_pieces(&self.0).expect("a shape"));
        list.chunks(2).map(|pair| (pair[0].clone(), pair[1].clone())).collect()
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

    /// The principal moments of inertia about the centre of mass, smallest first (by volume, area or length as
    /// for [`Shape::centroid`]).
    pub fn principal_moments(&self) -> Result<Vec3> {
        Ok(ffi::shape_principal_moments(&self.0)?)
    }

    /// The measure of the given dimension (3 volume, 2 area, 1 length), the centre of mass, and the matrix of
    /// inertia about it in world axes, row by row (with unit density).
    pub fn mass_properties(&self, dimension: i32) -> Result<(f64, Vec3, [f64; 9])> {
        let v = ffi::shape_mass_properties(&self.0, dimension)?;
        let mut m = [0.0; 9];
        m.copy_from_slice(&v[4..13]);
        Ok((v[0], Vec3::new(v[1], v[2], v[3]), m))
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

    /// Where the infinite line through `origin` along the unit vector `direction` crosses this shape's faces, as
    /// sorted distances along it.
    pub fn line_crossings(&self, origin: Vec3, direction: Vec3) -> Result<Vec<f64>> {
        Ok(ffi::line_crossings(&self.0, origin, direction)?)
    }

    /// How this body's two faces meet along `edge`: 0 convex, 1 concave, 2 smooth, 3 variable, or `None` if it isn't
    /// between two faces. With `by_curvature`, smooth stretches count as convex or concave by how the faces bend.
    pub fn edge_convexity(&self, edge: &Shape, by_curvature: bool) -> Option<i32> {
        ffi::edge_convexity(&self.0, &edge.0, by_curvature).ok().filter(|k| *k >= 0)
    }

    /// The Frenet frame at a fraction along this edge (origin the point, `normal` the tangent, `x` the normal) and
    /// the curvature (`radius1`).
    pub fn edge_curvature(&self, t: f64, arc_length: bool) -> Result<Conic> {
        Ok(ffi::edge_curvature(&self.0, t, arc_length)?)
    }

    /// The derivative of the curvature vector with respect to arc length at a fraction along this edge.
    pub fn edge_curvature_derivative(&self, t: f64, arc_length: bool) -> Result<Vec3> {
        Ok(ffi::edge_curvature_derivative(&self.0, t, arc_length)?)
    }

    /// A point inside this face (not just inside its parameter range).
    pub fn face_inner_point(&self) -> Result<Vec3> {
        Ok(ffi::face_inner_point(&self.0)?)
    }

    /// Whether a point is inside this solid.
    pub fn contains_point(&self, p: Vec3) -> Result<bool> {
        Ok(ffi::point_in_solid(&self.0, p)?)
    }

    /// The minimum distance to another shape, and the closest points.
    pub fn distance(&self, other: &Shape) -> Result<Distance> {
        Ok(ffi::shape_distance(&self.0, &other.0)?)
    }

    /// The line, circle or ellipse an edge lies on.
    pub fn curve_definition(&self) -> Result<Conic> {
        Ok(ffi::curve_definition(&self.0)?)
    }

    /// The B-spline (or Bézier) surface a face lies on, whole; an error for any other kind of surface.
    pub fn face_bspline(&self) -> Result<BSplineSurfaceData> {
        Ok(ffi::face_bspline(&self.0)?)
    }

    /// The curve under an edge, over the edge's range, as a B-spline: exactly where it can be (conics become
    /// rational), otherwise within `tolerance`. `force_cubic` and `force_non_rational` approximate it as a cubic
    /// or polynomial spline where it isn't one.
    pub fn edge_bspline(&self, tolerance: f64, force_cubic: bool, force_non_rational: bool) -> Result<BSplineData> {
        Ok(ffi::edge_bspline(&self.0, tolerance, force_cubic, force_non_rational)?)
    }

    /// The surface a face lies on: kind 0 plane, 1 cylinder (`radius1`), 2 cone (`radius1` the half angle, frame at
    /// the apex), 3 sphere (`radius1`), 4 torus (`radius1` major, `radius2` minor), 5 other.
    pub fn surface_definition(&self) -> Result<Conic> {
        Ok(ffi::surface_definition(&self.0)?)
    }

    /// The plane tangent to a face at fractions of its parameter-space bounding box.
    pub fn face_tangent_plane(&self, fu: f64, fv: f64) -> Result<Frame> {
        Ok(ffi::face_tangent_plane(&self.0, fu, fv)?)
    }

    /// A face's principal curvatures at fractions of its parameter-space box: `radius1` the least and `radius2`
    /// the greatest (positive where it bends away from its outward normal), their directions `frame.normal` and
    /// `frame.x`, the point `frame.origin`.
    pub fn face_curvatures(&self, fu: f64, fv: f64) -> Result<Conic> {
        Ok(ffi::face_curvatures(&self.0, fu, fv)?)
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

    /// An edge's first and last vertices, in its own direction (the same vertex twice for a closed edge).
    pub fn edge_end_vertices(&self) -> Result<Vec<Shape>> {
        let list = ffi::edge_end_vertices(&self.0)?;
        Ok(from_list(&list))
    }

    /// Whether an edge starts and ends at one vertex: a whole circle's, a closed spline's.
    pub fn is_closed_edge(&self) -> bool {
        self.edge_end_vertices().is_ok_and(|v| v.len() == 2 && v[0].is_same(&v[1]))
    }

    /// Whether this edge and `other`, meeting at `vertex`, continue smoothly through it.
    pub fn tangent_at(&self, other: &Shape, vertex: &Shape, angle_tolerance: f64) -> bool {
        ffi::edges_tangent_at(&self.0, &other.0, &vertex.0, angle_tolerance).unwrap_or(false)
    }

    /// Whether this wire continues smoothly through every vertex two of its edges share.
    pub fn wire_smooth(&self, angle_tolerance: f64) -> bool {
        ffi::wire_smooth(&self.0, angle_tolerance).unwrap_or(false)
    }

    /// Whether faces `a` and `b` meet smoothly along this edge.
    pub fn faces_tangent_along(&self, a: &Shape, b: &Shape, angle_tolerance: f64) -> bool {
        ffi::faces_tangent_along(&self.0, &a.0, &b.0, angle_tolerance).unwrap_or(false)
    }

    /// An edge or face extended to its underlying line, plane or closed curve.
    pub fn extended(&self, reach: f64) -> Result<Shape> {
        Ok(Shape::new(ffi::extended_shape(&self.0, reach)?))
    }

    /// Whether OCCT's checker finds the shape valid.
    pub fn is_valid(&self) -> bool {
        ffi::shape_valid(&self.0).unwrap_or(false)
    }

    /// The largest tolerance of the shape's edges and vertices: how far OCCT lets them stand off their curves and
    /// faces (1e-7 m at best).
    pub fn max_tolerance(&self) -> f64 {
        ffi::shape_max_tolerance(&self.0).unwrap_or(0.0)
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

/// A whole ellipse in a frame (x along the `major` axis), or an arc of it from `start` to `end` (radians from the
/// major axis towards the minor).
pub fn ellipse_edge(frame: Frame, major: f64, minor: f64, arc: Option<(f64, f64)>) -> Result<Shape> {
    let (start, end) = arc.unwrap_or((0.0, 0.0));
    Ok(Shape::new(ffi::make_ellipse_edge(frame, major, minor, arc.is_none(), start, end)?))
}

/// A cubic spline through points, each span the cubic Hermite segment fixed by the derivatives at its ends.
/// `parameters` has one value per point, plus one more (for the span back to the start) if `closed`.
pub fn interpolated_edge(points: &[Vec3], derivatives: &[Vec3], parameters: &[f64], closed: bool) -> Result<Shape> {
    Ok(Shape::new(ffi::make_interpolated_edge(points, derivatives, parameters, closed)?))
}

/// A Bézier curve with these control points.
/// An edge on a B-spline: poles, weights (empty if not rational), the full knot vector, degree and periodicity.
pub fn bspline_edge(poles: &[Vec3], weights: &[f64], knots: &[f64], degree: i32, periodic: bool) -> Result<Shape> {
    Ok(Shape::new(ffi::make_bspline_edge(poles, weights, knots, degree, periodic)?))
}

/// A face on a non-rational B-spline surface over its whole domain: `poles` row by row (`nu` rows of `nv`), with
/// full knot vectors.
pub fn bspline_surface_face(
    poles: &[Vec3],
    (nu, nv): (usize, usize),
    knots_u: &[f64],
    knots_v: &[f64],
    (degree_u, degree_v): (i32, i32),
) -> Result<Shape> {
    Ok(Shape::new(ffi::make_bspline_surface_face(poles, nu as i32, nv as i32, knots_u, knots_v, degree_u, degree_v)?))
}

/// A face on a rational B-spline surface over its whole domain: as [`bspline_surface_face`], with a weight for each
/// pole.
pub fn rational_bspline_surface_face(
    poles: &[Vec3],
    weights: &[f64],
    (nu, nv): (usize, usize),
    knots_u: &[f64],
    knots_v: &[f64],
    (degree_u, degree_v): (i32, i32),
) -> Result<Shape> {
    Ok(Shape::new(ffi::make_rational_bspline_surface_face(
        poles, weights, nu as i32, nv as i32, knots_u, knots_v, degree_u, degree_v,
    )?))
}

pub fn bezier_edge(points: &[Vec3]) -> Result<Shape> {
    Ok(Shape::new(ffi::make_bezier_edge(points)?))
}

/// A conic from `a` to `b` tangent at them to the lines through `control`, with weight `w` at the control point.
pub fn conic_edge(a: Vec3, control: Vec3, b: Vec3, w: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::make_conic_edge(a, control, b, w)?))
}

/// Edges that meet end to end, joined into wires.
pub fn connect_edges(edges: &[Shape]) -> Result<Vec<Shape>> {
    let wires = ffi::connect_edges(&to_list(edges))?;
    Ok(from_list(&wires))
}

/// The face between a planar wire offset `o1` to one side and `o2` to the other, with sharp corners: right of the
/// first edge (direction × normal) for an open wire, outward for a closed one.
pub fn offset_wire_region(wire: &Shape, normal: Vec3, o1: f64, o2: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::offset_wire_region(&wire.0, normal, o1, o2)?))
}

/// A helix about an axis, starting at `start` and turning from revolution `t0` to `t1`, `pitch` along the axis and
/// `spiral` outwards per revolution.
#[allow(clippy::too_many_arguments)]
pub fn helix_edge(
    origin: Vec3,
    direction: Vec3,
    start: Vec3,
    t0: f64,
    t1: f64,
    clockwise: bool,
    pitch: f64,
    spiral: f64,
) -> Result<Shape> {
    Ok(Shape::new(ffi::make_helix_edge(origin, direction, start, t0, t1, clockwise, pitch, spiral)?))
}

/// Sweeps a face or wire along a path wire.
/// A sweep whose profile keeps its orientation (`lock` none) or turns only about the direction `lock` gives.
pub fn sweep_controlled(profile: &Shape, path: &Shape, lock: Option<Vec3>) -> Result<Operation> {
    let (mode, direction) = match lock {
        None => (1, Vec3::new(0.0, 0.0, 1.0)),
        Some(d) => (2, d),
    };
    Ok(Operation(ffi::sweep_controlled_op(&profile.0, &path.0, mode, direction)?))
}

pub fn sweep(profile: &Shape, path: &Shape) -> Result<Operation> {
    Ok(Operation(ffi::sweep_op(&profile.0, &path.0)?))
}

/// Lofts through wires in order, into a solid or a sheet.
pub fn loft(wires: &[Shape], solid: bool) -> Result<Operation> {
    Ok(Operation(ffi::loft_op(&to_list(wires), solid)?))
}

/// A loft through wires whose starts and directions already line up, without OpenCascade's own matching.
pub fn loft_as_given(wires: &[Shape], solid: bool) -> Result<Operation> {
    Ok(Operation(ffi::loft_op_matching(&to_list(wires), solid, false)?))
}

/// Lofts end to end through groups of sections (each group's first section the last one's last), sewn into one
/// body with the first group's first end and the last group's last end.
pub fn loft_pieces(groups: &[Vec<Shape>], solid: bool) -> Result<Operation> {
    let mut wires: Vec<Shape> = Vec::new();
    let mut counts = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        counts.push(group.len() as u32);
        wires.extend(group.iter().skip(if g == 0 { 0 } else { 1 }).cloned());
    }
    Ok(Operation(ffi::loft_pieces_op(&to_list(&wires), &counts, solid)?))
}

/// The condition at one end of a loft: a vector, a magnitude, whether the derivative lies in the plane square to
/// the vector rather than along it, the faces whose tangency it matches (none: along the vector), and whether it
/// matches their curvature too.
#[derive(Clone, Debug)]
pub struct LoftEnd {
    pub vector: Vec3,
    pub magnitude: f64,
    pub tangent: bool,
    pub faces: Vec<Shape>,
    pub curvature: bool,
}

/// A loft with conditions at its ends (`None` leaves that end free).
pub fn loft_clamped(wires: &[Shape], solid: bool, start: Option<LoftEnd>, end: Option<LoftEnd>) -> Result<Operation> {
    let free = || LoftEnd {
        vector: Vec3::new(0.0, 0.0, 1.0),
        magnitude: f64::NAN,
        tangent: false,
        faces: Vec::new(),
        curvature: false,
    };
    let (a, b) = (start.unwrap_or_else(free), end.unwrap_or_else(free));
    Ok(Operation(ffi::loft_clamped_op(
        &to_list(wires),
        solid,
        a.vector,
        a.magnitude,
        a.tangent,
        &to_list(&a.faces),
        a.curvature,
        b.vector,
        b.magnitude,
        b.tangent,
        &to_list(&b.faces),
        b.curvature,
    )?))
}

/// A face's outer wire.
pub fn outer_wire(face: &Shape) -> Result<Shape> {
    Ok(Shape::new(ffi::outer_wire(&face.0)?))
}

/// The curves where two sets of faces meet.
pub fn section_edges(a: &[Shape], b: &[Shape]) -> Result<Vec<Shape>> {
    let edges = ffi::section_edges(&to_list(a), &to_list(b))?;
    Ok(from_list(&edges))
}

/// A face filling a closed loop of edges: planar if the loop is flat.
pub fn fill_edges(edges: &[Shape]) -> Result<Shape> {
    Ok(Shape::new(ffi::fill_edges(&to_list(edges))?))
}

/// Solids for the regions faces and bodies enclose.
pub fn enclose(shapes: &[Shape]) -> Result<Vec<Shape>> {
    let solids = ffi::enclose_shapes(&to_list(shapes))?;
    Ok(from_list(&solids))
}

/// Copies of faces, sewn where they share edges, offset along their normals.
pub fn extract_faces(faces: &[Shape], offset: f64) -> Result<Vec<Shape>> {
    let sheets = ffi::extract_faces(&to_list(faces), offset)?;
    Ok(from_list(&sheets))
}

/// A planar sheet with some boundary edges moved outward by `distance`.
pub fn extend_planar_sheet(sheet: &Shape, edges: &[Shape], distance: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::extend_planar_sheet(&sheet.0, &to_list(edges), distance)?))
}

/// A planar face with some straight boundary edges moved outward, each by its own distance (negative moves it in).
pub fn extend_planar_face(face: &Shape, edges: &[Shape], distances: &[f64]) -> Result<Shape> {
    Ok(Shape::new(ffi::extend_planar_face(&face.0, &to_list(edges), distances)?))
}

/// A face on the cylinder about `frame`'s normal through its origin, all the way round and `half_height` along the
/// axis each way, its normal pointing away from the axis.
/// A face on the same surface as `face`, bounded by the parameter rectangle `u` × `v`, facing the same way.
pub fn surface_patch(face: &Shape, u: (f64, f64), v: (f64, f64)) -> Result<Shape> {
    Ok(Shape::new(ffi::surface_patch(&face.0, u.0, u.1, v.0, v.1)?))
}

pub fn cylinder_face(frame: Frame, radius: f64, half_height: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::cylinder_face(frame, radius, half_height)?))
}

/// A planar face with corners at some vertices rounded to `size`, or (`chamfer`) cut back `size` along both edges.
/// A planar face with one corner chamfered (`d1` along the edge coming into the corner in the face's loop order,
/// `d2` along the one leaving it).
pub fn face_corner_chamfer_2d(face: &Shape, vertex: &Shape, d1: f64, d2: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::face_corner_2d(&face.0, &vertex.0, 1, d1, d2, 0.0)?))
}

/// A planar face with one corner rounded by a conic set back `setback` along both edges, its shoulder at `rho` of
/// the way from the chord's middle to the corner.
pub fn face_corner_conic_2d(face: &Shape, vertex: &Shape, setback: f64, rho: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::face_corner_2d(&face.0, &vertex.0, 2, setback, setback, rho)?))
}

pub fn face_corners_2d(face: &Shape, vertices: &[Shape], size: f64, chamfer: bool) -> Result<Shape> {
    Ok(Shape::new(ffi::face_corners_2d(&face.0, &to_list(vertices), size, chamfer)?))
}

/// Faces sewn together where their edges meet: a shell (or compound of shells), for a sheet body.
pub fn sew_faces(faces: &[Shape], tolerance: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::sew_faces(&to_list(faces), tolerance)?))
}

/// The iso curve of a face at fraction `t` of its parameter range, along u or v, trimmed to the face.
pub fn iso_curves(face: &Shape, along_u: bool, t: f64) -> Result<Vec<Shape>> {
    let edges = ffi::iso_curves(&face.0, along_u, t)?;
    Ok(from_list(&edges))
}

/// Whether a face's parameter is periodic along u (or v).
pub fn face_periodic(face: &Shape, along_u: bool) -> bool {
    ffi::face_periodic(&face.0, along_u).unwrap_or(false)
}

/// A wire's edges joined into one edge, approximating where their curves differ.
pub fn join_edges(wire: &Shape) -> Result<Shape> {
    Ok(Shape::new(ffi::join_edges(&wire.0)?))
}

/// An edge on a face's surface through points given as fractions (u, v) of its parameter ranges, a spline through
/// them in its parameter plane.
pub fn edge_on_face(face: &Shape, uv: &[(f64, f64)]) -> Result<Shape> {
    let flat: Vec<f64> = uv.iter().flat_map(|(u, v)| [*u, *v]).collect();
    Ok(Shape::new(ffi::edge_on_face(&face.0, &flat)?))
}

/// The outline of solids seen along a plane's normal, drawn on the plane: the faces they cover.
pub fn outline_on_plane(tools: &[Shape], plane: Frame) -> Result<Shape> {
    Ok(Shape::new(ffi::outline_on_plane(&to_list(tools), plane)?))
}

/// A closed edge on a face's surface through points given as fractions (u, v) of its parameter ranges (the first
/// not repeated at the end), a periodic spline through them in its parameter plane.
pub fn closed_edge_on_face(face: &Shape, uv: &[(f64, f64)]) -> Result<Shape> {
    let flat: Vec<f64> = uv.iter().flat_map(|(u, v)| [*u, *v]).collect();
    Ok(Shape::new(ffi::closed_edge_on_face(&face.0, &flat)?))
}

/// A wire of edges in the order given, their ends joined where they nearly meet.
pub fn ordered_wire(edges: &[Shape]) -> Result<Shape> {
    Ok(Shape::new(ffi::ordered_wire(&to_list(edges))?))
}

/// The piece of an edge from fraction `f0` to `f1` of its parameter range, along the edge's direction (its curve
/// reversed if `reverse`); on a closed edge, `f0` past `f1` goes round through its end.
pub fn edge_between(edge: &Shape, f0: f64, f1: f64, reverse: bool) -> Result<Shape> {
    Ok(Shape::new(ffi::edge_between(&edge.0, f0, f1, reverse)?))
}

/// Edges projected onto faces along a direction.
pub fn project_edges(edges: &[Shape], faces: &[Shape], direction: Vec3) -> Result<Vec<Shape>> {
    let projected = ffi::project_edges(&to_list(edges), &to_list(faces), direction)?;
    Ok(from_list(&projected))
}

/// A straight edge of a planar face moved `distance` across the face, inwards for positive distances.
pub fn offset_edge_on_planar_face(edge: &Shape, face: &Shape, distance: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::offset_edge_on_planar_face(&edge.0, &face.0, distance)?))
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

/// A shape with its same-domain edges merged (collinear lines meeting at a vertex only they share), its faces
/// left as they are.
pub fn merge_edges(s: &Shape) -> Result<Operation> {
    Ok(Operation(ffi::merge_edges(&s.0)?))
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

/// Rounds edges of a solid with a constant radius.
/// Rounds edges with a radius varying from `r0[i]` at the end of edge i nearest `p0[i]` to `r1[i]` at the other,
/// smoothly or linearly.
pub fn variable_fillet(
    solid: &Shape,
    edges: &[Shape],
    p0: &[Vec3],
    r0: &[f64],
    r1: &[f64],
    smooth: bool,
) -> Result<Operation> {
    Ok(Operation(ffi::variable_fillet_op(&solid.0, &to_list(edges), p0, r0, r1, smooth)?))
}

pub fn fillet(solid: &Shape, edges: &[Shape], radius: f64) -> Result<Operation> {
    Ok(Operation(ffi::fillet_op(&solid.0, &to_list(edges), radius)?))
}

/// How a chamfer is sized.
#[derive(Clone, Copy, Debug)]
pub enum Chamfer {
    /// Both new edges this far from the original edge.
    Equal(f64),
    /// The first distance on the edge's reference face, the second on the other face.
    TwoDistances(f64, f64),
    /// A distance on the reference face and an angle (radians) from it.
    DistanceAngle(f64, f64),
}

/// Chamfers edges of a solid; `faces` holds a reference face next to each edge for the uneven kinds.
pub fn chamfer(solid: &Shape, edges: &[Shape], faces: &[Shape], kind: Chamfer) -> Result<Operation> {
    let (k, d1, d2) = match kind {
        Chamfer::Equal(d) => (0, d, 0.0),
        Chamfer::TwoDistances(a, b) => (1, a, b),
        Chamfer::DistanceAngle(d, a) => (2, d, a),
    };
    Ok(Operation(ffi::chamfer_op(&solid.0, &to_list(edges), &to_list(faces), k, d1, d2)?))
}

/// Shells a solid, removing `faces`: `thickness` positive outward, negative inward.
pub fn shell(solid: &Shape, faces: &[Shape], thickness: f64) -> Result<Operation> {
    Ok(Operation(ffi::shell_op(&solid.0, &to_list(faces), thickness)?))
}

/// The whole shape offset by `distance` (positive outward), with sharp corners.
pub fn offset(shape: &Shape, distance: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::offset_shape(&shape.0, distance)?))
}

/// Drafts faces of a solid by `angle` about where they meet a neutral plane.
pub fn draft(solid: &Shape, faces: &[Shape], pull: Vec3, angle: f64, neutral: Frame) -> Result<Operation> {
    Ok(Operation(ffi::draft_op(&solid.0, &to_list(faces), pull, angle, &[neutral])?))
}

/// Drafts faces of a solid by `angle`, each about where it meets its own neutral plane.
pub fn draft_each(solid: &Shape, faces: &[Shape], pull: Vec3, angle: f64, neutrals: &[Frame]) -> Result<Operation> {
    Ok(Operation(ffi::draft_op(&solid.0, &to_list(faces), pull, angle, neutrals)?))
}

/// Faces of a solid drafted each about its own neutral plane, at its own angle.
pub fn draft_angles(
    solid: &Shape,
    faces: &[Shape],
    pull: Vec3,
    angles: &[f64],
    neutrals: &[Frame],
) -> Result<Operation> {
    Ok(Operation(ffi::draft_angles_op(&solid.0, &to_list(faces), pull, angles, neutrals)?))
}

/// A sheet with its faces turned to agree with each other across their shared edges.
/// The shape with its curved faces and edges made B-splines (planes stay planes).
pub fn nurbs_converted(shape: &Shape) -> Result<Shape> {
    Ok(Shape::new(ffi::nurbs_converted(&shape.0)?))
}

/// A face on the same surface as `face`, bounded by one loop of edges in its parameter space, each the polyline
/// through its (u, v) points (an edge at constant u or v is the surface's iso curve there), facing the same way.
pub fn face_from_uv(face: &Shape, edges: &[Vec<(f64, f64)>]) -> Result<Shape> {
    let uv: Vec<f64> = edges.iter().flatten().flat_map(|&(u, v)| [u, v]).collect();
    let counts: Vec<u32> = edges.iter().map(|e| e.len() as u32).collect();
    Ok(Shape::new(ffi::face_from_uv(&face.0, &uv, &counts)?))
}

pub fn orient_sheet(sheet: &Shape) -> Result<Shape> {
    Ok(Shape::new(ffi::orient_sheet(&sheet.0)?))
}

/// Thickens a smooth sheet `t1` along its normals and `t2` against them, from its offsets and faces across the
/// thickness at its boundary (sheet metal's walls and rounded bends). Gives the solid only, without history.
pub fn thicken_sheet(sheet: &Shape, t1: f64, t2: f64) -> Result<Shape> {
    Ok(Shape::new(ffi::thicken_sheet(&sheet.0, t1, t2)?))
}

/// Thickens a face or sheet `t1` along its normal and `t2` against it.
pub fn thicken(sheet: &Shape, t1: f64, t2: f64) -> Result<Operation> {
    Ok(Operation(ffi::thicken_op(&sheet.0, t1, t2)?))
}

/// Offsets faces of a solid along their normals, adjusting the faces around them.
pub fn offset_faces(solid: &Shape, faces: &[Shape], distance: f64) -> Result<Operation> {
    Ok(Operation(ffi::offset_faces_op(&solid.0, &to_list(faces), distance)?))
}

/// Removes faces of a solid and extends the faces around them to close it.
pub fn delete_faces(solid: &Shape, faces: &[Shape]) -> Result<Operation> {
    Ok(Operation(ffi::delete_faces_op(&solid.0, &to_list(faces))?))
}

/// Replaces a planar face of a solid with a plane, extending or trimming the faces next to it, which must be
/// perpendicular to it.
/// Replaces a planar face of a solid with another face's surface (offset along its normal); see
/// `replace_planar_face`.
pub fn replace_face_with_surface(solid: &Shape, face: &Shape, template: &Shape, offset: f64) -> Result<Operation> {
    Ok(Operation(ffi::replace_face_with_surface_op(&solid.0, &face.0, &template.0, offset)?))
}

pub fn replace_planar_face(solid: &Shape, face: &Shape, target: Frame) -> Result<Operation> {
    Ok(Operation(ffi::replace_planar_face_op(&solid.0, &face.0, target)?))
}

/// A solid from a shape: its first solid, or one made from its closed shells.
/// The solid some faces of a solid bound once their open boundary is capped with planar faces: a boss's material
/// or a pocket's void.
pub fn face_feature_solid(faces: &[Shape]) -> Result<Shape> {
    Ok(Shape::new(ffi::face_feature_solid(&to_list(faces))?))
}

/// A rolling ball blend of `radius` between two sets of faces meeting along edges, each set reversed if asked (so
/// its normals point away from the blend).
pub fn face_blend(side1: &[Shape], reverse1: bool, side2: &[Shape], reverse2: bool, radius: f64) -> Result<Operation> {
    Ok(Operation(ffi::face_blend_op(&to_list(side1), reverse1, &to_list(side2), reverse2, radius)?))
}

/// Faces of a cylinder unwrapped onto a plane without stretching: the source frame (anchor and direction on the
/// cylinder, and its face's normal) goes to the destination frame (anchor, direction and normal of the plane).
pub fn unwrap_from_cylinder(
    faces: &[Shape],
    source: &Shape,
    source_frame: (Vec3, Vec3),
    destination: (Vec3, Vec3, Vec3),
) -> Result<Operation> {
    let op = ffi::unwrap_from_cylinder(
        &to_list(faces),
        &source.0,
        source_frame.0,
        source_frame.1,
        destination.0,
        destination.1,
        destination.2,
    )?;
    Ok(Operation(op))
}

/// Planar faces wrapped onto a cylinder face without stretching: the source frame (anchor, direction, plane
/// normal) goes to the destination frame (anchor and direction on the cylinder, and the face's normal).
pub fn wrap_onto_cylinder(
    faces: &[Shape],
    source: (Vec3, Vec3, Vec3),
    destination: &Shape,
    destination_anchor: Vec3,
    destination_direction: Vec3,
    arc_radius: Option<f64>,
) -> Result<Operation> {
    let op = ffi::wrap_onto_cylinder(
        &to_list(faces),
        source.0,
        source.1,
        source.2,
        &destination.0,
        destination_anchor,
        destination_direction,
        arc_radius.unwrap_or(0.0),
    )?;
    Ok(Operation(op))
}

/// The faces of a shape grouped into a shell for each set connected through shared edges.
pub fn connected_shells(shape: &Shape) -> Result<Vec<Shape>> {
    let shells = ffi::connected_shells(&shape.0)?;
    Ok(from_list(&shells))
}

/// Rounds faces between two parallel planar side faces into a half cylinder tangent to both.
pub fn full_round(solid: &Shape, side1: &Shape, side2: &Shape, centre: &[Shape]) -> Result<Operation> {
    Ok(Operation(ffi::full_round_op(&solid.0, &side1.0, &side2.0, &to_list(centre))?))
}

/// Splits faces of a body where the tools (sheets, faces or wires) cross them.
/// A body with one of its faces split along edges lying on it.
pub fn split_face_by_edges(body: &Shape, face: &Shape, edges: &[Shape]) -> Result<Operation> {
    Ok(Operation(ffi::split_face_by_edges_op(&body.0, &face.0, &to_list(edges))?))
}

pub fn split_faces(body: &Shape, faces: &[Shape], tools: &[Shape]) -> Result<Operation> {
    Ok(Operation(ffi::split_faces_op(&body.0, &to_list(faces), &to_list(tools))?))
}

/// A body with faces on one surface merged across the `removable` edges between them (and edges left on one curve
/// where those met them joined); every other edge stays.
pub fn merge_faces_across(body: &Shape, removable: &[Shape]) -> Result<Operation> {
    Ok(Operation(ffi::merge_faces_across_op(&body.0, &to_list(removable))?))
}

/// Splits faces of a body where tools cross them, each tool only the face `owners` gives it (by index into
/// `faces`; a negative owner, every face). Tools that cross nothing leave the body as it was.
pub fn split_faces_paired(body: &Shape, faces: &[Shape], tools: &[Shape], owners: &[i32]) -> Result<Operation> {
    Ok(Operation(ffi::split_faces_paired_op(&body.0, &to_list(faces), &to_list(tools), owners)?))
}

/// A body of one doubly periodic face (a whole torus) split along two closed curves that each go once round it the
/// first way, given by points along them: the two bands between them.
pub fn split_by_bands(body: &Shape, face: &Shape, first: &[Vec3], second: &[Vec3]) -> Result<Operation> {
    Ok(Operation(ffi::split_by_bands_op(&body.0, &face.0, first, second)?))
}

/// A body's face periodic the second way but not the first (part of a torus) split along two curves through
/// `first` and `second` points that each run from one of its ends to the other: the bands between them, sewn with
/// the body's other faces.
pub fn split_by_open_bands(body: &Shape, face: &Shape, first: &[Vec3], second: &[Vec3]) -> Result<Operation> {
    Ok(Operation(ffi::split_by_open_bands_op(&body.0, &face.0, first, second)?))
}

pub fn solid_from(shape: &Shape) -> Result<Shape> {
    Ok(Shape::new(ffi::solid_from(&shape.0)?))
}

/// The faces of a solid other than `faces`, as an open sheet.
pub fn remove_faces_open(solid: &Shape, faces: &[Shape]) -> Result<Shape> {
    Ok(Shape::new(ffi::remove_faces_open(&solid.0, &to_list(faces))?))
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
