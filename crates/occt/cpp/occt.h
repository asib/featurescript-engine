// The C++ half of the `occt` crate: thin wrappers over OpenCascade that the cxx bridge in src/lib.rs exposes.
//
// Each function does one whole operation, so the bridge never has to deal with OCCT handles, templates or
// collections. OCCT throws `Standard_Failure`, which isn't a `std::exception`, so `rust::behavior::trycatch` below
// turns both kinds into Rust errors.

#pragma once

#include <Standard_Failure.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS_Shape.hxx>

#include <exception>
#include <memory>
#include <string>
#include <vector>

#include "rust/cxx.h"

namespace rust {
namespace behavior {
template <typename Try, typename Fail>
static void trycatch(Try &&func, Fail &&fail) noexcept try {
  func();
} catch (const Standard_Failure &e) {
  std::string message = e.DynamicType()->Name();
  const char *detail = e.GetMessageString();
  if (detail != nullptr && detail[0] != '\0') {
    message += ": ";
    message += detail;
  }
  fail(message);
} catch (const std::exception &e) {
  fail(e.what());
}
}  // namespace behavior
}  // namespace rust

class BRepTools_History;

namespace occt {

struct Vec3;
struct Frame;
struct Box3;
struct Transform;
struct Axis;
struct Distance;
struct Conic;
struct BSplineData;

// A TopoDS_Shape. Copying one is cheap: the copy shares the underlying topology.
class Shape {
 public:
  explicit Shape(TopoDS_Shape shape) : shape(std::move(shape)) {}
  TopoDS_Shape shape;
};

class ShapeList {
 public:
  std::vector<TopoDS_Shape> shapes;
};

// The result of a modelling operation and where each input vertex, edge, face and solid went.
class Operation {
 public:
  TopoDS_Shape result;
  opencascade::handle<BRepTools_History> history;
  // Extrude caps: the faces at the start and at the end of the sweep.
  std::vector<TopoDS_Shape> first, last;
};

std::unique_ptr<Shape> shape_clone(const Shape &s);
std::unique_ptr<Shape> shape_reversed(const Shape &s);
bool shape_is_null(const Shape &s);
int32_t shape_kind(const Shape &s);
int32_t shape_orientation(const Shape &s);
bool shape_is_same(const Shape &a, const Shape &b);
uint64_t shape_hash(const Shape &s);

std::unique_ptr<ShapeList> list_new();
size_t list_len(const ShapeList &l);
std::unique_ptr<Shape> list_get(const ShapeList &l, size_t i);
void list_push(ShapeList &l, const Shape &s);

std::unique_ptr<ShapeList> sub_shapes(const Shape &s, int32_t kind);
std::unique_ptr<ShapeList> ancestors(const Shape &root, const Shape &s, int32_t kind);
std::unique_ptr<Shape> make_compound(const ShapeList &l);

std::unique_ptr<Shape> make_vertex(Vec3 p);
std::unique_ptr<Shape> make_line_edge(Vec3 a, Vec3 b);
std::unique_ptr<Shape> make_circle_edge(Frame f, double radius);
std::unique_ptr<Shape> make_arc_edge(Frame f, double radius, double start, double end);
std::unique_ptr<Shape> make_arc_through(Vec3 a, Vec3 mid, Vec3 b);
std::unique_ptr<Shape> make_plane_face(Frame f, double umin, double umax, double vmin, double vmax);
std::unique_ptr<Shape> make_ellipse_edge(Frame f, double major, double minor, bool whole, double start, double end);
std::unique_ptr<Shape> make_interpolated_edge(rust::Slice<const Vec3> points, rust::Slice<const Vec3> derivatives,
                                              rust::Slice<const double> parameters, bool closed);
std::unique_ptr<Shape> orient_sheet(const Shape &input);
std::unique_ptr<Shape> thicken_sheet(const Shape &input, double t1, double t2);
std::unique_ptr<Shape> make_bspline_surface_face(rust::Slice<const Vec3> poles, int32_t nu, int32_t nv,
                                                 rust::Slice<const double> knots_u, rust::Slice<const double> knots_v,
                                                 int32_t degree_u, int32_t degree_v);
std::unique_ptr<Shape> make_bspline_edge(rust::Slice<const Vec3> poles, rust::Slice<const double> weights,
                                         rust::Slice<const double> knots, int32_t degree, bool periodic);
std::unique_ptr<Shape> make_bezier_edge(rust::Slice<const Vec3> points);
std::unique_ptr<Shape> make_conic_edge(Vec3 a, Vec3 c, Vec3 b, double w);
std::unique_ptr<ShapeList> connect_edges(const ShapeList &edges);
std::unique_ptr<Shape> offset_wire_region(const Shape &wire, Vec3 normal, double o1, double o2);
std::unique_ptr<Shape> make_helix_edge(Vec3 origin, Vec3 direction, Vec3 start, double t0, double t1, bool clockwise,
                                       double pitch, double spiral);
std::unique_ptr<Operation> sweep_op(const Shape &profile, const Shape &path);
std::unique_ptr<Operation> loft_op(const ShapeList &wires, bool solid);
std::unique_ptr<Shape> outer_wire(const Shape &face);
std::unique_ptr<ShapeList> section_edges(const ShapeList &a, const ShapeList &b);
std::unique_ptr<Shape> fill_edges(const ShapeList &edges);
std::unique_ptr<ShapeList> enclose_shapes(const ShapeList &shapes);
std::unique_ptr<ShapeList> extract_faces(const ShapeList &faces, double offset);
std::unique_ptr<Shape> extend_planar_sheet(const Shape &sheet, const ShapeList &edges, double distance);
std::unique_ptr<Shape> extend_planar_face(const Shape &sheet, const ShapeList &edges, rust::Slice<const double> distances);
std::unique_ptr<Shape> sew_faces(const ShapeList &faces, double tolerance);
std::unique_ptr<Shape> cylinder_face(Frame frame, double radius, double half_height);
std::unique_ptr<Shape> face_corners_2d(const Shape &face_shape, const ShapeList &vertices, double size, bool chamfer);
std::unique_ptr<ShapeList> iso_curves(const Shape &face, bool along_u, double t);
bool face_periodic(const Shape &face, bool along_u);
std::unique_ptr<Shape> join_edges(const Shape &wire);
std::unique_ptr<ShapeList> project_edges(const ShapeList &edges, const ShapeList &faces, Vec3 direction);
std::unique_ptr<Shape> offset_edge_on_planar_face(const Shape &edge, const Shape &face, double distance);

std::unique_ptr<ShapeList> sketch_regions(const ShapeList &edges, Frame f);
std::unique_ptr<Operation> merge_faces(const ShapeList &faces);
std::unique_ptr<Operation> prism(const Shape &profile, Vec3 v);
std::unique_ptr<Operation> boolean_op(int32_t kind, const ShapeList &objects, const ShapeList &tools, bool unify,
                                      double fuzzy);
std::unique_ptr<Operation> transform_op(const Shape &s, const Transform &t);
std::unique_ptr<Operation> revolve(const Shape &profile, Axis axis, double angle);
std::unique_ptr<Shape> make_sphere(Vec3 center, double radius);
std::unique_ptr<Operation> split_op(const ShapeList &objects, const ShapeList &tools);
std::unique_ptr<Operation> variable_fillet_op(const Shape &solid, const ShapeList &edges, rust::Slice<const Vec3> p0,
                                              rust::Slice<const double> r0, rust::Slice<const double> r1, bool smooth);
std::unique_ptr<Operation> fillet_op(const Shape &solid, const ShapeList &edges, double radius);
std::unique_ptr<Operation> chamfer_op(const Shape &solid, const ShapeList &edges, const ShapeList &faces,
                                      int32_t kind, double d1, double d2);
std::unique_ptr<Operation> shell_op(const Shape &solid, const ShapeList &faces, double thickness);
std::unique_ptr<Shape> offset_shape(const Shape &s, double distance);
std::unique_ptr<Operation> draft_op(const Shape &solid, const ShapeList &faces, Vec3 pull, double angle,
                                    rust::Slice<const Frame> neutrals);
std::unique_ptr<Operation> thicken_op(const Shape &sheet, double t1, double t2);
std::unique_ptr<Operation> offset_faces_op(const Shape &solid, const ShapeList &faces, double distance);
std::unique_ptr<Operation> delete_faces_op(const Shape &solid, const ShapeList &faces);
std::unique_ptr<Shape> remove_faces_open(const Shape &solid, const ShapeList &faces);
std::unique_ptr<Shape> solid_from(const Shape &s);
std::unique_ptr<Operation> replace_planar_face_op(const Shape &solid, const Shape &face, Frame target);

std::unique_ptr<Shape> operation_shape(const Operation &op);
std::unique_ptr<ShapeList> operation_modified(const Operation &op, const Shape &s);
std::unique_ptr<ShapeList> operation_generated(const Operation &op, const Shape &s);
bool operation_is_deleted(const Operation &op, const Shape &s);
std::unique_ptr<ShapeList> operation_first(const Operation &op);
std::unique_ptr<ShapeList> operation_last(const Operation &op);

Box3 bounding_box(const Shape &s, bool precise);
Box3 bounding_box_in(const Shape &s, Frame frame, bool precise);
double shape_volume(const Shape &s);
double shape_area(const Shape &s);
double shape_length(const Shape &s);
Vec3 shape_centroid(const Shape &s);
Vec3 shape_principal_moments(const Shape &s);
Vec3 vertex_point(const Shape &s);
int32_t surface_type(const Shape &face);
Frame face_plane(const Shape &face);
int32_t curve_type(const Shape &edge);
bool edge_is_seam(const Shape &root, const Shape &edge);
bool shape_valid(const Shape &s);
std::unique_ptr<ShapeList> visible_sub_shapes(const Shape &s, int32_t kind);
Axis shape_axis(const Shape &s);
Axis edge_tangent(const Shape &s, double t, bool arc_length);
double edge_fraction(const Shape &s, Vec3 p, bool arc_length);
double edge_fraction_extended(const Shape &s, Vec3 p);
Distance shape_distance(const Shape &a, const Shape &b);
rust::Vec<double> line_crossings(const Shape &s, Vec3 origin, Vec3 direction);
Vec3 face_inner_point(const Shape &s);
Conic edge_curvature(const Shape &s, double t, bool arc_length);
Vec3 edge_curvature_derivative(const Shape &s, double t, bool arc_length);
std::unique_ptr<Operation> full_round_op(const Shape &solid, const Shape &side1, const Shape &side2, const ShapeList &centre);
int32_t edge_convexity(const Shape &body, const Shape &edge, bool by_curvature);
std::unique_ptr<Shape> face_feature_solid(const ShapeList &faces);
std::unique_ptr<ShapeList> connected_shells(const Shape &s);
std::unique_ptr<Operation> wrap_onto_cylinder(const ShapeList &faces, Vec3 source_anchor, Vec3 source_direction,
                                              Vec3 source_normal, const Shape &destination, Vec3 destination_anchor,
                                              Vec3 destination_direction, double arc_radius);
std::unique_ptr<Operation> face_blend_op(const ShapeList &side1, bool reverse1, const ShapeList &side2, bool reverse2,
                                         double radius);
std::unique_ptr<Operation> split_faces_op(const Shape &body, const ShapeList &faces, const ShapeList &tools);
bool point_in_solid(const Shape &solid, Vec3 p);
Conic curve_definition(const Shape &s);
BSplineData edge_bspline(const Shape &s, double tolerance, bool force_cubic, bool force_non_rational);
Conic surface_definition(const Shape &s);
Frame face_tangent_plane(const Shape &s, double fu, double fv);
Vec3 face_fraction(const Shape &s, Vec3 p);
Box3 face_uv_bounds(const Shape &s);
std::unique_ptr<ShapeList> edge_end_vertices(const Shape &s);
bool edges_tangent_at(const Shape &a, const Shape &b, const Shape &vertex, double angle_tolerance);
bool faces_tangent_along(const Shape &edge, const Shape &a, const Shape &b, double angle_tolerance);
std::unique_ptr<Shape> extended_shape(const Shape &s, double reach);

void write_step(const ShapeList &shapes, const rust::Vec<rust::String> &names, rust::Str path);
std::unique_ptr<ShapeList> read_step(rust::Str path, rust::Vec<rust::String> &names);

}  // namespace occt
