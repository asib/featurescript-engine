// Measurements beyond volumes and boxes: axes, points and tangents along edges, distances.

#include "occt/cpp/occt.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepLProp_CLProps.hxx>
#include <gp.hxx>
#include <vector>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <gp_Pnt2d.hxx>
#include <IntCurvesFace_ShapeIntersector.hxx>
#include <algorithm>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepTools.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <gp_Elips.hxx>
#include <gp_Pln.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRep_Tool.hxx>
#include <Extrema_ExtPC.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Lin.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>

#include <cmath>

#include "occt/src/lib.rs.h"
#include "occt/cpp/internal.h"

namespace occt {

namespace {

Axis axis_of(const gp_Ax1 &a) {
  return Axis{vec3(a.Location().XYZ()), vec3(a.Direction().XYZ())};
}

// An edge's curve parameter at fraction `t` (0 to 1) along it, in the edge's own direction: by arc length, or
// linearly in the curve's parameter.
double edge_parameter_at(const TopoDS_Edge &edge, const BRepAdaptor_Curve &curve, double t, bool arc_length) {
  double first = curve.FirstParameter(), last = curve.LastParameter();
  bool reversed = edge.Orientation() == TopAbs_REVERSED;
  if (reversed) t = 1 - t;
  if (!arc_length) return first + t * (last - first);
  double length = GCPnts_AbscissaPoint::Length(curve);
  if (t <= 0) return first;
  if (t >= 1) return last;
  GCPnts_AbscissaPoint point(curve, t * length, first);
  if (!point.IsDone()) throw std::runtime_error("could not find the point along the edge");
  return point.Parameter();
}

}  // namespace

// The axis of an entity that has one: a line, a circle or arc, or a cylinder, cone, sphere, torus or revolved
// face. As in Onshape, a line edge's axis passes through the edge's midpoint and points along the edge.
Axis shape_axis(const Shape &s) {
  if (s.shape.ShapeType() == TopAbs_EDGE) {
    const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
    BRepAdaptor_Curve curve(edge);
    switch (curve.GetType()) {
      case GeomAbs_Line: {
        double u = edge_parameter_at(edge, curve, 0.5, false);
        gp_Pnt p;
        gp_Vec d;
        curve.D1(u, p, d);
        if (edge.Orientation() == TopAbs_REVERSED) d.Reverse();
        return Axis{vec3(p.XYZ()), vec3(gp_Dir(d).XYZ())};
      }
      case GeomAbs_Circle:
        return axis_of(curve.Circle().Axis());
      default:
        throw std::runtime_error("the edge has no axis");
    }
  }
  if (s.shape.ShapeType() == TopAbs_FACE) {
    BRepAdaptor_Surface surface(TopoDS::Face(s.shape));
    switch (surface.GetType()) {
      case GeomAbs_Cylinder: return axis_of(surface.Cylinder().Axis());
      case GeomAbs_Cone: return axis_of(surface.Cone().Axis());
      case GeomAbs_Sphere: return axis_of(surface.Sphere().Position().Axis());
      case GeomAbs_Torus: return axis_of(surface.Torus().Axis());
      case GeomAbs_SurfaceOfRevolution: return axis_of(surface.AxeOfRevolution());
      default: throw std::runtime_error("the face has no axis");
    }
  }
  throw std::runtime_error("only edges and faces have axes");
}

// The point at fraction `t` along an edge and the unit tangent there, in the edge's direction.
Axis edge_tangent(const Shape &s, double t, bool arc_length) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
  BRepAdaptor_Curve curve(edge);
  double u = edge_parameter_at(edge, curve, t, arc_length);
  gp_Pnt p;
  gp_Vec d;
  curve.D1(u, p, d);
  if (d.Magnitude() < 1e-300) throw std::runtime_error("no tangent");
  if (edge.Orientation() == TopAbs_REVERSED) d.Reverse();
  return Axis{vec3(p.XYZ()), vec3(gp_Dir(d).XYZ())};
}

