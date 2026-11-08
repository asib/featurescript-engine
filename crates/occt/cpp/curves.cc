// Curves for sketches and wires: ellipses, interpolated and Bézier splines, conics.

#include "occt/cpp/occt.h"
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepLib.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <gp_Ax3.hxx>
#include <gp_Lin2d.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepGProp.hxx>
#include <BRepOffsetAPI_MakeOffset.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Circ.hxx>
#include <gp_Pln.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <ShapeFix_Face.hxx>
#include <TopTools_HSequenceOfShape.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColStd_HArray1OfBoolean.hxx>
#include <TColStd_HArray1OfReal.hxx>
#include <TColgp_Array1OfVec.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TColgp_HArray1OfPnt.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <gp_Elips.hxx>

#include <cmath>
#include <algorithm>
#include <cstdio>
#include <BSplCLib.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_ApproxCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>

#include "occt/src/lib.rs.h"
#include "occt/cpp/internal.h"

namespace occt {

// An ellipse in a frame (x along the major axis), whole or from angle `start` to `end` (radians, from the major
// axis towards the minor). OCCT's ellipses need the major radius first; a "major" smaller than the minor is
// handled by turning the frame.
std::unique_ptr<Shape> make_ellipse_edge(Frame f, double major, double minor, bool whole, double start, double end) {
  gp_Ax2 axes = ax2(f);
  if (major < minor) {
    axes = gp_Ax2(axes.Location(), axes.Direction(), axes.YDirection());
    std::swap(major, minor);
    start -= M_PI / 2;
    end -= M_PI / 2;
  }
  Handle(Geom_Ellipse) ellipse = new Geom_Ellipse(gp_Elips(axes, major, minor));
  if (whole) return wrap(BRepBuilderAPI_MakeEdge(ellipse).Edge());
  BRepBuilderAPI_MakeEdge maker(ellipse, start, end);
  check_done(maker, "elliptical arc");
  return wrap(maker.Edge());
}

// A cubic spline through points with the given derivative at each point and the given parameters: each span is
// the cubic Hermite segment its ends fix, built as a B-spline from the spans' Bézier control points. For a
// `closed` spline, `parameters` has one more value, for the span back to the first point.
std::unique_ptr<Shape> make_interpolated_edge(rust::Slice<const Vec3> points, rust::Slice<const Vec3> derivatives,
                                              rust::Slice<const double> parameters, bool closed) {
  int count = static_cast<int>(points.size());
  if (count < 2) throw std::runtime_error("a spline needs two points");
  if (static_cast<int>(derivatives.size()) != count) throw std::runtime_error("one derivative per point");
  int spans = closed ? count : count - 1;
  if (static_cast<int>(parameters.size()) != spans + 1) throw std::runtime_error("bad parameter count");
  auto point = [&](int i) { return pnt(points[i % count]); };
  auto derivative = [&](int i) {
    const Vec3 &d = derivatives[i % count];
    return gp_Vec(d.x, d.y, d.z);
  };
  TColgp_Array1OfPnt poles(1, 3 * spans + 1);
  for (int i = 0; i < spans; ++i) {
    double h = parameters[i + 1] - parameters[i];
    poles.SetValue(3 * i + 1, point(i));
    poles.SetValue(3 * i + 2, point(i).Translated(derivative(i) * (h / 3)));
    poles.SetValue(3 * i + 3, point(i + 1).Translated(derivative(i + 1) * (-h / 3)));
  }
  poles.SetValue(3 * spans + 1, point(spans));
  TColStd_Array1OfReal knots(1, spans + 1);
  TColStd_Array1OfInteger multiplicities(1, spans + 1);
  for (int i = 0; i <= spans; ++i) {
    knots.SetValue(i + 1, parameters[i]);
    multiplicities.SetValue(i + 1, i == 0 || i == spans ? 4 : 3);
  }
  Handle(Geom_BSplineCurve) curve = new Geom_BSplineCurve(poles, knots, multiplicities, 3);
  // Each span is built on its own; where the spans meet with matching second derivatives (as they do unless a
  // point's derivative was given) the knot is simple, as in Onshape's splines (batch 011, c002).
  const double tolerance = 1e-9 * std::max(1.0, curve->Pole(1).Distance(curve->Pole(curve->NbPoles())));
  for (int i = 2; i < curve->NbKnots(); ++i) {
    if (!curve->RemoveKnot(i, 1, tolerance)) curve->RemoveKnot(i, 2, tolerance);
  }
  return wrap(BRepBuilderAPI_MakeEdge(curve).Edge());
}

// A Bézier curve with these control points.
// An edge on a B-spline given as std's BSplineCurve gives it: poles, weights (none if not rational), and the full
// knot vector with repeated knots; periodic curves list their poles without overlap.
std::unique_ptr<Shape> make_bspline_edge(rust::Slice<const Vec3> poles, rust::Slice<const double> weights,
                                         rust::Slice<const double> knots, int32_t degree, bool periodic) {
  if (poles.size() < 2 || knots.empty()) throw std::invalid_argument("bspline: too few poles or knots");
  TColgp_Array1OfPnt p(1, static_cast<int>(poles.size()));
  for (size_t i = 0; i < poles.size(); ++i) p.SetValue(static_cast<int>(i) + 1, pnt(poles[i]));
  std::vector<double> unique;
  std::vector<int> mults;
  for (double k : knots) {
    if (!unique.empty() && std::abs(k - unique.back()) < 1e-12) {
      ++mults.back();
    } else {
      unique.push_back(k);
      mults.push_back(1);
    }
  }
  if (periodic) {
    // OCCT wants the periodic knots with as many as there are poles, plus one; std gives the full vector.
    int n = static_cast<int>(poles.size());
    int need = n + 1;
    int have = 0;
    for (int m : mults) have += m;
    // Trim the knots std adds at each end to wrap.
    while (have > need && !unique.empty()) {
      int extra = have - need;
      int cut_front = std::min(extra / 2 + extra % 2, mults.front());
      mults.front() -= cut_front;
      have -= cut_front;
      if (mults.front() == 0) {
        unique.erase(unique.begin());
        mults.erase(mults.begin());
      }
      extra = have - need;
      if (extra <= 0) break;
      int cut_back = std::min(extra, mults.back());
      mults.back() -= cut_back;
      have -= cut_back;
      if (mults.back() == 0) {
        unique.pop_back();
        mults.pop_back();
      }
    }
  }
  TColStd_Array1OfReal k(1, static_cast<int>(unique.size()));
  TColStd_Array1OfInteger m(1, static_cast<int>(mults.size()));
  for (size_t i = 0; i < unique.size(); ++i) {
    k.SetValue(static_cast<int>(i) + 1, unique[i]);
    m.SetValue(static_cast<int>(i) + 1, mults[i]);
  }
  Handle(Geom_BSplineCurve) curve;
  if (weights.empty()) {
    curve = new Geom_BSplineCurve(p, k, m, degree, periodic);
  } else {
    TColStd_Array1OfReal w(1, static_cast<int>(weights.size()));
    for (size_t i = 0; i < weights.size(); ++i) w.SetValue(static_cast<int>(i) + 1, weights[i]);
    // OCCT drops weights that are all equal, but a curve std made rational stays rational in Onshape, weights
    // and all (batch 011, c004): nudging the last weight by an ulp keeps it so without changing its shape.
    bool all_equal = true;
    for (int i = w.Lower() + 1; i <= w.Upper(); ++i) all_equal = all_equal && w(i) == w(w.Lower());
    if (all_equal && w.Length() > 1) w.SetValue(w.Upper(), std::nextafter(w(w.Upper()), 2 * w(w.Upper())));
    curve = new Geom_BSplineCurve(p, w, k, m, degree, periodic);
  }
  return wrap(BRepBuilderAPI_MakeEdge(curve).Edge());
}

std::unique_ptr<Shape> make_bezier_edge(rust::Slice<const Vec3> points) {
  TColgp_Array1OfPnt poles(1, static_cast<int>(points.size()));
  for (size_t i = 0; i < points.size(); ++i) poles.SetValue(static_cast<int>(i) + 1, pnt(points[i]));
  Handle(Geom_BezierCurve) curve = new Geom_BezierCurve(poles);
  return wrap(BRepBuilderAPI_MakeEdge(curve).Edge());
}

// A conic from `a` to `b` tangent to the lines through the control point `c`: a rational quadratic with weight
// `w` at `c` (rho = w / (1 + w)).
std::unique_ptr<Shape> make_conic_edge(Vec3 a, Vec3 c, Vec3 b, double w) {
  TColgp_Array1OfPnt poles(1, 3);
  poles.SetValue(1, pnt(a));
  poles.SetValue(2, pnt(c));
  poles.SetValue(3, pnt(b));
  TColStd_Array1OfReal weights(1, 3);
  weights.SetValue(1, 1.0);
  weights.SetValue(2, w);
  weights.SetValue(3, 1.0);
  Handle(Geom_BezierCurve) curve = new Geom_BezierCurve(poles, weights);
  return wrap(BRepBuilderAPI_MakeEdge(curve).Edge());
}

// Joins edges that meet end to end into wires.
std::unique_ptr<ShapeList> connect_edges(const ShapeList &edges) {
  Handle(TopTools_HSequenceOfShape) input = new TopTools_HSequenceOfShape();
  for (const TopoDS_Shape &e : edges.shapes) input->Append(e);
  Handle(TopTools_HSequenceOfShape) wires;
  ShapeAnalysis_FreeBounds::ConnectEdgesToWires(input, 1e-7, Standard_False, wires);
  auto out = std::make_unique<ShapeList>();
  for (int i = 1; i <= wires->Length(); ++i) out->shapes.push_back(wires->Value(i));
  return out;
}

namespace {

// A planar wire offset by `distance` (positive or negative) in its plane, with sharp corners. Returns the wire
// itself for zero.
TopoDS_Wire offset_wire(const TopoDS_Wire &wire, const gp_Pln &plane, double distance, bool open) {
  if (distance == 0) return wire;
  // OCCT's offset crashes shrinking a wire that is one whole circle; its offsets are circles anyway.
  TopExp_Explorer edges(wire, TopAbs_EDGE);
  if (edges.More()) {
    const TopoDS_Edge &edge = TopoDS::Edge(edges.Current());
    edges.Next();
    BRepAdaptor_Curve curve(edge);
    if (!edges.More() && curve.GetType() == GeomAbs_Circle && BRep_Tool::IsClosed(edge)) {
      gp_Circ circle = curve.Circle();
      double radius = circle.Radius() + distance;
      if (radius <= 0) throw std::runtime_error("offset wire: the circle vanishes");
      circle.SetRadius(radius);
      return BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(circle).Edge()).Wire();
    }
  }
  BRepOffsetAPI_MakeOffset offset;
  if (open) {
    offset.Init(GeomAbs_Intersection, Standard_True);
    offset.AddWire(wire);
  } else {
    BRepBuilderAPI_MakeFace face_maker(plane, wire, Standard_True);
    if (!face_maker.IsDone()) throw std::runtime_error("offset wire: no face");
    offset.Init(face_maker.Face(), GeomAbs_Intersection, Standard_False);
  }
  offset.Perform(distance);
  if (!offset.IsDone()) throw std::runtime_error("offset wire failed");
  TopExp_Explorer it(offset.Shape(), TopAbs_WIRE);
  if (!it.More()) throw std::runtime_error("offset wire made no wire");
  return TopoDS::Wire(it.Current());
}

double enclosed_area(const TopoDS_Wire &wire, const gp_Pln &plane) {
  BRepBuilderAPI_MakeFace maker(plane, wire, Standard_True);
  GProp_GProps props;
  BRepGProp::SurfaceProperties(maker.Face(), props);
  return std::abs(props.Mass());
}

// The ends of a wire, in order along it.
void wire_ends(const TopoDS_Wire &wire, gp_Pnt &start, gp_Pnt &end) {
  TopoDS_Vertex first, last;
  TopExp::Vertices(wire, first, last);
  start = BRep_Tool::Pnt(first);
  end = BRep_Tool::Pnt(last);
}

}  // namespace

