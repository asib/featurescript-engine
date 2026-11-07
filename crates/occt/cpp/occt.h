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

std::unique_ptr<ShapeList> sketch_regions(const ShapeList &edges, Frame f);
std::unique_ptr<Operation> merge_faces(const ShapeList &faces);
std::unique_ptr<Operation> prism(const Shape &profile, Vec3 v);
std::unique_ptr<Operation> boolean_op(int32_t kind, const ShapeList &objects, const ShapeList &tools, bool unify,
                                      double fuzzy);
std::unique_ptr<Operation> transform_op(const Shape &s, const Transform &t);
std::unique_ptr<Operation> revolve(const Shape &profile, Axis axis, double angle);
std::unique_ptr<Shape> make_sphere(Vec3 center, double radius);
std::unique_ptr<Operation> split_op(const ShapeList &objects, const ShapeList &tools);

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
Conic curve_definition(const Shape &s);
Frame face_tangent_plane(const Shape &s, double fu, double fv);
Vec3 face_fraction(const Shape &s, Vec3 p);
Box3 face_uv_bounds(const Shape &s);
std::unique_ptr<Shape> extended_shape(const Shape &s, double reach);

void write_step(const ShapeList &shapes, const rust::Vec<rust::String> &names, rust::Str path);
std::unique_ptr<ShapeList> read_step(rust::Str path, rust::Vec<rust::String> &names);

}  // namespace occt