// The Frenet frame of an edge at fraction `t`: origin the point, normal the x direction (pointing to the centre of
// curvature; arbitrary where the curvature is zero), tangent the "normal" (z); and the curvature in `radius1`.
Conic edge_curvature(const Shape &s, double t, bool arc_length) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
  BRepAdaptor_Curve curve(edge);
  double u = edge_parameter_at(edge, curve, t, arc_length);
  BRepLProp_CLProps props(curve, u, 2, 1e-9);
  if (!props.IsTangentDefined()) throw std::runtime_error("no tangent");
  gp_Dir tangent;
  props.Tangent(tangent);
  if (edge.Orientation() == TopAbs_REVERSED) tangent.Reverse();
  double k = props.Curvature();
  gp_Dir normal;
  if (k > 1e-12) {
    props.Normal(normal);
  } else {
    // Any direction square to the tangent.
    gp_Dir other = std::abs(tangent.X()) < 0.9 ? gp::DX() : gp::DY();
    normal = gp_Dir(gp_Vec(tangent).Crossed(gp_Vec(other)));
  }
  return Conic{0, Frame{vec3(props.Value().XYZ()), vec3(tangent.XYZ()), vec3(normal.XYZ())}, k, 0};
}

// The derivative of an edge's curvature vector with respect to arc length (its third derivative by arc length) at
// fraction `t`, in the edge's direction.
Vec3 edge_curvature_derivative(const Shape &s, double t, bool arc_length) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
  BRepAdaptor_Curve curve(edge);
  double u = edge_parameter_at(edge, curve, t, arc_length);
  gp_Pnt p;
  gp_Vec d1, d2, d3;
  curve.D3(u, p, d1, d2, d3);
  double v = d1.Magnitude();
  if (v < 1e-300) throw std::runtime_error("no tangent");
  double a = d1.Dot(d2), b = d2.Dot(d2) + d1.Dot(d3);
  gp_Vec k = d3 / std::pow(v, 3) - d2 * (3 * a / std::pow(v, 5)) - d1 * (b / std::pow(v, 5)) +
             d1 * (4 * a * a / std::pow(v, 7));
  // Walking the other way flips the third derivative.
  if (edge.Orientation() == TopAbs_REVERSED) k.Reverse();
  return vec3(k.XYZ());
}

// The fraction (0 to 1) along an edge, in its direction, of the point on it closest to `p`.
double edge_fraction(const Shape &s, Vec3 p, bool arc_length) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
  BRepAdaptor_Curve curve(edge);
  double first = curve.FirstParameter(), last = curve.LastParameter();
  Extrema_ExtPC extrema(pnt(p), curve);
  if (!extrema.IsDone() || extrema.NbExt() == 0) {
    // Closest at an end.
    double d0 = curve.Value(first).Distance(pnt(p)), d1 = curve.Value(last).Distance(pnt(p));
    double t = d0 <= d1 ? 0.0 : 1.0;
    return edge.Orientation() == TopAbs_REVERSED ? 1 - t : t;
  }
  int best = 1;
  for (int i = 2; i <= extrema.NbExt(); ++i)
    if (extrema.SquareDistance(i) < extrema.SquareDistance(best)) best = i;
  double u = extrema.Point(best).Parameter();
  double t;
  if (arc_length) {
    double total = GCPnts_AbscissaPoint::Length(curve);
    t = total > 0 ? GCPnts_AbscissaPoint::Length(curve, first, u) / total : 0;
  } else {
    t = (u - first) / (last - first);
  }
  return edge.Orientation() == TopAbs_REVERSED ? 1 - t : t;
}

// The fraction along an edge, extrapolated past its ends along its underlying curve, of the point on that curve
// closest to `p`: for a side of evDistance that was extended. Lines are measured linearly.
double edge_fraction_extended(const Shape &s, Vec3 p) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
  double first, last;
  Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
  if (curve.IsNull()) throw std::runtime_error("the edge has no curve");
  GeomAPI_ProjectPointOnCurve projector(pnt(p), curve);
  if (projector.NbPoints() == 0) throw std::runtime_error("could not project onto the curve");
  double t = (projector.LowerDistanceParameter() - first) / (last - first);
  return edge.Orientation() == TopAbs_REVERSED ? 1 - t : t;
}

// The minimum distance between two shapes and the closest points on each.
// Where the infinite line through `origin` along `direction` (a unit vector) crosses the faces of a shape, as
// distances along it, sorted, with crossings at the same place (on an edge between faces) counted once.
rust::Vec<double> line_crossings(const Shape &s, Vec3 origin, Vec3 direction) {
  IntCurvesFace_ShapeIntersector intersector;
  intersector.Load(s.shape, 1e-9);
  intersector.Perform(gp_Lin(pnt(origin), dir(direction)), -1e6, 1e6);
  std::vector<double> found;
  if (intersector.IsDone())
    for (int i = 1; i <= intersector.NbPnt(); ++i) found.push_back(intersector.WParameter(i));
  std::sort(found.begin(), found.end());
  rust::Vec<double> out;
  for (double w : found)
    if (out.empty() || w - out[out.size() - 1] > 1e-9) out.push_back(w);
  return out;
}