// The region between a planar wire offset by `o1` on one side and by `o2` on the other, with sharp corners: what a
// thin feature extrudes. The first side is to the right of the wire's first edge (its direction × `normal`) for an
// open wire, and outward for a closed one.
std::unique_ptr<Shape> offset_wire_region(const Shape &wire_shape, Vec3 normal, double o1, double o2) {
  TopoDS_Wire wire = TopoDS::Wire(wire_shape.shape);
  bool open = !wire.Closed() && !BRep_Tool::IsClosed(wire);
  TopoDS_Vertex v0, v1;
  TopExp::Vertices(wire, v0, v1);
  if (!v0.IsNull() && v0.IsSame(v1)) open = false;
  gp_Pnt origin = BRep_Tool::Pnt(v0.IsNull() ? TopExp::FirstVertex(TopoDS::Edge(TopExp_Explorer(wire, TopAbs_EDGE).Current())) : v0);
  gp_Pln plane(origin, dir(normal));
  if (!open) {
    // Outward is the offset that encloses more.
    TopoDS_Wire grown = offset_wire(wire, plane, 1e-4, false);
    double sign = enclosed_area(grown, plane) > enclosed_area(wire, plane) ? 1.0 : -1.0;
    TopoDS_Wire outer = offset_wire(wire, plane, sign * o1, false);
    TopoDS_Wire inner = offset_wire(wire, plane, -sign * o2, false);
    TopoDS_Face outer_face = BRepBuilderAPI_MakeFace(plane, outer, Standard_True).Face();
    TopoDS_Face inner_face = BRepBuilderAPI_MakeFace(plane, inner, Standard_True).Face();
    BRepAlgoAPI_Cut cut(outer_face, inner_face);
    if (!cut.IsDone()) throw std::runtime_error("offset region failed");
    TopExp_Explorer face(cut.Shape(), TopAbs_FACE);
    if (!face.More()) throw std::runtime_error("offset region is empty");
    return wrap(face.Current());
  }
  // Which way OCCT's positive offset goes for this wire: compare with the right of the first edge.
  BRepTools_WireExplorer first_edge(wire);
  BRepAdaptor_Curve curve(first_edge.Current());
  bool reversed = first_edge.Current().Orientation() == TopAbs_REVERSED;
  double u = reversed ? curve.LastParameter() : curve.FirstParameter();
  gp_Pnt p;
  gp_Vec d;
  curve.D1(u, p, d);
  if (reversed) d.Reverse();
  gp_Vec right = d.Crossed(gp_Vec(dir(normal))).Normalized();
  double probe = 1e-4;
  TopoDS_Wire test = offset_wire(wire, plane, probe, true);
  gp_Pnt t0, t1;
  wire_ends(test, t0, t1);
  gp_Pnt near = t0.Distance(p) < t1.Distance(p) ? t0 : t1;
  double sign = gp_Vec(p, near).Dot(right) > 0 ? 1.0 : -1.0;
  TopoDS_Wire a = offset_wire(wire, plane, sign * o1, true);
  TopoDS_Wire b = offset_wire(wire, plane, -sign * o2, true);
  gp_Pnt a0, a1, b0, b1;
  wire_ends(a, a0, a1);
  wire_ends(b, b0, b1);
  // Make the two wires run the same way: a's start nearer b's start than b's end.
  if (a0.Distance(b1) + a1.Distance(b0) < a0.Distance(b0) + a1.Distance(b1)) {
    b.Reverse();
    std::swap(b0, b1);
  }
  // Join the two sides and the end caps by position: the sides' vertices aren't shared.
  Handle(TopTools_HSequenceOfShape) pieces = new TopTools_HSequenceOfShape();
  for (TopExp_Explorer e(a, TopAbs_EDGE); e.More(); e.Next()) pieces->Append(e.Current());
  for (TopExp_Explorer e(b, TopAbs_EDGE); e.More(); e.Next()) pieces->Append(e.Current());
  if (a1.Distance(b1) > 1e-9) pieces->Append(BRepBuilderAPI_MakeEdge(a1, b1).Edge());
  if (b0.Distance(a0) > 1e-9) pieces->Append(BRepBuilderAPI_MakeEdge(b0, a0).Edge());
  Handle(TopTools_HSequenceOfShape) loops;
  ShapeAnalysis_FreeBounds::ConnectEdgesToWires(pieces, 1e-7, Standard_False, loops);
  if (loops->Length() != 1) throw std::runtime_error("offset region: the loop doesn't close");
  BRepBuilderAPI_MakeFace face(plane, TopoDS::Wire(loops->Value(1)), Standard_True);
  if (!face.IsDone()) throw std::runtime_error("offset region face failed");
  ShapeFix_Face fix(face.Face());
  fix.Perform();
  return wrap(fix.Face());
}

