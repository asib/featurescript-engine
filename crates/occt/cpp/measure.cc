// Measurements beyond volumes and boxes: axes, points and tangents along edges, distances.

#include "occt/cpp/occt.h"

#include <BRepAdaptor_Curve.hxx>
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