Distance shape_distance(const Shape &a, const Shape &b) {
  BRepExtrema_DistShapeShape extrema(a.shape, b.shape);
  if (!extrema.IsDone() || extrema.NbSolution() == 0) throw std::runtime_error("distance failed");
  return Distance{extrema.Value(), vec3(extrema.PointOnShape1(1).XYZ()), vec3(extrema.PointOnShape2(1).XYZ())};
}

// What kind of curve an edge lies on, and its frame and radii: kind 0 a line (origin at the edge's start,
// `normal` its direction), 1 a circle (`radius1`), 2 an ellipse (`radius1` major, `radius2` minor, `x` towards
// the major axis), 3 anything else.
Conic curve_definition(const Shape &s) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
  BRepAdaptor_Curve curve(edge);
  auto frame = [](const gp_Ax2 &a) {
    return Frame{vec3(a.Location().XYZ()), vec3(a.Direction().XYZ()), vec3(a.XDirection().XYZ())};
  };
  switch (curve.GetType()) {
    case GeomAbs_Line: {
      Axis axis = shape_axis(s);
      gp_Dir d = dir(axis.direction);
      gp_Ax2 a(pnt(axis.origin), d);
      return Conic{0, Frame{axis.origin, axis.direction, vec3(a.XDirection().XYZ())}, 0, 0};
    }
    case GeomAbs_Circle: {
      gp_Circ c = curve.Circle();
      return Conic{1, frame(c.Position()), c.Radius(), c.Radius()};
    }
    case GeomAbs_Ellipse: {
      gp_Elips e = curve.Ellipse();
      return Conic{2, frame(e.Position()), e.MajorRadius(), e.MinorRadius()};
    }
    default:
      return Conic{3, Frame{}, 0, 0};
  }
}

namespace {

// A face's parameter (u, v) from fractions of its parameter-space bounding box.
void face_uv_from_fraction(const TopoDS_Face &face, double fu, double fv, double &u, double &v) {
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  u = u0 + fu * (u1 - u0);
  v = v0 + fv * (v1 - v0);
}

}  // namespace

// The plane tangent to a face at fractions (fu, fv) of its parameter-space bounding box: origin on the face,
// normal pointing out of the face (as oriented), x along the u direction.
Frame face_tangent_plane(const Shape &s, double fu, double fv) {
  if (s.shape.ShapeType() != TopAbs_FACE) throw std::runtime_error("not a face");
  const TopoDS_Face &face = TopoDS::Face(s.shape);
  double u, v;
  face_uv_from_fraction(face, fu, fv, u, v);
  BRepAdaptor_Surface surface(face);
  BRepLProp_SLProps props(surface, u, v, 1, 1e-9);
  if (!props.IsNormalDefined()) throw std::runtime_error("no tangent plane");
  gp_Dir normal = props.Normal();
  if (face.Orientation() == TopAbs_REVERSED) normal.Reverse();
  gp_Vec du = props.D1U();
  // Make x exactly perpendicular to the normal.
  gp_Vec x = du - gp_Vec(normal) * du.Dot(gp_Vec(normal));
  if (x.Magnitude() < 1e-300) x = gp_Vec(gp_Ax2(props.Value(), normal).XDirection());
  return Frame{vec3(props.Value().XYZ()), vec3(normal.XYZ()), vec3(gp_Dir(x).XYZ())};
}

