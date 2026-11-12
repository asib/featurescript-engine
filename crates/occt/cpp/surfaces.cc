// Surface and curve operations: sections, fills, enclosures, sheet extension, iso curves, projections.

#include <HLRBRep_Algo.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <BRepBndLib.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <HLRBRep_HLRToShape.hxx>
#include <HLRAlgo_Projector.hxx>
#include <IntCurvesFace_ShapeIntersector.hxx>
#include "occt/cpp/occt.h"
#include <BRepFilletAPI_MakeFillet2d.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_CompCurveToBSplineCurve.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <BRepTools_History.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <Precision.hxx>
#include <gp_Pnt2d.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2d_Line.hxx>
#include <gp_Dir2d.hxx>
#include <gp_Vec2d.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2dConvert.hxx>
#include <ElSLib.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_ConicalSurface.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Lin.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <TColgp_HArray1OfPnt2d.hxx>
#include <gp_Cone.hxx>

#include <BOPAlgo_MakerVolume.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <Geom_BSplineCurve.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <Geom_Circle.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepFill_Filling.hxx>
#include <BRepGProp.hxx>
#include <BRepAlgo.hxx>
#include <BRepLib.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <TColgp_HArray1OfPnt.hxx>
#include <BRepLib_FindSurface.hxx>
#include <BRepOffsetAPI_MakeOffset.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_NormalProjection.hxx>
#include <BRepProj_Projection.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom_Plane.hxx>
#include <Geom_Surface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColStd_HArray1OfReal.hxx>
#include <TColStd_HArray1OfInteger.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Shell.hxx>
#include <ShapeFix_Wire.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_HSequenceOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <gp_Pln.hxx>

#include <sstream>

#include "occt/src/lib.rs.h"
#include "occt/cpp/internal.h"

namespace occt {

namespace {

std::unique_ptr<ShapeList> edges_of(const TopoDS_Shape &shape) {
  auto out = std::make_unique<ShapeList>();
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(shape, TopAbs_EDGE, edges);
  for (int i = 1; i <= edges.Extent(); ++i) out->shapes.push_back(edges(i));
  return out;
}

TopoDS_Compound compound_of(const ShapeList &l) {
  BRep_Builder builder;
  TopoDS_Compound c;
  builder.MakeCompound(c);
  for (const TopoDS_Shape &s : l.shapes) builder.Add(c, s);
  return c;
}

}  // namespace

// The curves where two sets of faces meet.
std::unique_ptr<ShapeList> section_edges(const ShapeList &a, const ShapeList &b) {
  BRepAlgoAPI_Section section(compound_of(a), compound_of(b), Standard_False);
  section.ComputePCurveOn1(Standard_True);
  section.Approximation(Standard_True);
  section.Build();
  if (!section.IsDone()) throw std::runtime_error("intersection failed");
  return edges_of(section.Shape());
}

// A face filling a closed loop of edges: planar when the loop is flat, otherwise a smooth filling surface.
std::unique_ptr<Shape> fill_edges(const ShapeList &edges) {
  Handle(TopTools_HSequenceOfShape) input = new TopTools_HSequenceOfShape();
  for (const TopoDS_Shape &e : edges.shapes) input->Append(e);
  Handle(TopTools_HSequenceOfShape) wires;
  ShapeAnalysis_FreeBounds::ConnectEdgesToWires(input, 1e-7, Standard_False, wires);
  if (wires->Length() != 1) throw std::runtime_error("fill: the edges don't make one loop");
  TopoDS_Wire wire = TopoDS::Wire(wires->Value(1));
  BRepLib_FindSurface planar(wire, 1e-7, Standard_True);
  if (planar.Found()) {
    // The surface found is relative to the finder's location.
    gp_Pln plane = Handle(Geom_Plane)::DownCast(planar.Surface())->Pln().Transformed(planar.Location().Transformation());
    BRepBuilderAPI_MakeFace maker(plane, wire, Standard_True);
    if (maker.IsDone()) {
      ShapeFix_Face fix(maker.Face());
      fix.Perform();
      return wrap(fix.Face());
    }
  }
  BRepFill_Filling filling;
  for (TopExp_Explorer it(wire, TopAbs_EDGE); it.More(); it.Next())
    filling.Add(TopoDS::Edge(it.Current()), GeomAbs_C0, Standard_True);
  filling.Build();
  if (!filling.IsDone()) throw std::runtime_error("fill failed");
  return wrap(filling.Face());
}

// Solids for the regions a set of faces and bodies encloses.
std::unique_ptr<ShapeList> enclose_shapes(const ShapeList &shapes) {
  BOPAlgo_MakerVolume maker;
  TopTools_ListOfShape arguments = to_list(shapes);
  maker.SetArguments(arguments);
  maker.SetRunParallel(Standard_False);
  maker.SetIntersect(Standard_True);
  maker.Perform();
  if (maker.HasErrors()) {
    std::ostringstream message;
    maker.DumpErrors(message);
    throw std::runtime_error("enclose: " + message.str());
  }
  auto out = std::make_unique<ShapeList>();
  for (TopExp_Explorer it(maker.Shape(), TopAbs_SOLID); it.More(); it.Next()) out->shapes.push_back(it.Current());
  return out;
}

// Copies of faces, sewn into sheets where they share edges, optionally offset along their normals.
std::unique_ptr<ShapeList> extract_faces(const ShapeList &faces, double offset) {
  BRepBuilderAPI_Sewing sewing(1e-7);
  for (const TopoDS_Shape &f : faces.shapes) sewing.Add(BRepBuilderAPI_Copy(f).Shape());
  sewing.Perform();
  TopoDS_Shape sewn = sewing.SewedShape();
  auto out = std::make_unique<ShapeList>();
  TopTools_ListOfShape pieces;
  for (TopExp_Explorer it(sewn, TopAbs_SHELL); it.More(); it.Next()) pieces.Append(it.Current());
  for (TopExp_Explorer it(sewn, TopAbs_FACE, TopAbs_SHELL); it.More(); it.Next()) pieces.Append(it.Current());
  for (const TopoDS_Shape &piece : pieces) {
    if (offset == 0) {
      out->shapes.push_back(piece);
      continue;
    }
    BRepOffsetAPI_MakeOffsetShape maker;
    maker.PerformByJoin(piece, offset, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Intersection);
    if (!maker.IsDone()) throw std::runtime_error("offset surface failed");
    out->shapes.push_back(maker.Shape());
  }
  return out;
}

// A planar sheet with some of its boundary edges moved outward in its plane by `distance`, the edges around them
// extended to meet (linear extension).
std::unique_ptr<Shape> extend_planar_sheet(const Shape &sheet, const ShapeList &edges, double distance) {
  std::vector<double> distances(edges.shapes.size(), distance);
  return extend_planar_face(sheet, edges, rust::Slice<const double>(distances.data(), distances.size()));
}

// A planar sheet of one face with some straight boundary edges moved outward, each by its own distance (negative
// moves it in); the neighbouring edges run on to meet them.
std::unique_ptr<Shape> extend_planar_face(const Shape &sheet, const ShapeList &edges, rust::Slice<const double> distances) {
  TopExp_Explorer faces(sheet.shape, TopAbs_FACE);
  if (!faces.More()) throw std::runtime_error("extend: no face");
  TopoDS_Face face = TopoDS::Face(faces.Current());
  faces.Next();
  if (faces.More()) throw std::runtime_error("extend: only single planar faces are supported yet");
  BRepAdaptor_Surface surface(face);
  if (surface.GetType() != GeomAbs_Plane) throw std::runtime_error("extend: only planar sheets are supported yet");
  gp_Pln plane = surface.Plane();
  TopoDS_Wire outer = BRepTools::OuterWire(face);
  // Each boundary edge is a line moved outward if selected; the new boundary joins consecutive lines.
  std::vector<std::pair<gp_Pnt, gp_Pnt>> lines;
  gp_Dir normal = plane.Axis().Direction();
  if (face.Orientation() == TopAbs_REVERSED) normal.Reverse();
  auto distance_of = [&](const TopoDS_Shape &edge) -> double {
    for (size_t i = 0; i < edges.shapes.size(); ++i)
      if (edges.shapes[i].IsSame(edge)) return distances[i];
    return 0;
  };
  for (BRepTools_WireExplorer it(outer, face); it.More(); it.Next()) {
    const TopoDS_Edge &edge = it.Current();
    BRepAdaptor_Curve curve(edge);
    if (curve.GetType() != GeomAbs_Line) throw std::runtime_error("extend: only straight edges are supported yet");
    gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge, Standard_True));
    gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge, Standard_True));
    if (const double distance = distance_of(edge); distance != 0) {
      // Outward, for a face on the left of the loop.
      gp_Vec outward = gp_Vec(a, b).Crossed(gp_Vec(normal)).Normalized() * distance;
      a.Translate(outward);
      b.Translate(outward);
    }
    lines.emplace_back(a, b);
  }
  // Corners where consecutive lines meet. Parallel lines apart (a piece of an edge moved, its collinear neighbour
  // held: a partial flange's) are joined by a step square to them; collinear ones meet where they join.
  std::vector<gp_Pnt> corners;
  size_t n = lines.size();
  for (size_t i = 0; i < n; ++i) {
    const auto &p = lines[(i + n - 1) % n];
    const auto &q = lines[i];
    gp_Vec d1(p.first, p.second), d2(q.first, q.second);
    gp_Vec w(p.first, q.first);
    gp_Vec cross = d1.Crossed(d2);
    double denom = cross.SquareMagnitude();
    if (denom < 1e-24 * d1.SquareMagnitude() * d2.SquareMagnitude()) {
      gp_Vec apart(p.second, q.first);
      gp_Vec along = d1.Normalized();
      if ((apart - along * apart.Dot(along)).Magnitude() > 1e-9) corners.push_back(p.second);
      corners.push_back(q.first);
      continue;
    }
    double t = w.Crossed(d2).Dot(cross) / denom;
    corners.push_back(p.first.Translated(d1 * t));
  }
  BRepBuilderAPI_MakeWire wire;
  const size_t m = corners.size();
  for (size_t i = 0; i < m; ++i) {
    if (corners[i].Distance(corners[(i + 1) % m]) < 1e-12) continue;
    wire.Add(BRepBuilderAPI_MakeEdge(corners[i], corners[(i + 1) % m]).Edge());
  }
  BRepBuilderAPI_MakeFace maker(plane, wire.Wire(), Standard_True);
  if (!maker.IsDone()) throw std::runtime_error("extend failed");
  TopoDS_Face result = maker.Face();
  if (face.Orientation() == TopAbs_REVERSED) result.Reverse();
  return wrap(result);
}