// A helix about an axis through `origin` along `direction`, starting at `start` (off the axis) and turning from
// revolution `t0` to `t1`, `pitch` along the axis and `spiral` outwards per revolution, clockwise or not (looking
// along the direction). Exact: a straight line on a cylinder, or on a cone when the radius changes.
std::unique_ptr<Shape> make_helix_edge(Vec3 origin, Vec3 direction, Vec3 start, double t0, double t1, bool clockwise,
                                       double pitch, double spiral) {
  gp_Dir z = dir(direction);
  gp_Pnt o = pnt(origin), s = pnt(start);
  gp_Vec radial(o, s);
  radial -= gp_Vec(z) * radial.Dot(gp_Vec(z));
  double r0 = radial.Magnitude();
  if (r0 < 1e-12) throw std::runtime_error("helix: the start point is on the axis");
  gp_Pnt base = o.Translated(gp_Vec(o, s).Dot(gp_Vec(z)) * gp_Vec(z));
  Handle(Geom_Surface) surface;
  gp_Lin2d line;
  if (std::abs(spiral) < 1e-15) {
    // Clockwise looking along the direction is a left-handed frame about it.
    gp_Ax3 frame(base, clockwise ? z.Reversed() : z, gp_Dir(radial));
    double rise = clockwise ? -pitch : pitch;
    surface = new Geom_CylindricalSurface(frame, r0);
    // u is the angle, v the height: v = rise * u / 2π.
    line = gp_Lin2d(gp_Pnt2d(0, 0), gp_Dir2d(2 * M_PI, rise));
  } else {
    // A cone's radius grows along its axis: r0 + v sin(a), height v cos(a), with tan(a) = |spiral| / pitch. A
    // shrinking helix uses a cone pointing back along the direction and runs down its slant (v decreasing), and
    // turns the other way in u, since that frame's y is reversed.
    bool shrinking = spiral < 0;
    gp_Ax3 frame(base, shrinking ? z.Reversed() : z, gp_Dir(radial));
    double a = std::atan2(std::abs(spiral), std::abs(pitch));
    surface = new Geom_ConicalSurface(frame, a, r0);
    double slant_per_turn = std::hypot(spiral, pitch);
    double du = (clockwise != shrinking) ? -2 * M_PI : 2 * M_PI;
    double dv = shrinking ? -slant_per_turn : slant_per_turn;
    line = gp_Lin2d(gp_Pnt2d(0, 0), gp_Dir2d(du, dv));
  }
  Handle(Geom2d_Line) curve = new Geom2d_Line(line);
  // The line's parameter is its length in (u, v): per turn, |(2π, dv)|.
  double length_per_turn = 2 * M_PI / std::abs(line.Direction().X());
  BRepBuilderAPI_MakeEdge maker(curve, surface, t0 * length_per_turn, t1 * length_per_turn);
  if (!maker.IsDone()) throw std::runtime_error("helix failed");
  TopoDS_Edge edge = maker.Edge();
  BRepLib::BuildCurves3d(edge, 1e-9, GeomAbs_C2, 14, 400);
  return wrap(edge);
}

}  // namespace occt