// The fractions (u in x, v in y) of a face's parameter-space bounding box at the point on it closest to `p`.
namespace {

// A face's outward normal at a point near it, and its normal curvature there in direction `across` (negative where
// the face bends away from its outward normal, as a convex face does).
bool normal_and_curvature(const TopoDS_Face &face, const gp_Pnt &p, const gp_Dir *across, gp_Dir &normal,
                          double &curvature) {
  GeomAPI_ProjectPointOnSurf projector(p, BRep_Tool::Surface(face));
  if (projector.NbPoints() == 0) return false;
  double u, v;
  projector.LowerDistanceParameters(u, v);
  BRepAdaptor_Surface surface(face, Standard_False);
  BRepLProp_SLProps props(surface, u, v, 2, 1e-9);
  if (!props.IsNormalDefined()) return false;
  normal = props.Normal();
  curvature = 0;
  if (across != nullptr && props.IsCurvatureDefined()) {
    gp_Dir d1, d2;
    props.CurvatureDirections(d1, d2);
    double c1 = across->Dot(d1), c2 = across->Dot(d2);
    curvature = props.MaxCurvature() * c1 * c1 + props.MinCurvature() * c2 * c2;
    if (props.IsUmbilic()) curvature = props.MaxCurvature();
  }
  if (face.Orientation() == TopAbs_REVERSED) {
    normal.Reverse();
    curvature = -curvature;
  }
  return true;
}

}  // namespace

// How a body's two faces meet along an edge: 0 convex, 1 concave, 2 smooth, 3 variable, -1 if the edge isn't
// between two faces. With `by_curvature`, where the faces meet smoothly the way they bend decides: convex if one
// bends convexly and the other is flat or bends the same way, concave the other way round (the edges of fillets).
int32_t edge_convexity(const Shape &body, const Shape &edge, bool by_curvature) {
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndAncestors(body.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  const TopTools_ListOfShape *faces = edge_faces.Seek(edge.shape);
  if (faces == nullptr) return -1;
  std::vector<TopoDS_Face> distinct;
  for (const TopoDS_Shape &f : *faces)
    if (std::none_of(distinct.begin(), distinct.end(), [&](const TopoDS_Face &g) { return g.IsSame(f); }))
      distinct.push_back(TopoDS::Face(f));
  if (distinct.size() != 2) return -1;
  const TopoDS_Face &f1 = distinct[0], &f2 = distinct[1];
  // The edge's direction with f1 on its left.
  bool reversed = false;
  for (TopExp_Explorer it(f1, TopAbs_EDGE); it.More(); it.Next())
    if (it.Current().IsSame(edge.shape)) {
      reversed = it.Current().Orientation() == TopAbs_REVERSED;
      break;
    }
  BRepAdaptor_Curve curve(TopoDS::Edge(edge.shape));
  int seen[3] = {0, 0, 0};
  const int samples = 5;
  for (int i = 0; i < samples; ++i) {
    double t = curve.FirstParameter() + (i + 0.5) / samples * (curve.LastParameter() - curve.FirstParameter());
    gp_Pnt p;
    gp_Vec tangent;
    curve.D1(t, p, tangent);
    if (tangent.Magnitude() < 1e-12) continue;
    if (reversed) tangent.Reverse();
    gp_Dir n1, n2;
    double k1, k2;
    if (!normal_and_curvature(f1, p, nullptr, n1, k1) || !normal_and_curvature(f2, p, nullptr, n2, k2)) continue;
    gp_Vec cross = gp_Vec(n1).Crossed(gp_Vec(n2));
    int kind;
    if (cross.Magnitude() > 1e-6) {
      kind = cross.Dot(tangent) > 0 ? 0 : 1;
    } else if (!by_curvature) {
      kind = 2;
    } else {
      gp_Dir along(tangent);
      gp_Dir across1 = gp_Dir(gp_Vec(n1).Crossed(gp_Vec(along)));
      normal_and_curvature(f1, p, &across1, n1, k1);
      normal_and_curvature(f2, p, &across1, n2, k2);
      const double eps = 1e-9;
      bool convex = k1 < -eps || k2 < -eps, concave = k1 > eps || k2 > eps;
      kind = convex && !concave ? 0 : concave && !convex ? 1 : 2;
    }
    seen[kind] += 1;
  }
  int total = seen[0] + seen[1] + seen[2];
  if (total == 0) return -1;
  for (int k = 0; k < 3; ++k)
    if (seen[k] == total) return k;
  return 3;
}

// A point inside a face, not just inside its parameter box: the middle of the box if that's on the face, else the
// first point of finer and finer grids over the box that is.
Vec3 face_inner_point(const Shape &s) {
  if (s.shape.ShapeType() != TopAbs_FACE) throw std::runtime_error("not a face");
  const TopoDS_Face &face = TopoDS::Face(s.shape);
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  BRepClass_FaceClassifier classifier;
  for (int n : {1, 3, 5, 9, 17, 33})
    for (int i = 0; i < n; ++i)
      for (int j = 0; j < n; ++j) {
        double u = u0 + (i + 0.5) / n * (u1 - u0), v = v0 + (j + 0.5) / n * (v1 - v0);
        classifier.Perform(face, gp_Pnt2d(u, v), 1e-9);
        if (classifier.State() == TopAbs_IN) return vec3(surface->Value(u, v).XYZ());
      }
  throw std::runtime_error("no point inside the face");
}

Vec3 face_fraction(const Shape &s, Vec3 p) {
  if (s.shape.ShapeType() != TopAbs_FACE) throw std::runtime_error("not a face");
  const TopoDS_Face &face = TopoDS::Face(s.shape);
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  GeomAPI_ProjectPointOnSurf projector(pnt(p), BRep_Tool::Surface(face));
  if (projector.NbPoints() == 0) throw std::runtime_error("could not project onto the face");
  double u, v;
  projector.LowerDistanceParameters(u, v);
  return Vec3{u1 > u0 ? (u - u0) / (u1 - u0) : 0, v1 > v0 ? (v - v0) / (v1 - v0) : 0, 0};
}

// An edge or face extended to its underlying curve or surface, as far as practical: lines and planes to `reach`
// either way, closed curves to the whole curve. Other geometry comes back as it is.
std::unique_ptr<Shape> extended_shape(const Shape &s, double reach) {
  if (s.shape.ShapeType() == TopAbs_EDGE) {
    const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
    double first, last;
    Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
    if (curve.IsNull()) return wrap(s.shape);
    BRepAdaptor_Curve adaptor(edge);
    if (adaptor.GetType() == GeomAbs_Line) {
      gp_Lin line = adaptor.Line();
      gp_Pnt a = line.Location().Translated(gp_Vec(line.Direction()) * -reach);
      gp_Pnt b = line.Location().Translated(gp_Vec(line.Direction()) * reach);
      return wrap(BRepBuilderAPI_MakeEdge(a, b).Edge());
    }
    if (curve->IsPeriodic()) return wrap(BRepBuilderAPI_MakeEdge(curve).Edge());
    return wrap(s.shape);
  }
  if (s.shape.ShapeType() == TopAbs_FACE) {
    BRepAdaptor_Surface surface(TopoDS::Face(s.shape));
    if (surface.GetType() == GeomAbs_Plane)
      return wrap(BRepBuilderAPI_MakeFace(surface.Plane(), -reach, reach, -reach, reach).Face());
    return wrap(s.shape);
  }
  return wrap(s.shape);
}

// A face's parameter-space bounding box: u in x, v in y.
Box3 face_uv_bounds(const Shape &s) {
  if (s.shape.ShapeType() != TopAbs_FACE) throw std::runtime_error("not a face");
  double u0, u1, v0, v1;
  BRepTools::UVBounds(TopoDS::Face(s.shape), u0, u1, v0, v1);
  return Box3{Vec3{u0, v0, 0}, Vec3{u1, v1, 0}, false};
}

// An edge's first and last vertices, in its own direction. A closed edge has the same vertex twice.
std::unique_ptr<ShapeList> edge_end_vertices(const Shape &s) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  TopoDS_Vertex first, last;
  TopExp::Vertices(TopoDS::Edge(s.shape.Oriented(TopAbs_FORWARD)), first, last);
  auto out = std::make_unique<ShapeList>();
  if (!first.IsNull()) out->shapes.push_back(first);
  if (!last.IsNull()) out->shapes.push_back(last);
  return out;
}