// Whether a face's parameter is periodic along u (or v).
bool face_periodic(const Shape &face_shape, bool along_u) {
  Handle(Geom_Surface) surface = BRep_Tool::Surface(TopoDS::Face(face_shape.shape));
  // A trimmed surface (a swept tube's torus) is periodic as its basis is.
  while (auto trimmed = Handle(Geom_RectangularTrimmedSurface)::DownCast(surface)) surface = trimmed->BasisSurface();
  return along_u ? surface->IsUPeriodic() : surface->IsVPeriodic();
}

// An edge on a face's surface through points given as fractions of its parameter ranges: a spline through them in
// its parameter plane, its 3D curve built from that.
static std::unique_ptr<Shape> edge_on_face_impl(const Shape &face, rust::Slice<const double> uv, bool closed);

std::unique_ptr<Shape> edge_on_face(const Shape &face, rust::Slice<const double> uv) {
  return edge_on_face_impl(face, uv, false);
}

// A closed edge on a face's surface: a periodic spline through the points in its parameter plane.
std::unique_ptr<Shape> closed_edge_on_face(const Shape &face, rust::Slice<const double> uv) {
  return edge_on_face_impl(face, uv, true);
}

static std::unique_ptr<Shape> edge_on_face_impl(const Shape &face, rust::Slice<const double> uv, bool closed) {
  if (face.shape.ShapeType() != TopAbs_FACE) throw std::runtime_error("not a face");
  const size_t n = uv.size() / 2;
  if (n < 2) throw std::runtime_error("edge on face: two points at least");
  const TopoDS_Face &f = TopoDS::Face(face.shape);
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  Handle(TColgp_HArray1OfPnt2d) points = new TColgp_HArray1OfPnt2d(1, static_cast<int>(n));
  for (size_t i = 0; i < n; ++i)
    points->SetValue(static_cast<int>(i) + 1, gp_Pnt2d(u0 + uv[2 * i] * (u1 - u0), v0 + uv[2 * i + 1] * (v1 - v0)));
  Geom2dAPI_Interpolate interpolate(points, closed ? Standard_True : Standard_False, 1e-12);
  interpolate.Perform();
  if (!interpolate.IsDone()) throw std::runtime_error("edge on face: interpolation failed");
  TopLoc_Location location;
  Handle(Geom_Surface) surface = BRep_Tool::Surface(f, location);
  BRepBuilderAPI_MakeEdge make(interpolate.Curve(), surface);
  if (!make.IsDone()) throw std::runtime_error("edge on face: couldn't make the edge");
  TopoDS_Edge edge = make.Edge();
  BRepLib::BuildCurves3d(edge);
  if (!location.IsIdentity()) edge = TopoDS::Edge(edge.Moved(location));
  return wrap(edge);
}

// A wire of edges in the order given, their ends joined where they nearly meet.
std::unique_ptr<Shape> ordered_wire(const ShapeList &edges) {
  BRep_Builder builder;
  TopoDS_Wire wire;
  builder.MakeWire(wire);
  for (const TopoDS_Shape &e : edges.shapes) builder.Add(wire, e);
  Handle(ShapeFix_Wire) fix = new ShapeFix_Wire();
  fix->Load(wire);
  fix->SetPrecision(1e-6);
  fix->FixConnected(1e-5);
  fix->FixClosed(1e-5);
  return wrap(fix->Wire());
}

// The piece of an edge from fraction `f0` to fraction `f1` of its parameter range, both counted along the edge's
// own direction and the piece running that way, or the other way if `reverse`. On a closed edge, `f0` past `f1`
// goes round through the end.
std::unique_ptr<Shape> edge_between(const Shape &edge, double f0, double f1, bool reverse) {
  if (edge.shape.ShapeType() != TopAbs_EDGE) throw std::runtime_error("not an edge");
  const TopoDS_Edge &e = TopoDS::Edge(edge.shape);
  double a, b;
  Handle(Geom_Curve) curve = BRep_Tool::Curve(e, a, b);
  if (curve.IsNull()) throw std::runtime_error("no curve");
  const bool reversed = e.Orientation() == TopAbs_REVERSED;
  // A fraction along the edge's direction to the curve's parameter.
  auto at = [&](double f) { return reversed ? b - f * (b - a) : a + f * (b - a); };
  auto piece = [&](double u0, double u1) -> Handle(Geom_BSplineCurve) {
    Handle(Geom_TrimmedCurve) t = new Geom_TrimmedCurve(curve, std::min(u0, u1), std::max(u0, u1));
    Handle(Geom_BSplineCurve) spline = GeomConvert::CurveToBSplineCurve(t);
    if (u0 > u1) spline->Reverse();
    return spline;
  };
  // A piece from the very end or to the very start doesn't wrap.
  if (f0 > 1.0 - 1e-9) f0 = 0.0;
  if (f1 < 1e-9) f1 = 1.0;
  Handle(Geom_BSplineCurve) result;
  if (f0 < f1) {
    result = piece(at(f0), at(f1));
  } else {
    GeomConvert_CompCurveToBSplineCurve join(piece(at(f0), at(1.0)));
    if (!join.Add(piece(at(0.0), at(f1)), 1e-5, Standard_True)) throw std::runtime_error("the edge isn't closed");
    result = join.BSplineCurve();
  }
  if (reverse) result->Reverse();
  BRepBuilderAPI_MakeEdge make(result);
  if (!make.IsDone()) throw std::runtime_error("couldn't make the edge");
  return wrap(make.Edge());
}

