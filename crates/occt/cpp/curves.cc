// Curves for sketches and wires: ellipses, interpolated and Bézier splines, conics.

#include "occt/cpp/occt.h"

#include <BRepBuilderAPI_MakeEdge.hxx>
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
  return wrap(BRepBuilderAPI_MakeEdge(curve).Edge());
}

// A Bézier curve with these control points.
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

}  // namespace occt