namespace {

// The unit tangent of an edge at one of its vertices, pointing away from the vertex.
gp_Vec tangent_leaving(const TopoDS_Edge &edge, const TopoDS_Vertex &vertex) {
  BRepAdaptor_Curve curve(edge);
  TopoDS_Vertex first, last;
  TopExp::Vertices(edge, first, last);
  bool at_start = first.IsSame(vertex);
  double u = at_start ? curve.FirstParameter() : curve.LastParameter();
  gp_Pnt p;
  gp_Vec d;
  curve.D1(u, p, d);
  if (d.Magnitude() < 1e-300) return gp_Vec();
  d.Normalize();
  return at_start ? d : -d;
}

}  // namespace

// Whether two edges meeting at `vertex` continue smoothly through it (their tangents are opposite within
// `angle_tolerance` radians).
bool edges_tangent_at(const Shape &a, const Shape &b, const Shape &vertex, double angle_tolerance) {
  const TopoDS_Vertex &v = TopoDS::Vertex(vertex.shape);
  gp_Vec ta = tangent_leaving(TopoDS::Edge(a.shape), v), tb = tangent_leaving(TopoDS::Edge(b.shape), v);
  if (ta.Magnitude() < 0.5 || tb.Magnitude() < 0.5) return false;
  return ta.Angle(-tb) <= angle_tolerance;
}