// Edges that run end to end, joined into one edge (an approximation where their curves differ), as Parasolid
// makes the curve where a projection or intersection closes on itself.
std::unique_ptr<Shape> join_edges(const Shape &wire) {
  // The edges may only meet within tolerance: make them share vertices first.
  Handle(ShapeFix_Wire) fix = new ShapeFix_Wire();
  fix->Load(TopoDS::Wire(wire.shape));
  fix->SetPrecision(1e-6);
  fix->FixReorder();
  fix->FixConnected(1e-6);
  fix->FixClosed(1e-6);
  TopoDS_Wire fixed = fix->Wire();
  TopoDS_Edge joined;
  try {
    joined = BRepAlgo::ConcatenateWireC0(fixed);
  } catch (const Standard_Failure &) {
    joined.Nullify();
  }
  if (!joined.IsNull()) return wrap(joined);
  // Otherwise fit one spline through points sampled along the edges in order.
  std::vector<gp_Pnt> points;
  for (BRepTools_WireExplorer it(fixed); it.More(); it.Next()) {
    BRepAdaptor_Curve curve(it.Current());
    bool reversed = it.Current().Orientation() == TopAbs_REVERSED;
    const int samples = 60;
    for (int i = 0; i < samples; ++i) {
      double t = static_cast<double>(i) / samples;
      double u = reversed ? curve.LastParameter() - t * (curve.LastParameter() - curve.FirstParameter())
                          : curve.FirstParameter() + t * (curve.LastParameter() - curve.FirstParameter());
      gp_Pnt p = curve.Value(u);
      if (points.empty() || points.back().Distance(p) > 1e-9) points.push_back(p);
    }
  }
  bool closed = fixed.Closed() || BRep_Tool::IsClosed(fixed);
  if (!closed && !points.empty()) {
    TopoDS_Vertex first, last;
    TopExp::Vertices(fixed, first, last);
    if (!last.IsNull()) points.push_back(BRep_Tool::Pnt(last));
  }
  if (points.size() < 3) return wrap(wire.shape);
  Handle(TColgp_HArray1OfPnt) poles = new TColgp_HArray1OfPnt(1, static_cast<int>(points.size()));
  for (size_t i = 0; i < points.size(); ++i) poles->SetValue(static_cast<int>(i) + 1, points[i]);
  GeomAPI_Interpolate interpolate(poles, closed ? Standard_True : Standard_False, 1e-12);
  interpolate.Perform();
  if (!interpolate.IsDone()) return wrap(wire.shape);
  return wrap(BRepBuilderAPI_MakeEdge(interpolate.Curve()).Edge());
}

// The iso curve of a face at fraction `t` of its parameter range in u (or v), trimmed to the face.
std::unique_ptr<ShapeList> iso_curves(const Shape &face_shape, bool along_u, double t) {
  const TopoDS_Face &face = TopoDS::Face(face_shape.shape);
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  Handle(Geom_Curve) curve;
  double first, last;
  if (along_u) {
    // A curve along which u varies: v fixed.
    curve = surface->VIso(v0 + t * (v1 - v0));
    first = u0;
    last = u1;
  } else {
    curve = surface->UIso(u0 + t * (u1 - u0));
    first = v0;
    last = v1;
  }
  TopoDS_Edge edge = BRepBuilderAPI_MakeEdge(curve, first, last).Edge();
  BRepAlgoAPI_Common common(edge, face);
  common.Build();
  if (!common.IsDone()) throw std::runtime_error("iso curve trim failed");
  return edges_of(common.Shape());
}

// Edges projected onto faces, along a direction or (with a zero direction) along the faces' normals.
std::unique_ptr<ShapeList> project_edges(const ShapeList &edges, const ShapeList &faces, Vec3 direction) {
  TopoDS_Compound targets = compound_of(faces);
  auto out = std::make_unique<ShapeList>();
  for (const TopoDS_Shape &e : edges.shapes) {
    gp_Vec d(direction.x, direction.y, direction.z);
    if (d.Magnitude() < 1e-12) {
      // Along the faces' normals: each point of the edge to the nearest point on the faces.
      BRepOffsetAPI_NormalProjection normal(targets);
      normal.Add(e);
      normal.SetParams(1e-7, 1e-7, GeomAbs_C2, 14, 64);
      normal.Build();
      if (!normal.IsDone()) throw std::runtime_error("normal projection failed");
      for (TopExp_Explorer it(normal.Shape(), TopAbs_EDGE); it.More(); it.Next()) out->shapes.push_back(it.Current());
      continue;
    }
    BRepProj_Projection projection(e, targets, gp_Dir(d));
    if (!projection.IsDone()) throw std::runtime_error("projection failed");
    // OCCT projects both ways along the direction; keep only what lies ahead of the source edge.
    for (TopExp_Explorer it(projection.Shape(), TopAbs_EDGE); it.More(); it.Next()) {
      BRepAdaptor_Curve curve(TopoDS::Edge(it.Current()));
      gp_Pnt mid = curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2);
      BRepExtrema_DistShapeShape closest(BRepBuilderAPI_MakeVertex(mid).Vertex(), e);
      if (closest.IsDone() && closest.NbSolution() > 0 &&
          gp_Vec(closest.PointOnShape2(1), mid).Dot(d) < -1e-9)
        continue;
      out->shapes.push_back(it.Current());
    }
  }
  return out;
}

// Sheets that cut faces where edges project onto them along the faces' normals: for each edge and each face, at 128
// points along the edge its nearest point on the face's surface and the normal there; the sheet ruled between curves
// through those points `reach` above and below the surface. Cutting with it splits the face along the projection,
// across its seam too (OCCT's normal projection of a circle onto a cylinder loses the part past the seam), and
// what projects beyond the face cuts nothing.
std::unique_ptr<ShapeList> normal_projection_sheets(const ShapeList &edges, const ShapeList &faces, double reach) {
  auto out = std::make_unique<ShapeList>();
  for (const TopoDS_Shape &e : edges.shapes) {
    BRepAdaptor_Curve curve(TopoDS::Edge(e));
    const bool closed = BRep_Tool::IsClosed(TopoDS::Edge(e)) ||
                        curve.Value(curve.FirstParameter()).Distance(curve.Value(curve.LastParameter())) < 1e-9;
    // Both curves through their points at the same parameters (the edge's own): at their default chord lengths,
    // longer outside a cylinder than inside, the rulings tilted off the normals and a circle's projection round a
    // cylinder came out 6 µm inside it (batch 137 c004). A closed edge's first point again at the end.
    const int n = 128;
    const int count = n + 1;
    for (const TopoDS_Shape &f : faces.shapes) {
      Handle(Geom_Surface) surface = BRep_Tool::Surface(TopoDS::Face(f));
      Handle(TColgp_HArray1OfPnt) above = new TColgp_HArray1OfPnt(1, count), below = new TColgp_HArray1OfPnt(1, count);
      Handle(TColStd_HArray1OfReal) params = new TColStd_HArray1OfReal(1, count);
      bool ok = true;
      for (int i = 0; i < count && ok; ++i) {
        double t = curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * i / n;
        GeomAPI_ProjectPointOnSurf projector(curve.Value(t), surface);
        if (projector.NbPoints() == 0) {
          ok = false;
          break;
        }
        double u, v;
        projector.LowerDistanceParameters(u, v);
        gp_Pnt q;
        gp_Vec du, dv;
        surface->D1(u, v, q, du, dv);
        gp_Vec normal = du.Crossed(dv);
        if (normal.Magnitude() < 1e-15) {
          ok = false;
          break;
        }
        normal.Normalize();
        above->SetValue(i + 1, q.Translated(normal * reach));
        below->SetValue(i + 1, q.Translated(normal * -reach));
        params->SetValue(i + 1, t);
      }
      if (!ok) continue;
      auto through = [&](const Handle(TColgp_HArray1OfPnt) &points) {
        GeomAPI_Interpolate interpolate(points, params, Standard_False, 1e-9);
        interpolate.Perform();
        if (!interpolate.IsDone()) throw std::runtime_error("normal projection: interpolation failed");
        return BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(interpolate.Curve()).Edge()).Wire();
      };
      BRepOffsetAPI_ThruSections sheet(Standard_False, Standard_True, 1e-7);
      sheet.AddWire(through(above));
      sheet.AddWire(through(below));
      sheet.Build();
      if (!sheet.IsDone()) throw std::runtime_error("normal projection: no sheet");
      out->shapes.push_back(sheet.Shape());
    }
  }
  return out;
}