namespace occt {

// The curve under an edge, over the edge's range, as a B-spline: a B-spline curve as it is (cut to the edge's
// range), conics exactly as rational splines, anything else approximated within `tolerance`. `force_cubic` and
// `force_non_rational` approximate it as a cubic or polynomial spline where it isn't one. The edge's orientation
// is ignored, as everywhere else that measures curves.
BSplineData edge_bspline(const Shape &s, double tolerance, bool force_cubic, bool force_non_rational) {
  if (s.shape.ShapeType() != TopAbs_EDGE) throw std::invalid_argument("not an edge");
  const TopoDS_Edge &edge = TopoDS::Edge(s.shape);
  double first, last;
  TopLoc_Location location;
  Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, location, first, last);
  if (curve.IsNull()) throw std::runtime_error("the edge has no 3D curve");
  if (!location.IsIdentity()) curve = Handle(Geom_Curve)::DownCast(curve->Transformed(location.Transformation()));
  Handle(Geom_BSplineCurve) spline = Handle(Geom_BSplineCurve)::DownCast(curve);
  const bool whole = std::abs(first - curve->FirstParameter()) < 1e-12 && std::abs(last - curve->LastParameter()) < 1e-12;
  if (!spline.IsNull()) {
    spline = Handle(Geom_BSplineCurve)::DownCast(spline->Copy());
    if (!whole) spline->Segment(first, last);
  } else {
    Handle(Geom_TrimmedCurve) trimmed = new Geom_TrimmedCurve(curve, first, last);
    try {
      spline = GeomConvert::CurveToBSplineCurve(trimmed, Convert_TgtThetaOver2);
    } catch (const Standard_Failure &) {
      spline.Nullify();
    }
    if (spline.IsNull()) {
      GeomConvert_ApproxCurve approx(trimmed, tolerance, GeomAbs_C2, 1000, 9);
      if (!approx.HasResult()) throw std::runtime_error("can't approximate the curve as a B-spline");
      spline = approx.Curve();
    }
  }
  if ((force_non_rational && spline->IsRational()) || (force_cubic && spline->Degree() > 3)) {
    GeomConvert_ApproxCurve approx(spline, tolerance, GeomAbs_C2, 1000, 3);
    if (!approx.HasResult()) throw std::runtime_error("can't approximate the curve as a cubic B-spline");
    spline = approx.Curve();
  }
  if (force_cubic && spline->Degree() < 3) spline->IncreaseDegree(3);
  BSplineData out;
  out.degree = spline->Degree();
  out.periodic = spline->IsPeriodic();
  out.rational = spline->IsRational();
  for (int i = 1; i <= spline->NbPoles(); ++i) {
    out.poles.push_back(vec3(spline->Pole(i).XYZ()));
    if (out.rational) out.weights.push_back(spline->Weight(i));
  }
  TColStd_Array1OfReal knots(1, spline->NbKnots());
  TColStd_Array1OfInteger mults(1, spline->NbKnots());
  spline->Knots(knots);
  spline->Multiplicities(mults);
  TColStd_Array1OfReal sequence(1, BSplCLib::KnotSequenceLength(mults, spline->Degree(), spline->IsPeriodic()));
  BSplCLib::KnotSequence(knots, mults, spline->Degree(), spline->IsPeriodic(), sequence);
  for (int i = sequence.Lower(); i <= sequence.Upper(); ++i) out.knots.push_back(sequence(i));
  return out;
}

}  // namespace occt