// Whether a wire continues smoothly through every vertex two of its edges share (their tangents opposite within
// `angle_tolerance` radians there).
bool wire_smooth(const Shape &wire, double angle_tolerance) {
  TopTools_IndexedDataMapOfShapeListOfShape vertex_edges;
  TopExp::MapShapesAndAncestors(wire.shape, TopAbs_VERTEX, TopAbs_EDGE, vertex_edges);
  for (int i = 1; i <= vertex_edges.Extent(); ++i) {
    const TopTools_ListOfShape &edges = vertex_edges(i);
    if (edges.Extent() != 2 || edges.First().IsSame(edges.Last())) continue;
    const TopoDS_Vertex &v = TopoDS::Vertex(vertex_edges.FindKey(i));
    gp_Vec ta = tangent_leaving(TopoDS::Edge(edges.First()), v), tb = tangent_leaving(TopoDS::Edge(edges.Last()), v);
    if (ta.Magnitude() < 0.5 || tb.Magnitude() < 0.5 || ta.Angle(-tb) > angle_tolerance) return false;
  }
  return true;
}

// Whether two faces meet smoothly along an edge: their normals agree within `angle_tolerance` radians at points
// along it.
bool faces_tangent_along(const Shape &edge, const Shape &a, const Shape &b, double angle_tolerance) {
  const TopoDS_Edge &e = TopoDS::Edge(edge.shape);
  BRepAdaptor_Curve curve(e);
  for (int i = 0; i <= 4; ++i) {
    double u = curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * (0.1 + 0.2 * i);
    gp_Pnt p = curve.Value(u);
    gp_Dir normals[2];
    const Shape *faces[2] = {&a, &b};
    for (int k = 0; k < 2; ++k) {
      const TopoDS_Face &face = TopoDS::Face(faces[k]->shape);
      GeomAPI_ProjectPointOnSurf projector(p, BRep_Tool::Surface(face));
      if (projector.NbPoints() == 0) return false;
      double fu, fv;
      projector.LowerDistanceParameters(fu, fv);
      BRepAdaptor_Surface surface(face);
      BRepLProp_SLProps props(surface, fu, fv, 1, 1e-9);
      if (!props.IsNormalDefined()) return false;
      normals[k] = props.Normal();
      if (face.Orientation() == TopAbs_REVERSED) normals[k].Reverse();
    }
    if (normals[0].Angle(normals[1]) > angle_tolerance) return false;
  }
  return true;
}

// What surface a face lies on: kind 0 a plane, 1 a cylinder (`radius1`), 2 a cone (`radius1` its half angle,
// with the frame at its apex), 3 a sphere (`radius1`), 4 a torus (`radius1` major, `radius2` minor), 5 anything else.
Conic surface_definition(const Shape &s) {
  if (s.shape.ShapeType() != TopAbs_FACE) throw std::runtime_error("not a face");
  BRepAdaptor_Surface surface(TopoDS::Face(s.shape));
  auto frame = [](const gp_Ax3 &a) {
    return Frame{vec3(a.Location().XYZ()), vec3(a.Direction().XYZ()), vec3(a.XDirection().XYZ())};
  };
  switch (surface.GetType()) {
    case GeomAbs_Plane: return Conic{0, frame(surface.Plane().Position()), 0, 0};
    case GeomAbs_Cylinder: return Conic{1, frame(surface.Cylinder().Position()), surface.Cylinder().Radius(), 0};
    case GeomAbs_Cone: {
      gp_Cone cone = surface.Cone();
      gp_Ax3 position = cone.Position();
      position.SetLocation(cone.Apex());
      return Conic{2, frame(position), std::abs(cone.SemiAngle()), 0};
    }
    case GeomAbs_Sphere: return Conic{3, frame(surface.Sphere().Position()), surface.Sphere().Radius(), 0};
    case GeomAbs_Torus:
      return Conic{4, frame(surface.Torus().Position()), surface.Torus().MajorRadius(), surface.Torus().MinorRadius()};
    default: return Conic{5, Frame{}, 0, 0};
  }
}

}  // namespace occt