// An edge of a planar face moved `distance` across the face (towards its inside for positive distances).
std::unique_ptr<Shape> offset_edge_on_planar_face(const Shape &edge_shape, const Shape &face_shape, double distance) {
  const TopoDS_Face &face = TopoDS::Face(face_shape.shape);
  BRepAdaptor_Surface surface(face);
  if (surface.GetType() == GeomAbs_Cylinder) {
    // On a cylinder (batch 027): a circle round its axis moves along the axis, a line along it round the axis, each
    // the way into the face, by the distance along the surface.
    const gp_Cylinder cylinder = surface.Cylinder();
    const gp_Ax1 axis = cylinder.Axis();
    BRepAdaptor_Curve curve(TopoDS::Edge(edge_shape.shape));
    const double mid = (curve.FirstParameter() + curve.LastParameter()) / 2;
    const gp_Pnt p = curve.Value(mid);
    auto inside = [&](const gp_Pnt &q) {
      double u, v;
      ElSLib::Parameters(cylinder, q, u, v);
      BRepClass_FaceClassifier classifier(face, gp_Pnt2d(u, v), 1e-9);
      return classifier.State() == TopAbs_IN;
    };
    gp_Trsf move;
    if (curve.GetType() == GeomAbs_Circle && curve.Circle().Axis().Direction().IsParallel(axis.Direction(), 1e-9) &&
        gp_Lin(axis).Distance(curve.Circle().Location()) < 1e-9) {
      gp_Vec along(axis.Direction());
      if (!inside(p.Translated(along * 1e-6))) along.Reverse();
      move.SetTranslation(along * distance);
    } else if (curve.GetType() == GeomAbs_Line && curve.Line().Direction().IsParallel(axis.Direction(), 1e-9)) {
      double angle = distance / cylinder.Radius();
      gp_Trsf probe;
      probe.SetRotation(axis, 1e-6 / cylinder.Radius());
      if (!inside(p.Transformed(probe))) angle = -angle;
      move.SetRotation(axis, angle);
    } else {
      throw std::runtime_error("offset curve: only circles round a cylinder's axis and lines along it are supported yet");
    }
    return wrap(BRepBuilderAPI_Transform(edge_shape.shape, move, Standard_True).Shape());
  }
  if (surface.GetType() != GeomAbs_Plane) throw std::runtime_error("offset curve: only planar and cylindrical faces are supported yet");
  gp_Dir normal = surface.Plane().Axis().Direction();
  if (face.Orientation() == TopAbs_REVERSED) normal.Reverse();
  // The edge as the face uses it: the face is on its left.
  TopoDS_Edge edge = TopoDS::Edge(edge_shape.shape);
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next())
    if (it.Current().IsSame(edge)) edge = TopoDS::Edge(it.Current());
  BRepAdaptor_Curve curve(edge);
  if (curve.GetType() == GeomAbs_Circle) {
    // A circle (or arc) moves towards the face's inside: concentric, its radius shrinking or growing.
    gp_Circ circle = curve.Circle();
    double mid = (curve.FirstParameter() + curve.LastParameter()) / 2;
    gp_Pnt p = curve.Value(mid);
    gp_Vec toward_centre(p, circle.Location());
    toward_centre.Normalize();
    BRepClass_FaceClassifier classifier(face, p.Translated(toward_centre * 1e-6), 1e-9);
    bool inside_is_centre = classifier.State() == TopAbs_IN;
    double radius = circle.Radius() + (inside_is_centre ? -distance : distance);
    if (radius <= 1e-12) throw std::runtime_error("offset curve: the circle shrinks away");
    circle.SetRadius(radius);
    Handle(Geom_Circle) c = new Geom_Circle(circle);
    return wrap(BRepBuilderAPI_MakeEdge(c, curve.FirstParameter(), curve.LastParameter()).Edge());
  }
  if (curve.GetType() != GeomAbs_Line) throw std::runtime_error("offset curve: only straight and circular edges are supported yet");
  gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(edge, Standard_True));
  gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(edge, Standard_True));
  gp_Vec inward = gp_Vec(normal).Crossed(gp_Vec(a, b)).Normalized() * distance;
  return wrap(BRepBuilderAPI_MakeEdge(a.Translated(inward), b.Translated(inward)).Edge());
}

// Wraps planar faces onto a cone face without stretching: the source frame (anchor, direction, normal) on the plane
// goes to the destination frame on the cone (anchor, direction tangent there, and the face's normal), and the plane
// rolls onto the cone about its apex. The cone develops into a sector of the plane round the apex, so a point at
// distance ρ from the apex and angle φ from the anchor's generator lands ρ from the apex, φ / sin α round the axis.
// That isn't affine, so each edge is sampled and its curve on the cone fitted through the images (a generator or a
// circle round the axis exactly).
static std::unique_ptr<Operation> wrap_onto_cone(const ShapeList &faces, Vec3 source_anchor, Vec3 source_direction,
                                                 Vec3 source_normal, const TopoDS_Face &dest,
                                                 Handle(Geom_ConicalSurface) cone, Vec3 destination_anchor,
                                                 Vec3 destination_direction) {
  const gp_Cone c = cone->Cone();
  const double alpha = c.SemiAngle(), sin_a = std::abs(std::sin(alpha)), ref = c.RefRadius();
  const gp_Pnt apex = c.Apex();
  double ua, va;
  ElSLib::Parameters(c, pnt(destination_anchor), ua, va);
  const gp_Pnt anchor = ElSLib::Value(ua, va, c);
  gp_Vec du, dv;
  gp_Pnt at;
  cone->D1(ua, va, at, du, dv);
  const gp_Vec t_a = du.Normalized();                         // round the axis, the way u grows
  const gp_Vec g_a = gp_Vec(apex, anchor).Normalized();      // along the generator, away from the apex
  const double rho_a = apex.Distance(anchor);
  gp_Vec n_dest = du.Crossed(dv).Normalized();
  if (dest.Orientation() == TopAbs_REVERSED) n_dest.Reverse();
  const gp_Vec d = gp_Vec(dir(destination_direction));
  const gp_Vec d_perp = n_dest.Crossed(d);
  const gp_Vec ds = gp_Vec(dir(source_direction)), ns = gp_Vec(dir(source_normal));
  const gp_Vec ds_perp = ns.Crossed(ds);
  const gp_Pnt as = pnt(source_anchor);
  // A point of the source plane to the cone's (u, v).
  auto map = [&](const gp_Pnt &p) {
    const gp_Vec rel(as, p);
    const double a = rel.Dot(ds), b = rel.Dot(ds_perp);
    const gp_Vec q = d * a + d_perp * b;
    const double x = q.Dot(t_a), y = q.Dot(g_a) + rho_a;
    const double rho = std::sqrt(x * x + y * y), phi = std::atan2(x, y);
    return gp_Pnt2d(ua + phi / sin_a, -ref / std::sin(alpha) + (alpha > 0 ? rho : -rho));
  };
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History;
  BRep_Builder builder;
  TopoDS_Compound result;
  builder.MakeCompound(result);
  const int samples = 64;
  for (const TopoDS_Shape &shape : faces.shapes) {
    std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> edge_images;
    const TopoDS_Face &face = TopoDS::Face(shape);
    Handle(Geom_Plane) plane = Handle(Geom_Plane)::DownCast(BRep_Tool::Surface(face));
    if (plane.IsNull()) throw std::runtime_error("wrap: the source faces must be planar");
    BRepBuilderAPI_MakeFace maker(cone, Precision::Confusion());
    bool first = true;
    for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next()) {
      BRepBuilderAPI_MakeWire wire;
      for (BRepTools_WireExplorer e(TopoDS::Wire(w.Current()), face); e.More(); e.Next()) {
        BRepAdaptor_Curve curve(e.Current());
        const bool reversed = e.Current().Orientation() == TopAbs_REVERSED;
        Handle(TColgp_HArray1OfPnt2d) points = new TColgp_HArray1OfPnt2d(1, samples + 1);
        for (int i = 0; i <= samples; ++i) {
          double t = static_cast<double>(i) / samples;
          if (reversed) t = 1 - t;
          points->SetValue(i + 1, map(curve.Value(curve.FirstParameter() + t * (curve.LastParameter() - curve.FirstParameter()))));
        }
        const gp_Pnt2d a = points->Value(1), b = points->Value(samples + 1);
        bool same_u = true, same_v = true;
        for (int i = 1; i <= samples + 1; ++i) {
          same_u = same_u && std::abs(points->Value(i).X() - a.X()) < 1e-12;
          same_v = same_v && std::abs(points->Value(i).Y() - a.Y()) < 1e-12;
        }
        TopoDS_Edge edge;
        if (same_u || same_v) {
          // A generator or a circle round the axis: exactly the cone's line or circle there.
          Handle(Geom_Curve) iso = same_u ? cone->UIso(a.X()) : cone->VIso(a.Y());
          Handle(Geom2d_Line) pcurve = same_u ? new Geom2d_Line(gp_Pnt2d(a.X(), 0), gp_Dir2d(0, 1))
                                              : new Geom2d_Line(gp_Pnt2d(0, a.Y()), gp_Dir2d(1, 0));
          const double p1 = same_u ? a.Y() : a.X(), p2 = same_u ? b.Y() : b.X();
          edge = BRepBuilderAPI_MakeEdge(iso, std::min(p1, p2), std::max(p1, p2)).Edge();
          BRep_Builder().UpdateEdge(edge, pcurve, cone, TopLoc_Location(), Precision::Confusion());
          if (p2 < p1) edge.Reverse();
        } else {
          Geom2dAPI_Interpolate fit(points, Standard_False, 1e-12);
          fit.Perform();
          if (!fit.IsDone()) throw std::runtime_error("wrap: an edge's image couldn't be fitted");
          Handle(Geom2d_BSplineCurve) spline = fit.Curve();
          edge = BRepBuilderAPI_MakeEdge(spline, cone, spline->FirstParameter(), spline->LastParameter()).Edge();
          BRepLib::BuildCurves3d(edge, 1e-7, GeomAbs_C1, 14, 200);
        }
        wire.Add(edge);
        edge_images.emplace_back(e.Current(), wire.Edge());
      }
      if (!wire.IsDone()) throw std::runtime_error("wrap: a wire couldn't be rebuilt");
      if (first) {
        maker = BRepBuilderAPI_MakeFace(cone, wire.Wire(), Standard_True);
        first = false;
      } else {
        maker.Add(wire.Wire());
      }
    }
    if (!maker.IsDone()) throw std::runtime_error("wrap: the face couldn't be rebuilt");
    ShapeFix_Face fix(maker.Face());
    fix.SetContext(new ShapeBuild_ReShape);
    fix.Perform();
    TopoDS_Face built = fix.Face();
    if (dest.Orientation() == TopAbs_REVERSED) built.Reverse();
    op->history->AddModified(face, built);
    TopTools_IndexedMapOfShape built_edges;
    TopExp::MapShapes(built, TopAbs_EDGE, built_edges);
    for (const auto &[source, image] : edge_images) {
      const TopoDS_Shape fixed = fix.Context()->Value(image);
      for (TopExp_Explorer x(fixed, TopAbs_EDGE); x.More(); x.Next())
        if (built_edges.Contains(x.Current())) op->history->AddModified(source, x.Current());
    }
    builder.Add(result, built);
  }
  op->result = result;
  return op;
}

// Wraps planar faces onto a cylinder face without stretching (or with distances round it taken at `arc_radius`). The source frame (anchor, direction, normal) on the
// plane goes to the destination frame on the cylinder (anchor, direction tangent there, and the face's normal), and
// distances along the plane become distances along the cylinder: the map from the plane's parameters to the
// cylinder's (angle, height) is affine, so each edge's curve on the plane, as a B-spline, maps exactly by its poles.
// Faces of a cylinder unwrapped onto a plane without stretching (batch 046): the inverse of wrap_onto_cylinder's
// mapping, the source frame (anchor and direction on the cylinder, and the face's normal) going to the destination
// frame (anchor, direction and normal of the plane). A face all the way round fails, as in Onshape.
std::unique_ptr<Operation> unwrap_from_cylinder(const ShapeList &faces, const Shape &source, Vec3 source_anchor,
                                                Vec3 source_direction, Vec3 destination_anchor,
                                                Vec3 destination_direction, Vec3 destination_normal) {
  const TopoDS_Face &src = TopoDS::Face(source.shape);
  Handle(Geom_CylindricalSurface) cylinder = Handle(Geom_CylindricalSurface)::DownCast(BRep_Tool::Surface(src));
  if (cylinder.IsNull()) throw std::runtime_error("unwrap: the source isn't a cylinder");
  const double r = cylinder->Radius();
  const gp_Ax3 position = cylinder->Position();
  double ta, za;
  ElSLib::Parameters(cylinder->Cylinder(), pnt(source_anchor), ta, za);
  gp_Dir axis = position.Direction();
  gp_Vec radial = gp_Vec(position.XDirection()) * std::cos(ta) + gp_Vec(position.YDirection()) * std::sin(ta);
  gp_Vec around = gp_Vec(axis).Crossed(radial);
  if (!position.Direct()) around.Reverse();
  // The cylinder's outward normal, whichever way the face is turned (a sheet's faces inward, batch 046).
  gp_Vec n_src = radial;
  gp_Vec d = gp_Vec(dir(source_direction));
  gp_Vec d_perp = n_src.Crossed(d);
  const double dt = d.Dot(around), dz = d.Dot(gp_Vec(axis)), pt = d_perp.Dot(around), pz = d_perp.Dot(gp_Vec(axis));
  const double det = dt * pz - pt * dz;
  if (std::abs(det) < 1e-12) throw std::runtime_error("unwrap: the anchor direction runs along the normal");
  gp_Ax3 frame(pnt(destination_anchor), dir(destination_normal), dir(destination_direction));
  Handle(Geom_Plane) plane = new Geom_Plane(frame);
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History;
  BRep_Builder builder;
  TopoDS_Compound result;
  builder.MakeCompound(result);
  for (const TopoDS_Shape &shape : faces.shapes) {
    const TopoDS_Face &face = TopoDS::Face(shape);
    Handle(Geom_CylindricalSurface) own = Handle(Geom_CylindricalSurface)::DownCast(BRep_Tool::Surface(face));
    if (own.IsNull() || !own->Position().IsCoplanar(position, 1e-9, 1e-9) ||
        !own->Position().XDirection().IsEqual(position.XDirection(), 1e-9) || std::abs(own->Radius() - r) > 1e-9)
      throw std::runtime_error("unwrap: the faces must lie on the source cylinder");
    double u1, u2, v1, v2;
    BRepTools::UVBounds(face, u1, u2, v1, v2);
    if (u2 - u1 > 2 * M_PI - 1e-9) throw std::runtime_error("unwrap: a face all the way round");
    // The face's angles taken within a turn of the anchor's, on the side the face is.
    double shift = 0;
    while ((u1 + u2) / 2 + shift - ta > M_PI) shift -= 2 * M_PI;
    while ((u1 + u2) / 2 + shift - ta < -M_PI) shift += 2 * M_PI;
    auto map = [&](const gp_Pnt2d &q) {
      const double s = (q.X() + shift - ta) * r, z = q.Y() - za;
      return gp_Pnt2d((pz * s - pt * z) / det, (-dz * s + dt * z) / det);
    };
    TopoDS_Face built;
    bool first = true;
    for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next()) {
      BRepBuilderAPI_MakeWire wire;
      for (BRepTools_WireExplorer e(TopoDS::Wire(w.Current()), face); e.More(); e.Next()) {
        double f, l;
        Handle(Geom2d_Curve) pcurve = BRep_Tool::CurveOnSurface(e.Current(), face, f, l);
        if (pcurve.IsNull()) throw std::runtime_error("unwrap: an edge has no curve on its face");
        Handle(Geom2d_BSplineCurve) spline = Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(pcurve, f, l));
        for (int i = 1; i <= spline->NbPoles(); ++i) spline->SetPole(i, map(spline->Pole(i)));
        TopoDS_Edge edge;
        if (spline->Degree() == 1 && spline->NbPoles() == 2) {
          gp_Pnt a = plane->Value(spline->Pole(1).X(), spline->Pole(1).Y());
          gp_Pnt b = plane->Value(spline->Pole(2).X(), spline->Pole(2).Y());
          edge = BRepBuilderAPI_MakeEdge(a, b).Edge();
        } else {
          edge = BRepBuilderAPI_MakeEdge(spline, plane).Edge();
          BRepLib::BuildCurves3d(edge);
        }
        if (e.Current().Orientation() == TopAbs_REVERSED) edge.Reverse();
        wire.Add(edge);
      }
      if (!wire.IsDone()) throw std::runtime_error("unwrap: a loop didn't close");
      if (first) {
        BRepBuilderAPI_MakeFace maker(plane, wire.Wire(), Standard_True);
        if (!maker.IsDone()) throw std::runtime_error("unwrap: no face");
        built = maker.Face();
        first = false;
      } else {
        BRepBuilderAPI_MakeFace maker(built, wire.Wire());
        built = maker.Face();
      }
    }
    ShapeFix_Face fix(built);
    fix.FixOrientation();
    fix.Perform();
    builder.Add(result, fix.Face());
  }
  op->result = result;
  return op;
}

std::unique_ptr<Operation> wrap_onto_cylinder(const ShapeList &faces, Vec3 source_anchor, Vec3 source_direction,
                                              Vec3 source_normal, const Shape &destination, Vec3 destination_anchor,
                                              Vec3 destination_direction, double arc_radius) {
  const TopoDS_Face &dest = TopoDS::Face(destination.shape);
  Handle(Geom_Surface) dest_surface = BRep_Tool::Surface(dest);
  Handle(Geom_CylindricalSurface) cylinder = Handle(Geom_CylindricalSurface)::DownCast(dest_surface);
  Handle(Geom_ConicalSurface) cone = Handle(Geom_ConicalSurface)::DownCast(dest_surface);
  if (!cone.IsNull())
    return wrap_onto_cone(faces, source_anchor, source_direction, source_normal, dest, cone, destination_anchor,
                          destination_direction);
  if (cylinder.IsNull()) throw std::runtime_error("wrap: the destination isn't a cylinder or a cone");
  const double r = cylinder->Radius();
  const gp_Ax3 position = cylinder->Position();
  double ta, za;
  ElSLib::Parameters(cylinder->Cylinder(), pnt(destination_anchor), ta, za);
  gp_Dir axis = position.Direction();
  gp_Vec radial = gp_Vec(position.XDirection()) * std::cos(ta) + gp_Vec(position.YDirection()) * std::sin(ta);
  gp_Vec around = gp_Vec(axis).Crossed(radial);  // direction of increasing angle (for a direct frame)
  if (!position.Direct()) around.Reverse();
  // The destination face's normal at the anchor.
  gp_Vec n_dest = radial;
  if (dest.Orientation() == TopAbs_REVERSED) n_dest.Reverse();
  gp_Vec d = gp_Vec(dir(destination_direction));
  gp_Vec d_perp = n_dest.Crossed(d);
  // Source frame.
  gp_Vec ds = gp_Vec(dir(source_direction));
  gp_Vec ns = gp_Vec(dir(source_normal));
  gp_Vec ds_perp = ns.Crossed(ds);
  gp_Pnt as = pnt(source_anchor);
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History;
  BRep_Builder builder;
  TopoDS_Compound result;
  builder.MakeCompound(result);
  for (const TopoDS_Shape &shape : faces.shapes) {
    std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> edge_images;
    const TopoDS_Face &face = TopoDS::Face(shape);
    Handle(Geom_Plane) plane = Handle(Geom_Plane)::DownCast(BRep_Tool::Surface(face));
    if (plane.IsNull()) throw std::runtime_error("wrap: the source faces must be planar");
    const gp_Ax3 &pp = plane->Position();
    gp_Vec o(as, pp.Location());
    gp_Vec xp(pp.XDirection()), yp(pp.YDirection());
    // (u, v) in the source frame from the plane's (pu, pv): u = o.ds + pu xp.ds + pv yp.ds, likewise v.
    double u0 = o.Dot(ds), uu = xp.Dot(ds), uv = yp.Dot(ds);
    double v0 = o.Dot(ds_perp), vu = xp.Dot(ds_perp), vv = yp.Dot(ds_perp);
    // (s, z) on the unrolled cylinder from (u, v), then angle = s / r.
    double dt = d.Dot(around), dz = d.Dot(gp_Vec(axis)), pt = d_perp.Dot(around), pz = d_perp.Dot(gp_Vec(axis));
    auto map = [&](const gp_Pnt2d &q) {
      double u = u0 + q.X() * uu + q.Y() * uv, v = v0 + q.X() * vu + q.Y() * vv;
      double s = u * dt + v * pt, z = u * dz + v * pz;
      // Distances round the cylinder are measured at `arc_radius` if one is given (a sheet metal bend's allowance).
      return gp_Pnt2d(ta + s / (arc_radius > 0 ? arc_radius : r), za + z);
    };
    BRepBuilderAPI_MakeFace maker(cylinder, Precision::Confusion());
    bool first = true;
    TopoDS_Face built;
    for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next()) {
      BRepBuilderAPI_MakeWire wire;
      for (BRepTools_WireExplorer e(TopoDS::Wire(w.Current()), face); e.More(); e.Next()) {
        double f, l;
        Handle(Geom2d_Curve) pcurve = BRep_Tool::CurveOnSurface(e.Current(), face, f, l);
        if (pcurve.IsNull()) throw std::runtime_error("wrap: an edge has no curve on its face");
        Handle(Geom2d_BSplineCurve) spline = Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(pcurve, f, l));
        for (int i = 1; i <= spline->NbPoles(); ++i) spline->SetPole(i, map(spline->Pole(i)));
        TopoDS_Edge edge;
        const gp_Pnt2d a = spline->Pole(1), b = spline->Pole(spline->NbPoles());
        const bool straight = spline->Degree() == 1 && spline->NbPoles() == 2;
        if (straight && (std::abs(a.X() - b.X()) < 1e-12 || std::abs(a.Y() - b.Y()) < 1e-12)) {
          // An edge along the axis or round it: exactly the cylinder's line or circle there, not a B-spline.
          const bool along = std::abs(a.X() - b.X()) < 1e-12;
          Handle(Geom_Curve) curve = along ? cylinder->UIso(a.X()) : cylinder->VIso(a.Y());
          Handle(Geom2d_Line) pcurve = along ? new Geom2d_Line(gp_Pnt2d(a.X(), 0), gp_Dir2d(0, 1))
                                             : new Geom2d_Line(gp_Pnt2d(0, a.Y()), gp_Dir2d(1, 0));
          const double p1 = along ? a.Y() : a.X(), p2 = along ? b.Y() : b.X();
          edge = BRepBuilderAPI_MakeEdge(curve, std::min(p1, p2), std::max(p1, p2)).Edge();
          BRep_Builder().UpdateEdge(edge, pcurve, cylinder, TopLoc_Location(), Precision::Confusion());
          if (p2 < p1) edge.Reverse();
        } else {
          edge = BRepBuilderAPI_MakeEdge(spline, cylinder, spline->FirstParameter(), spline->LastParameter()).Edge();
          // Tightly: the default 1e-5 leaves vertices that tolerant, and a sheet metal closed corner's bends end
          // only 2e-5 apart.
          BRepLib::BuildCurves3d(edge, 1e-7, GeomAbs_C1, 14, 200);
        }
        wire.Add(edge);
        edge_images.emplace_back(e.Current(), wire.Edge());
      }
      if (!wire.IsDone()) throw std::runtime_error("wrap: a wire couldn't be rebuilt");
      if (first) {
        maker = BRepBuilderAPI_MakeFace(cylinder, wire.Wire(), Standard_True);
        first = false;
      } else {
        maker.Add(wire.Wire());
      }
    }
    if (!maker.IsDone()) throw std::runtime_error("wrap: the face couldn't be rebuilt");
    ShapeFix_Face fix(maker.Face());
    fix.SetContext(new ShapeBuild_ReShape);
    fix.Perform();
    built = fix.Face();
    // The result's normal points the way the destination's does.
    if (dest.Orientation() == TopAbs_REVERSED) built.Reverse();
    // Each face and edge is the image of the one it was wrapped from.
    op->history->AddModified(face, built);
    TopTools_IndexedMapOfShape built_edges;
    TopExp::MapShapes(built, TopAbs_EDGE, built_edges);
    for (const auto &[source, image] : edge_images) {
      const TopoDS_Shape fixed = fix.Context()->Value(image);
      for (TopExp_Explorer x(fixed, TopAbs_EDGE); x.More(); x.Next())
        if (built_edges.Contains(x.Current())) op->history->AddModified(source, x.Current());
    }
    builder.Add(result, built);
  }
  op->result = result;
  return op;
}

// A face on a non-rational B-spline surface over its whole domain: `poles` row by row (`nu` rows of `nv`), and
// full knot vectors (knots repeated by their multiplicity) in each direction.
std::unique_ptr<Shape> make_bspline_surface_face(rust::Slice<const Vec3> poles, int32_t nu, int32_t nv,
                                                 rust::Slice<const double> knots_u, rust::Slice<const double> knots_v,
                                                 int32_t degree_u, int32_t degree_v) {
  if (nu < 2 || nv < 2 || static_cast<int>(poles.size()) != nu * nv) throw std::invalid_argument("bad pole grid");
  TColgp_Array2OfPnt grid(1, nu, 1, nv);
  for (int i = 0; i < nu; ++i)
    for (int j = 0; j < nv; ++j) grid.SetValue(i + 1, j + 1, pnt(poles[i * nv + j]));
  auto split = [](rust::Slice<const double> full, Handle(TColStd_HArray1OfReal) &knots,
                  Handle(TColStd_HArray1OfInteger) &mults) {
    std::vector<double> unique;
    std::vector<int> counts;
    for (double k : full) {
      if (!unique.empty() && std::abs(k - unique.back()) < 1e-12) {
        ++counts.back();
      } else {
        unique.push_back(k);
        counts.push_back(1);
      }
    }
    knots = new TColStd_HArray1OfReal(1, static_cast<int>(unique.size()));
    mults = new TColStd_HArray1OfInteger(1, static_cast<int>(unique.size()));
    for (size_t i = 0; i < unique.size(); ++i) {
      knots->SetValue(static_cast<int>(i) + 1, unique[i]);
      mults->SetValue(static_cast<int>(i) + 1, counts[i]);
    }
  };
  Handle(TColStd_HArray1OfReal) ku, kv;
  Handle(TColStd_HArray1OfInteger) mu, mv;
  split(knots_u, ku, mu);
  split(knots_v, kv, mv);
  Handle(Geom_BSplineSurface) surface =
      new Geom_BSplineSurface(grid, ku->Array1(), kv->Array1(), mu->Array1(), mv->Array1(), degree_u, degree_v);
  return wrap(BRepBuilderAPI_MakeFace(surface, Precision::Confusion()).Face());
}

// The same, rational: a weight for each pole.
std::unique_ptr<Shape> make_rational_bspline_surface_face(rust::Slice<const Vec3> poles, rust::Slice<const double> weights,
                                                          int32_t nu, int32_t nv, rust::Slice<const double> knots_u,
                                                          rust::Slice<const double> knots_v, int32_t degree_u,
                                                          int32_t degree_v) {
  auto plain = make_bspline_surface_face(poles, nu, nv, knots_u, knots_v, degree_u, degree_v);
  if (static_cast<int>(weights.size()) != nu * nv) throw std::invalid_argument("bad weight grid");
  Handle(Geom_BSplineSurface) surface =
      Handle(Geom_BSplineSurface)::DownCast(BRep_Tool::Surface(TopoDS::Face(plain->shape))->Copy());
  for (int i = 0; i < nu; ++i)
    for (int j = 0; j < nv; ++j) surface->SetWeight(i + 1, j + 1, weights[i * nv + j]);
  return wrap(BRepBuilderAPI_MakeFace(surface, Precision::Confusion()).Face());
}

// Faces sewn together where their edges meet: one shell, or a compound of shells if they fall apart.
std::unique_ptr<Shape> sew_faces(const ShapeList &faces, double tolerance) {
  if (faces.shapes.size() == 1) {
    BRep_Builder builder;
    TopoDS_Shell shell;
    builder.MakeShell(shell);
    builder.Add(shell, faces.shapes[0]);
    return wrap(shell);
  }
  BRepBuilderAPI_Sewing sewing(tolerance);
  for (const TopoDS_Shape &f : faces.shapes) sewing.Add(f);
  sewing.Perform();
  TopoDS_Shape sewn = sewing.SewedShape();
  if (sewn.ShapeType() == TopAbs_FACE) {
    BRep_Builder builder;
    TopoDS_Shell shell;
    builder.MakeShell(shell);
    builder.Add(shell, sewn);
    return wrap(shell);
  }
  return wrap(sewn);
}

// A face on the cylinder about `frame` (origin on the axis, normal along it, x where the angle starts) of this
// radius: all the way round, and `half_height` along the axis each way. Its normal points away from the axis.
std::unique_ptr<Shape> cylinder_face(Frame frame, double radius, double half_height) {
  gp_Ax3 position(pnt(frame.origin), dir(frame.normal), dir(frame.x));
  Handle(Geom_CylindricalSurface) surface = new Geom_CylindricalSurface(position, radius);
  BRepBuilderAPI_MakeFace maker(surface, 0, 2 * M_PI, -half_height, half_height, Precision::Confusion());
  if (!maker.IsDone()) throw std::runtime_error("cylinder face failed");
  return wrap(maker.Face());
}

// A face on the same surface as `face_shape`, bounded by the parameter rectangle [u0, u1] x [v0, v1], facing the
// same way.
std::unique_ptr<Shape> surface_patch(const Shape &face_shape, double u0, double u1, double v0, double v1) {
  const TopoDS_Face &face = TopoDS::Face(face_shape.shape);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  BRepBuilderAPI_MakeFace maker(surface, u0, u1, v0, v1, Precision::Confusion());
  if (!maker.IsDone()) throw std::runtime_error("surface patch failed");
  TopoDS_Face patch = maker.Face();
  if (face.Orientation() == TopAbs_REVERSED) patch.Reverse();
  return wrap(patch);
}

// A face on the same surface as `face_shape`, bounded by one loop of edges given in its parameter space: edge i is
// the polyline through counts[i] (u, v) points (flattened into `uv`), each edge starting where the last ended. An
// edge at constant u or v is the surface's iso curve there; any other is interpolated through its points. The face
// faces the same way as `face_shape`.
std::unique_ptr<Shape> face_from_uv(const Shape &face_shape, rust::Slice<const double> uv, rust::Slice<const uint32_t> counts) {
  const TopoDS_Face &face = TopoDS::Face(face_shape.shape);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  BRepBuilderAPI_MakeWire wire;
  size_t at = 0;
  for (uint32_t count : counts) {
    if (count < 2 || 2 * (at + count) > uv.size()) throw std::runtime_error("face from uv: bad edge points");
    Handle(TColgp_HArray1OfPnt2d) points = new TColgp_HArray1OfPnt2d(1, count);
    for (uint32_t i = 0; i < count; ++i) points->SetValue(i + 1, gp_Pnt2d(uv[2 * (at + i)], uv[2 * (at + i) + 1]));
    at += count;
    const gp_Pnt2d a = points->Value(1), b = points->Value(count);
    bool same_u = true, same_v = true;
    for (uint32_t i = 1; i <= count; ++i) {
      same_u = same_u && std::abs(points->Value(i).X() - a.X()) < 1e-12;
      same_v = same_v && std::abs(points->Value(i).Y() - a.Y()) < 1e-12;
    }
    TopoDS_Edge edge;
    if (same_u || same_v) {
      Handle(Geom_Curve) iso = same_u ? surface->UIso(a.X()) : surface->VIso(a.Y());
      Handle(Geom2d_Line) pcurve = same_u ? new Geom2d_Line(gp_Pnt2d(a.X(), 0), gp_Dir2d(0, 1))
                                          : new Geom2d_Line(gp_Pnt2d(0, a.Y()), gp_Dir2d(1, 0));
      const double p1 = same_u ? a.Y() : a.X(), p2 = same_u ? b.Y() : b.X();
      edge = BRepBuilderAPI_MakeEdge(iso, std::min(p1, p2), std::max(p1, p2)).Edge();
      BRep_Builder().UpdateEdge(edge, pcurve, surface, TopLoc_Location(), Precision::Confusion());
      if (p2 < p1) edge.Reverse();
    } else {
      Geom2dAPI_Interpolate fit(points, Standard_False, 1e-12);
      fit.Perform();
      if (!fit.IsDone()) throw std::runtime_error("face from uv: an edge couldn't be fitted");
      Handle(Geom2d_BSplineCurve) spline = fit.Curve();
      edge = BRepBuilderAPI_MakeEdge(spline, surface, spline->FirstParameter(), spline->LastParameter()).Edge();
      BRepLib::BuildCurves3d(edge, 1e-7, GeomAbs_C1, 14, 200);
    }
    wire.Add(edge);
  }
  if (!wire.IsDone()) throw std::runtime_error("face from uv: the loop couldn't be built");
  auto make = [&](const TopoDS_Wire &w) {
    BRepBuilderAPI_MakeFace maker(surface, w, Standard_True);
    if (!maker.IsDone()) throw std::runtime_error("face from uv: the face couldn't be built");
    ShapeFix_Face fix(maker.Face());
    fix.Perform();
    return fix.Face();
  };
  TopoDS_Face built = make(wire.Wire());
  // A loop that runs clockwise in the surface's parameters (a torus's, say) bounds the outside: the other way round.
  GProp_GProps props;
  BRepGProp::SurfaceProperties(built, props);
  if (props.Mass() < 0) built = make(TopoDS::Wire(wire.Wire().Reversed()));
  if (face.Orientation() == TopAbs_REVERSED) built.Reverse();
  return wrap(built);
}

// A planar face with corners at some of its vertices rounded to `size` (or, for `chamfer`, cut back `size` along
// both edges), as a 2D fillet in its plane.
std::unique_ptr<Shape> face_corners_2d(const Shape &face_shape, const ShapeList &vertices, double size, bool chamfer) {
  const TopoDS_Face &face = TopoDS::Face(face_shape.shape);
  BRepFilletAPI_MakeFillet2d maker(face);
  TopTools_IndexedDataMapOfShapeListOfShape vertex_edges;
  TopExp::MapShapesAndAncestors(face, TopAbs_VERTEX, TopAbs_EDGE, vertex_edges);
  for (const TopoDS_Shape &v : vertices.shapes) {
    const int index = vertex_edges.FindIndex(v);
    if (index == 0) throw std::runtime_error("corner: the vertex isn't on the face");
    const TopoDS_Vertex vertex = TopoDS::Vertex(vertex_edges.FindKey(index));
    if (chamfer) {
      const TopTools_ListOfShape &edges = vertex_edges(index);
      if (edges.Extent() != 2) throw std::runtime_error("corner: a chamfer needs two edges at the vertex");
      maker.AddChamfer(TopoDS::Edge(edges.First()), TopoDS::Edge(edges.Last()), size, size);
    } else {
      maker.AddFillet(vertex, size);
    }
  }
  maker.Build();
  if (!maker.IsDone()) throw std::runtime_error("corner: the 2D fillet failed");
  return wrap(maker.Shape());
}

// A planar face with the corner at `vertex_shape` broken: `kind` 1 a chamfer `d1` along the edge coming into the
// corner (in the face's own loop order) and `d2` along the one leaving it; 2 a conic set back `d1` along both edges,
// its shoulder at `rho` of the way from the chord's middle to the corner.
std::unique_ptr<Shape> face_corner_2d(const Shape &face_shape, const Shape &vertex_shape, int kind, double d1, double d2, double rho) {
  const TopoDS_Face &face = TopoDS::Face(face_shape.shape);
  const TopoDS_Vertex &vertex = TopoDS::Vertex(vertex_shape.shape);
  TopoDS_Edge incoming, outgoing;
  for (BRepTools_WireExplorer it(BRepTools::OuterWire(face), face); it.More(); it.Next()) {
    const TopoDS_Edge &e = it.Current();
    if (TopExp::LastVertex(e, Standard_True).IsSame(vertex)) incoming = e;
    if (TopExp::FirstVertex(e, Standard_True).IsSame(vertex)) outgoing = e;
  }
  if (incoming.IsNull() || outgoing.IsNull()) throw std::runtime_error("corner: the vertex isn't a corner of the face");
  if (kind == 1) {
    BRepFilletAPI_MakeFillet2d maker(face);
    maker.AddChamfer(incoming, outgoing, d1, d2);
    maker.Build();
    if (!maker.IsDone()) throw std::runtime_error("corner: the 2D chamfer failed");
    return wrap(maker.Shape());
  }
  gp_Pnt v = BRep_Tool::Pnt(vertex);
  auto toward = [&](const TopoDS_Edge &e, bool from_end) {
    gp_Pnt other = BRep_Tool::Pnt(from_end ? TopExp::FirstVertex(e, Standard_True) : TopExp::LastVertex(e, Standard_True));
    gp_Vec d(v, other);
    if (d.Magnitude() < 1e-12) throw std::runtime_error("corner: a degenerate edge");
    return v.Translated(d.Normalized() * d1);
  };
  gp_Pnt p0 = toward(incoming, true), p2 = toward(outgoing, false);
  TColgp_Array1OfPnt poles(1, 3);
  poles(1) = p0;
  poles(2) = v;
  poles(3) = p2;
  TColStd_Array1OfReal weights(1, 3);
  weights(1) = 1;
  weights(2) = rho / (1 - rho);
  weights(3) = 1;
  TColStd_Array1OfReal knots(1, 2);
  knots(1) = 0;
  knots(2) = 1;
  TColStd_Array1OfInteger mults(1, 2);
  mults(1) = 3;
  mults(2) = 3;
  Handle(Geom_BSplineCurve) conic = new Geom_BSplineCurve(poles, weights, knots, mults, 2);
  BRepBuilderAPI_MakeWire wire;
  wire.Add(BRepBuilderAPI_MakeEdge(p0, v).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(v, p2).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(conic).Edge());
  if (!wire.IsDone()) throw std::runtime_error("corner: the conic's region failed");
  BRepBuilderAPI_MakeFace region(wire.Wire(), Standard_True);
  if (!region.IsDone()) throw std::runtime_error("corner: the conic's region failed");
  BRepAlgoAPI_Cut cut(face, region.Face());
  cut.Build();
  if (!cut.IsDone()) throw std::runtime_error("corner: cutting the conic corner failed");
  TopExp_Explorer first(cut.Shape(), TopAbs_FACE);
  if (!first.More()) throw std::runtime_error("corner: nothing left");
  return wrap(first.Current());
}

// The outline of solids projected along a plane's normal onto it (std's opCreateOutline for a planar target): their
// edges and silhouettes seen along the normal, drawn on the plane, split a face covering them; the pieces whose
// line along the normal passes through a solid are kept and joined.
std::unique_ptr<Shape> outline_on_plane(const ShapeList &tools, Frame plane) {
  gp_Ax2 axes = ax2(plane);
  Handle(HLRBRep_Algo) algo = new HLRBRep_Algo();
  for (const TopoDS_Shape &t : tools.shapes) algo->Add(t);
  algo->Projector(HLRAlgo_Projector(axes));
  algo->Update();
  algo->Hide();
  HLRBRep_HLRToShape lines(algo);
  // The projector's drawing is in its own plane's coordinates: carried onto the plane.
  gp_Trsf onto;
  onto.SetTransformation(gp_Ax3(axes), gp_Ax3());
  TopoDS_Compound edges;
  BRep_Builder builder;
  builder.MakeCompound(edges);
  bool any = false;
  for (const TopoDS_Shape &c : {lines.VCompound(), lines.HCompound(), lines.OutLineVCompound(), lines.OutLineHCompound(),
                                lines.Rg1LineVCompound(), lines.Rg1LineHCompound()}) {
    if (c.IsNull()) continue;
    BRepLib::BuildCurves3d(c);
    for (TopExp_Explorer e(c, TopAbs_EDGE); e.More(); e.Next()) {
      TopoDS_Shape moved = BRepBuilderAPI_Transform(e.Current(), onto, Standard_True).Shape();
      builder.Add(edges, moved);
      any = true;
    }
  }
  if (!any) throw std::runtime_error("outline: nothing to project");
  Bnd_Box box;
  BRepBndLib::Add(edges, box);
  double x0, y0, z0, x1, y1, z1;
  box.Get(x0, y0, z0, x1, y1, z1);
  const double size = gp_Pnt(x0, y0, z0).Distance(gp_Pnt(x1, y1, z1)) + 1.0;
  gp_Pln pln(axes.Location(), axes.Direction());
  TopoDS_Face big = BRepBuilderAPI_MakeFace(pln, -size * 4, size * 4, -size * 4, size * 4).Face();
  // Centre the face on the drawing.
  gp_Pnt centre((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
  gp_Pnt at = pln.Location();
  gp_Vec shift(at, centre);
  shift -= gp_Vec(axes.Direction()) * shift.Dot(gp_Vec(axes.Direction()));
  gp_Trsf move;
  move.SetTranslation(shift);
  big = TopoDS::Face(BRepBuilderAPI_Transform(big, move, Standard_True).Shape());
  BRepAlgoAPI_Splitter splitter;
  TopTools_ListOfShape args, cutters;
  args.Append(big);
  for (TopExp_Explorer e(edges, TopAbs_EDGE); e.More(); e.Next()) cutters.Append(e.Current());
  splitter.SetArguments(args);
  splitter.SetTools(cutters);
  splitter.Build();
  if (!splitter.IsDone()) throw std::runtime_error("outline: the drawing doesn't split the plane");
  TopoDS_Compound tool_compound;
  builder.MakeCompound(tool_compound);
  for (const TopoDS_Shape &t : tools.shapes) builder.Add(tool_compound, t);
  IntCurvesFace_ShapeIntersector through;
  through.Load(tool_compound, 1e-7);
  TopoDS_Compound kept;
  builder.MakeCompound(kept);
  int count = 0;
  for (TopExp_Explorer f(splitter.Shape(), TopAbs_FACE); f.More(); f.Next()) {
    TopoDS_Face face = TopoDS::Face(f.Current());
    gp_Pnt p = pnt(face_inner_point(Shape(face)));
    gp_Lin ray(p.Translated(gp_Vec(axes.Direction()) * (-size * 4)), axes.Direction());
    through.Perform(ray, 0, size * 8);
    if (through.IsDone() && through.NbPnt() > 0) {
      builder.Add(kept, face);
      ++count;
    }
  }
  if (count == 0) throw std::runtime_error("outline: no piece lies under the tools");
  ShapeUpgrade_UnifySameDomain unifier(kept, Standard_True, Standard_True, Standard_False);
  unifier.Build();
  return wrap(unifier.Shape());
}

}  // namespace occt
