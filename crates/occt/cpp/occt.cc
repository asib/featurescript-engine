#include "occt/cpp/occt.h"
#include <ShapeFix_Edge.hxx>
#include <Geom_BSplineCurve.hxx>
#include <BRepTools_ReShape.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_CompCurveToBSplineCurve.hxx>
#include <Geom2d_Curve.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <Geom_BezierCurve.hxx>
#include <ElCLib.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Line.hxx>
#include <GC_MakeCircle.hxx>
#include <BRepLib.hxx>
#include <GProp_PrincipalProps.hxx>
#include <algorithm>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_NurbsConvert.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools_History.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <Poly_Triangulation.hxx>
#include <Geom_Curve.hxx>
#include <Bnd_Box.hxx>
#include <ElSLib.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GProp_GProps.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <Interface_InterfaceModel.hxx>
#include <Interface_Static.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <STEPControl_Reader.hxx>
#include <StepData_StepModel.hxx>
#include <ShapeUpgrade_ShapeDivideArea.hxx>
#include <ShapeUpgrade_ShapeDivideClosed.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopTools_DataMapOfShapeShape.hxx>
#include <TopTools_DataMapOfIntegerShape.hxx>
#include <TopTools_DataMapOfIntegerListOfShape.hxx>
#include <BRepLib_FuseEdges.hxx>
#include <StepShape_ManifoldSolidBrep.hxx>
#include <TCollection_ExtendedString.hxx>
#include <TCollection_HAsciiString.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <TopLoc_Location.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_MapOfShape.hxx>
#include <TopoDS.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <gp_Elips.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Compound.hxx>
#include <TransferBRep.hxx>
#include <Transfer_TransientProcess.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <XSControl_TransferReader.hxx>
#include <XSControl_WorkSession.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Pln.hxx>
#include <gp_Trsf.hxx>

#include <algorithm>
#include <sstream>
#include <stdexcept>

#include "occt/src/lib.rs.h"
#include "occt/cpp/internal.h"

namespace occt {

std::unique_ptr<Shape> shape_clone(const Shape &s) { return wrap(s.shape); }
std::unique_ptr<Shape> shape_reversed(const Shape &s) { return wrap(s.shape.Reversed()); }
bool shape_is_null(const Shape &s) { return s.shape.IsNull(); }
int32_t shape_kind(const Shape &s) { return s.shape.IsNull() ? -1 : static_cast<int32_t>(s.shape.ShapeType()); }
int32_t shape_orientation(const Shape &s) { return static_cast<int32_t>(s.shape.Orientation()); }
bool shape_is_same(const Shape &a, const Shape &b) { return a.shape.IsSame(b.shape); }
uint64_t shape_hash(const Shape &s) { return std::hash<TopoDS_Shape>{}(s.shape); }

std::unique_ptr<ShapeList> list_new() { return std::make_unique<ShapeList>(); }
size_t list_len(const ShapeList &l) { return l.shapes.size(); }
std::unique_ptr<Shape> list_get(const ShapeList &l, size_t i) { return wrap(l.shapes.at(i)); }
void list_push(ShapeList &l, const Shape &s) { l.shapes.push_back(s.shape); }

std::unique_ptr<ShapeList> sub_shapes(const Shape &s, int32_t kind) {
  TopTools_IndexedMapOfShape map;
  TopExp::MapShapes(s.shape, shape_enum(kind), map);
  auto out = std::make_unique<ShapeList>();
  for (int i = 1; i <= map.Extent(); ++i) out->shapes.push_back(map(i));
  return out;
}

std::unique_ptr<ShapeList> ancestors(const Shape &root, const Shape &s, int32_t kind) {
  TopTools_IndexedDataMapOfShapeListOfShape map;
  TopExp::MapShapesAndUniqueAncestors(root.shape, s.shape.ShapeType(), shape_enum(kind), map);
  const TopTools_ListOfShape *found = map.Seek(s.shape);
  return found == nullptr ? std::make_unique<ShapeList>() : wrap_list(*found);
}

std::unique_ptr<Shape> make_compound(const ShapeList &l) {
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (const TopoDS_Shape &s : l.shapes) builder.Add(compound, s);
  return wrap(compound);
}

std::unique_ptr<Shape> make_vertex(Vec3 p) { return wrap(BRepBuilderAPI_MakeVertex(pnt(p)).Vertex()); }

std::unique_ptr<Shape> make_line_edge(Vec3 a, Vec3 b) {
  BRepBuilderAPI_MakeEdge maker(pnt(a), pnt(b));
  check_done(maker, "line edge");
  return wrap(maker.Edge());
}

std::unique_ptr<Shape> make_circle_edge(Frame f, double radius) {
  BRepBuilderAPI_MakeEdge maker(gp_Circ(ax2(f), radius));
  check_done(maker, "circle edge");
  return wrap(maker.Edge());
}

std::unique_ptr<Shape> make_arc_edge(Frame f, double radius, double start, double end) {
  BRepBuilderAPI_MakeEdge maker(gp_Circ(ax2(f), radius), start, end);
  check_done(maker, "arc edge");
  return wrap(maker.Edge());
}

std::unique_ptr<Shape> make_arc_through(Vec3 a, Vec3 mid, Vec3 b) {
  GC_MakeArcOfCircle arc(pnt(a), pnt(mid), pnt(b));
  if (!arc.IsDone()) throw std::runtime_error("arc through three points failed");
  BRepBuilderAPI_MakeEdge maker(arc.Value());
  check_done(maker, "arc edge");
  return wrap(maker.Edge());
}

std::unique_ptr<Shape> make_plane_face(Frame f, double umin, double umax, double vmin, double vmax) {
  BRepBuilderAPI_MakeFace maker(gp_Pln(ax3(f)), umin, umax, vmin, vmax);
  check_done(maker, "plane face");
  return wrap(maker.Face());
}

// A closed edge on a periodic curve restarted at the first point where another of the edges meets it (crossing or
// touching), so that the regions get no vertex at its own seam: Onshape's have none where curves bounding regions
// meet a closed one (batch 194: an ellipse's region edges run between where another crosses it, a circle touching
// an ellipse starts both their edges where they touch; a line merely touching one, bounding nothing, leaves it be,
// batch 192 c011). Unchanged if nothing meets it, or something meets it at its seam.
static TopoDS_Shape restart_closed(const TopoDS_Shape &shape, const std::vector<TopoDS_Shape> &others) {
  if (shape.ShapeType() != TopAbs_EDGE) return shape;
  const TopoDS_Edge &edge = TopoDS::Edge(shape);
  TopoDS_Vertex v0, v1;
  TopExp::Vertices(edge, v0, v1);
  if (v0.IsNull() || !v0.IsSame(v1)) return shape;
  double first, last;
  Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
  if (curve.IsNull() || !curve->IsPeriodic()) return shape;
  const double period = curve->Period();
  if (std::abs(last - first - period) > 1e-9) return shape;
  Bnd_Box box;
  BRepBndLib::Add(edge, box);
  box.Enlarge(1e-6);
  bool found = false;
  double best = 0.0;
  for (const TopoDS_Shape &other_edge : others) {
    if (other_edge.IsSame(shape)) continue;
    Bnd_Box other;
    BRepBndLib::Add(other_edge, other);
    if (box.IsOut(other)) continue;
    BRepExtrema_DistShapeShape distance(edge, other_edge);
    if (!distance.IsDone() || distance.Value() > 1e-7) continue;
    for (int k = 1; k <= distance.NbSolution(); ++k) {
      double t = first;
      if (distance.SupportTypeShape1(k) == BRepExtrema_IsOnEdge) distance.ParOnEdgeS1(k, t);
      double along = std::fmod(t - first, period);
      if (along < 0) along += period;
      if (along < 1e-9 || period - along < 1e-9) return shape;
      if (!found || along < best) best = along, found = true;
    }
  }
  if (!found) return shape;
  BRepBuilderAPI_MakeEdge maker(curve, first + best, first + best + period);
  if (!maker.IsDone()) return shape;
  TopoDS_Shape restarted = maker.Edge();
  restarted.Orientation(edge.Orientation());
  return restarted;
}

// Splits a face larger than all the edges by the edges, then drops the faces that reach the big face's boundary:
// the regions the edges enclose, oriented with the frame's normal. `bounding` gets the edges whose pieces bound them.
static std::vector<TopoDS_Shape> split_regions(const std::vector<TopoDS_Shape> &edges, Frame f,
                                               std::vector<TopoDS_Shape> *bounding) {
  std::vector<TopoDS_Shape> out;
  gp_Pln plane(ax3(f));
  Bnd_Box box;
  for (const TopoDS_Shape &e : edges) BRepBndLib::Add(e, box);
  double x0, y0, z0, x1, y1, z1;
  box.Get(x0, y0, z0, x1, y1, z1);
  double umin = 1e100, umax = -1e100, vmin = 1e100, vmax = -1e100;
  for (int i = 0; i < 8; ++i) {
    gp_Pnt corner(i & 1 ? x1 : x0, i & 2 ? y1 : y0, i & 4 ? z1 : z0);
    double u, v;
    ElSLib::Parameters(plane, corner, u, v);
    umin = std::min(umin, u), umax = std::max(umax, u), vmin = std::min(vmin, v), vmax = std::max(vmax, v);
  }
  double margin = std::max(umax - umin, vmax - vmin) + 1.0;
  BRepBuilderAPI_MakeFace outer(plane, umin - margin, umax + margin, vmin - margin, vmax + margin);
  check_done(outer, "region bounding face");

  BRepAlgoAPI_Splitter splitter;
  TopTools_ListOfShape arguments, tools;
  arguments.Append(outer.Face());
  for (const TopoDS_Shape &e : edges) tools.Append(e);
  splitter.SetArguments(arguments);
  splitter.SetTools(tools);
  splitter.SetRunParallel(Standard_False);
  splitter.Build();
  if (splitter.HasErrors()) {
    std::ostringstream message;
    splitter.DumpErrors(message);
    throw std::runtime_error("sketch regions: " + message.str());
  }

  TopTools_MapOfShape boundary;
  TopTools_IndexedMapOfShape outer_edges;
  TopExp::MapShapes(outer.Face(), TopAbs_EDGE, outer_edges);
  for (int i = 1; i <= outer_edges.Extent(); ++i) {
    boundary.Add(outer_edges(i));
    for (const TopoDS_Shape &piece : splitter.Modified(outer_edges(i))) boundary.Add(piece);
  }
  TopTools_MapOfShape kept_edges;
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(splitter.Shape(), TopAbs_FACE, faces);
  for (int i = 1; i <= faces.Extent(); ++i) {
    TopTools_IndexedMapOfShape face_edges;
    TopExp::MapShapes(faces(i), TopAbs_EDGE, face_edges);
    bool outside = false;
    for (int j = 1; j <= face_edges.Extent() && !outside; ++j) outside = boundary.Contains(face_edges(j));
    if (outside) continue;
    out.push_back(faces(i));
    for (int j = 1; j <= face_edges.Extent(); ++j) kept_edges.Add(face_edges(j));
  }
  if (bounding) {
    for (const TopoDS_Shape &e : edges) {
      const TopTools_ListOfShape &pieces = splitter.Modified(e);
      bool bounds = pieces.IsEmpty() ? kept_edges.Contains(e) : false;
      for (const TopoDS_Shape &piece : pieces) bounds = bounds || kept_edges.Contains(piece);
      if (bounds) bounding->push_back(e);
    }
  }
  return out;
}

// The regions the edges enclose, closed edges restarted where the edges bounding regions meet them.
std::unique_ptr<ShapeList> sketch_regions(const ShapeList &edges, Frame f) {
  auto out = std::make_unique<ShapeList>();
  if (edges.shapes.empty()) return out;
  std::vector<TopoDS_Shape> bounding;
  std::vector<TopoDS_Shape> regions = split_regions(edges.shapes, f, &bounding);
  std::vector<TopoDS_Shape> restarted;
  bool changed = false;
  for (const TopoDS_Shape &e : edges.shapes) {
    restarted.push_back(restart_closed(e, bounding));
    changed = changed || !restarted.back().IsSame(e);
  }
  if (changed) regions = split_regions(restarted, f, nullptr);
  out->shapes = regions;
  return out;
}

// Merges faces that share edges and lie on the same surface into single faces.
std::unique_ptr<Operation> merge_faces(const ShapeList &faces) {
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History();
  if (faces.shapes.size() < 2) {
    op->result = faces.shapes.empty() ? TopoDS_Shape() : faces.shapes[0];
    return op;
  }
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (const TopoDS_Shape &s : faces.shapes) builder.Add(compound, s);
  ShapeUpgrade_UnifySameDomain unify(compound, Standard_True, Standard_True, Standard_False);
  unify.Build();
  op->result = unify.Shape();
  op->history = unify.History();
  return op;
}

// A shape with its same-domain edges merged (collinear lines meeting at a vertex only they share, arcs of one
// circle), faces left as they are.
std::unique_ptr<Operation> merge_edges(const Shape &s) {
  auto op = std::make_unique<Operation>();
  ShapeUpgrade_UnifySameDomain unify(s.shape, Standard_True, Standard_False, Standard_False);
  unify.Build();
  op->result = unify.Shape();
  op->history = unify.History();
  return op;
}

std::unique_ptr<Operation> prism(const Shape &profile, Vec3 v) {
  BRepPrimAPI_MakePrism maker(profile.shape, gp_Vec(v.x, v.y, v.z), Standard_False, Standard_True);
  maker.Build();
  check_done(maker, "extrude");
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape arguments;
  arguments.Append(profile.shape);
  op->history = new BRepTools_History(arguments, maker);
  op->result = maker.Shape();
  // The caps: the profile's faces at each end, or for a profile of edges (a sheet), its edges at each end.
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(profile.shape, TopAbs_FACE, faces);
  if (faces.IsEmpty()) TopExp::MapShapes(profile.shape, TopAbs_EDGE, faces);
  for (int i = 1; i <= faces.Extent(); ++i) {
    op->first.push_back(maker.FirstShape(faces(i)));
    op->last.push_back(maker.LastShape(faces(i)));
  }
  return op;
}

// Edges left on one ellipse (or another conic) that meet at a vertex nothing else uses, joined into one: the
// same-domain unifier joins only lines and circles. A closed hem's corner sheet is one with the piece std adds at
// its base (batch 021 c012), its end one edge as in Onshape.
namespace {

// Joins chains of B-spline edges that lie between the same two faces and meet at vertices only they share: the
// curves concatenated (each piece turned to run along the chain), one edge on both faces. A chain whose joined edge
// isn't as tight as its pieces is left as it was. Recorded in `history`.
TopoDS_Shape bspline_chains_joined(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history) {
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces, vertex_edges;
  TopExp::MapShapesAndUniqueAncestors(shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  TopExp::MapShapesAndUniqueAncestors(shape, TopAbs_VERTEX, TopAbs_EDGE, vertex_edges);
  auto is_spline = [](const TopoDS_Edge &e) {
    if (BRep_Tool::Degenerated(e)) return false;
    GeomAbs_CurveType t = BRepAdaptor_Curve(e).GetType();
    return t == GeomAbs_BSplineCurve || t == GeomAbs_BezierCurve;
  };
  auto same_faces = [&](const TopoDS_Shape &a, const TopoDS_Shape &b) {
    if (!edge_faces.Contains(a) || !edge_faces.Contains(b)) return false;
    const TopTools_ListOfShape &fa = edge_faces.FindFromKey(a), &fb = edge_faces.FindFromKey(b);
    if (fa.Extent() != 2 || fb.Extent() != 2) return false;
    for (const TopoDS_Shape &f : fa) {
      bool found = false;
      for (const TopoDS_Shape &g : fb) found = found || f.IsSame(g);
      if (!found) return false;
    }
    // Neither a seam of a face it lies on.
    for (const TopoDS_Shape &f : fa)
      if (BRep_Tool::IsClosed(TopoDS::Edge(a), TopoDS::Face(f)) || BRep_Tool::IsClosed(TopoDS::Edge(b), TopoDS::Face(f)))
        return false;
    return true;
  };
  // The way an edge leaves a vertex of it.
  auto leaving = [](const TopoDS_Edge &e, const TopoDS_Vertex &v) {
    BRepAdaptor_Curve curve(e);
    const double t = BRep_Tool::Parameter(v, e);
    gp_Pnt p;
    gp_Vec d;
    curve.D1(t, p, d);
    if (std::abs(t - curve.LastParameter()) < std::abs(t - curve.FirstParameter())) d.Reverse();
    return d;
  };
  // Vertices a chain runs through: two edges only, both B-splines between the same two faces, running on smoothly
  // (a rectangle's projection round a cylinder keeps its corners, batch 138 c005).
  auto joint = [&](const TopoDS_Shape &v, TopoDS_Shape &a, TopoDS_Shape &b) {
    const TopTools_ListOfShape &edges = vertex_edges.FindFromKey(v);
    if (edges.Extent() != 2) return false;
    a = edges.First();
    b = edges.Last();
    if (a.IsSame(b)) return false;
    if (!is_spline(TopoDS::Edge(a)) || !is_spline(TopoDS::Edge(b)) || !same_faces(a, b)) return false;
    const gp_Vec ta = leaving(TopoDS::Edge(a), TopoDS::Vertex(v)), tb = leaving(TopoDS::Edge(b), TopoDS::Vertex(v));
    return ta.Magnitude() > 1e-12 && tb.Magnitude() > 1e-12 && ta.Angle(-tb) < 1e-3;
  };
  TopTools_MapOfShape used;
  Handle(BRepTools_ReShape) reshape = new BRepTools_ReShape;
  Handle(BRepTools_History) joined_history = new BRepTools_History;
  bool changed = false;
  for (int i = 1; i <= vertex_edges.Extent(); ++i) {
    TopoDS_Shape a, b;
    if (!joint(vertex_edges.FindKey(i), a, b) || used.Contains(a)) continue;
    // Walk the chain both ways from this edge.
    std::vector<TopoDS_Edge> chain{TopoDS::Edge(a)};
    TopTools_MapOfShape in_chain;
    in_chain.Add(a);
    for (int direction = 0; direction < 2; ++direction) {
      TopoDS_Edge end = direction == 0 ? chain.back() : chain.front();
      while (true) {
        bool extended = false;
        for (TopExp_Explorer v(end, TopAbs_VERTEX); v.More() && !extended; v.Next()) {
          TopoDS_Shape x, y;
          if (!joint(v.Current(), x, y)) continue;
          TopoDS_Shape next = x.IsSame(end) ? y : x;
          if (in_chain.Contains(next)) continue;
          in_chain.Add(next);
          if (direction == 0) chain.push_back(TopoDS::Edge(next)); else chain.insert(chain.begin(), TopoDS::Edge(next));
          end = TopoDS::Edge(next);
          extended = true;
        }
        if (!extended) break;
      }
    }
    for (const TopoDS_Edge &e : chain) used.Add(e);
    if (chain.size() < 2) continue;
    // The curves, each run along the chain from the first edge's start.
    TopoDS_Vertex first_vertex, current;
    GeomConvert_CompCurveToBSplineCurve compound;
    bool ok = true;
    double tolerance = 0;
    for (size_t k = 0; k < chain.size() && ok; ++k) {
      const TopoDS_Edge &e = chain[k];
      TopoDS_Edge forward = TopoDS::Edge(e.Oriented(TopAbs_FORWARD));
      TopoDS_Vertex v0 = TopExp::FirstVertex(forward), v1 = TopExp::LastVertex(forward);
      double f, l;
      Handle(Geom_Curve) curve = BRep_Tool::Curve(forward, f, l);
      if (curve.IsNull()) { ok = false; break; }
      Handle(Geom_BSplineCurve) piece = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(curve, f, l));
      bool reversed = false;
      if (k == 0) {
        // Start where the chain does: at the end the next edge doesn't share.
        const TopoDS_Edge &next = chain[1];
        bool v1_shared = false;
        for (TopExp_Explorer v(next, TopAbs_VERTEX); v.More(); v.Next()) v1_shared = v1_shared || v.Current().IsSame(v1);
        if (!v1_shared) reversed = true;
        first_vertex = reversed ? v1 : v0;
        current = reversed ? v0 : v1;
      } else if (v0.IsSame(current)) {
        current = v1;
      } else if (v1.IsSame(current)) {
        reversed = true;
        current = v0;
      } else {
        ok = false;
        break;
      }
      if (reversed) piece->Reverse();
      tolerance = std::max(tolerance, BRep_Tool::Tolerance(e));
      if (!compound.Add(piece, std::max(tolerance, 1e-7) * 10, Standard_True)) ok = false;
    }
    if (!ok) continue;
    Handle(Geom_BSplineCurve) curve = compound.BSplineCurve();
    // The vertices kept, their tolerances widened to the curve's ends if need be.
    BRep_Builder builder;
    for (const TopoDS_Vertex &v : {first_vertex, current}) {
      gp_Pnt p = BRep_Tool::Pnt(v);
      double gap = std::max(p.Distance(curve->StartPoint()) * (v.IsSame(first_vertex) ? 1 : 0),
                            p.Distance(curve->EndPoint()) * (v.IsSame(current) ? 1 : 0));
      if (gap > BRep_Tool::Tolerance(v)) builder.UpdateVertex(v, gap * 1.01);
    }
    BRepBuilderAPI_MakeEdge maker(curve, first_vertex, current, curve->FirstParameter(), curve->LastParameter());
    if (!maker.IsDone()) continue;
    TopoDS_Edge joined = maker.Edge();
    // On both faces: curves there projected from the 3D curve.
    const TopTools_ListOfShape &faces = edge_faces.FindFromKey(chain[0]);
    ShapeFix_Edge fix;
    for (const TopoDS_Shape &f : faces) fix.FixAddPCurve(joined, TopoDS::Face(f), Standard_False, 1e-7);
    fix.FixSameParameter(joined);
    if (BRep_Tool::Tolerance(joined) > std::max(10 * tolerance, 1e-6)) continue;
    // The joined edge runs as the chain's first edge did, when it was forward; replacing that edge, the others go.
    TopoDS_Edge first = chain[0];
    TopoDS_Vertex fv = TopExp::FirstVertex(TopoDS::Edge(first.Oriented(TopAbs_FORWARD)));
    TopoDS_Shape replacement = fv.IsSame(first_vertex) ? TopoDS_Shape(joined) : joined.Reversed();
    reshape->Replace(first.Oriented(TopAbs_FORWARD), replacement);
    for (size_t k = 1; k < chain.size(); ++k) reshape->Remove(chain[k]);
    for (const TopoDS_Edge &e : chain) joined_history->AddModified(e, joined);
    changed = true;
  }
  if (!changed) return shape;
  TopoDS_Shape out = reshape->Apply(shape);
  // The faces rebuilt round the joined edges carry on the faces they were.
  {
    Handle(BRepTools_History) rebuilt = reshape->History();
    for (TopExp_Explorer f(shape, TopAbs_FACE); f.More(); f.Next()) {
      if (rebuilt.IsNull()) break;
      const TopTools_ListOfShape &images = rebuilt->Modified(f.Current());
      for (const TopoDS_Shape &image : images)
        if (!image.IsSame(f.Current())) joined_history->AddModified(f.Current(), image);
    }
  }
  // The vertices between pieces go.
  TopTools_IndexedMapOfShape kept;
  TopExp::MapShapes(out, TopAbs_VERTEX, kept);
  for (int i = 1; i <= vertex_edges.Extent(); ++i)
    if (!kept.Contains(vertex_edges.FindKey(i))) joined_history->Remove(vertex_edges.FindKey(i));
  history->Merge(joined_history);
  return out;
}

}  // namespace

// Edges between the same two faces meeting at a vertex only they share become one edge: conics on one circle or
// line (`BRepLib_FuseEdges`), and chains of B-splines, which OCCT leaves in pieces where Parasolid keeps one edge
// (an intersection curve, batch 124) and whose own concatenation can come out millimetres out (batch 125 c008).
TopoDS_Shape edges_fused(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history) {
  TopoDS_Shape result = shape;
  {
    BRepLib_FuseEdges fuser(shape, Standard_False);
    fuser.SetConcatBSpl(Standard_False);
    if (fuser.NbVertices() > 0) {
      fuser.Perform();
      TopTools_DataMapOfIntegerListOfShape old_edges;
      TopTools_DataMapOfIntegerShape new_edges;
      TopTools_DataMapOfShapeShape faces;
      fuser.Edges(old_edges);
      fuser.ResultEdges(new_edges);
      fuser.Faces(faces);
      Handle(BRepTools_History) fused = new BRepTools_History;
      for (TopTools_DataMapIteratorOfDataMapOfIntegerListOfShape it(old_edges); it.More(); it.Next()) {
        if (!new_edges.IsBound(it.Key())) continue;
        const TopoDS_Shape &edge = new_edges(it.Key());
        TopTools_IndexedMapOfShape kept;
        TopExp::MapShapes(edge, TopAbs_VERTEX, kept);
        for (TopTools_ListIteratorOfListOfShape e(it.Value()); e.More(); e.Next()) {
          fused->AddModified(e.Value(), edge);
          for (TopExp_Explorer v(e.Value(), TopAbs_VERTEX); v.More(); v.Next())
            if (!kept.Contains(v.Current())) fused->Remove(v.Current());
        }
      }
      for (TopTools_DataMapIteratorOfDataMapOfShapeShape it(faces); it.More(); it.Next())
        if (!it.Key().IsSame(it.Value())) fused->AddModified(it.Key(), it.Value());
      history->Merge(fused);
      result = fuser.Shape();
    }
  }
  return bspline_chains_joined(result, history);
}

// The shape with its B-spline edges that lie on a line or a circle (within 0.1 µm) given that line or circle as
// their curve, their curves on their faces refitted: as Onshape's, whose split of a cylinder's side by a rectangle
// projected onto it has lines up it and arcs round it (batch 138 c005). The faces are kept.
std::unique_ptr<Operation> canonical_edges_op(const Shape &s) {
  BRepBuilderAPI_Copy copier(s.shape, Standard_True);
  const TopoDS_Shape copy = copier.Shape();
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape arguments;
  arguments.Append(s.shape);
  op->history = new BRepTools_History(arguments, copier);
  op->result = copy;
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(copy, TopAbs_EDGE, edges);
  BRep_Builder builder;
  const double tolerance = 1e-7;
  for (int i = 1; i <= edges.Extent(); ++i) {
    TopoDS_Edge edge = TopoDS::Edge(edges(i));
    if (BRep_Tool::Degenerated(edge)) continue;
    double first, last;
    Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
    if (curve.IsNull()) continue;
    if (!curve->IsKind(STANDARD_TYPE(Geom_BSplineCurve)) && !curve->IsKind(STANDARD_TYPE(Geom_BezierCurve))) continue;
    const int samples = 32;
    std::vector<gp_Pnt> points;
    for (int k = 0; k <= samples; ++k) points.push_back(curve->Value(first + (last - first) * k / samples));
    const gp_Pnt a = points.front(), m = points[samples / 2], b = points.back();
    Handle(Geom_Curve) canonical;
    double c0 = 0, c1 = 0;
    // A line through its ends.
    if (a.Distance(b) > 1e-9) {
      gp_Lin line(a, gp_Dir(gp_Vec(a, b)));
      bool on = true;
      for (const gp_Pnt &p : points) on = on && line.Distance(p) < tolerance;
      if (on) {
        canonical = new Geom_Line(line);
        c0 = ElCLib::Parameter(line, a);
        c1 = ElCLib::Parameter(line, b);
      }
    }
    // A circle through its ends and middle (or, closed, its quarters).
    if (canonical.IsNull()) {
      gp_Pnt p0 = a, p1 = points[samples / 4], p2 = m;
      if (a.Distance(b) > 1e-9) p1 = m, p2 = b;
      GC_MakeCircle through(p0, p1, p2);
      if (through.IsDone()) {
        Handle(Geom_Circle) circle = through.Value();
        const gp_Circ c = circle->Circ();
        bool on = true;
        for (const gp_Pnt &p : points) on = on && c.Distance(p) < tolerance;
        if (on) {
          // Running the way the edge runs: the circle's own way round or turned over.
          const double u0 = ElCLib::Parameter(c, a), uq = ElCLib::Parameter(c, points[1]);
          double forward = uq - u0;
          while (forward < 0) forward += 2 * M_PI;
          if (forward > M_PI) circle->Reverse();
          const gp_Circ turned = circle->Circ();
          c0 = ElCLib::Parameter(turned, a);
          c1 = ElCLib::Parameter(turned, b);
          if (c1 <= c0 + 1e-12) c1 += 2 * M_PI;
          canonical = circle;
        }
      }
    }
    if (canonical.IsNull()) continue;
    builder.UpdateEdge(edge, canonical, BRep_Tool::Tolerance(edge));
    builder.Range(edge, c0, c1, Standard_True);
    BRepLib::SameParameter(edge, tolerance, Standard_True);
  }
  if (!BRepCheck_Analyzer(copy).IsValid()) {
    // Leave it as it was.
    op->history = new BRepTools_History;
    op->result = s.shape;
  }
  return op;
}

// An edge OCCT split at seam vertices, whole again, as Onshape's (whose faces have no seams): its pieces (the first
// running the way the edge runs) chained, one arc where they lie on one circle, one line where they lie on one line,
// otherwise their curves concatenated (batch 138 c005: a frame's profile placed at an arc's start).
std::unique_ptr<Shape> joined_edge(const ShapeList &pieces) {
  if (pieces.shapes.empty()) throw std::runtime_error("joined edge: no pieces");
  if (pieces.shapes.size() == 1) return wrap(pieces.shapes[0]);
  BRepBuilderAPI_MakeWire wire;
  for (const TopoDS_Shape &e : pieces.shapes) wire.Add(TopoDS::Edge(e));
  if (!wire.IsDone()) throw std::runtime_error("joined edge: the pieces don't chain");
  // The chain's ordered edges, turned to run along it, starting with the first piece.
  std::vector<TopoDS_Edge> ordered;
  for (BRepTools_WireExplorer it(wire.Wire()); it.More(); it.Next()) ordered.push_back(it.Current());
  const TopoDS_Edge first = TopoDS::Edge(pieces.shapes[0]);
  size_t start = 0;
  for (size_t i = 0; i < ordered.size(); ++i)
    if (ordered[i].IsSame(first)) start = i;
  // Running the first piece's way round.
  const bool flip = ordered[start].Orientation() != first.Orientation();
  std::vector<TopoDS_Edge> chain;
  for (size_t k = 0; k < ordered.size(); ++k) {
    size_t i = flip ? (start + ordered.size() - k) % ordered.size() : (start + k) % ordered.size();
    chain.push_back(TopoDS::Edge(flip ? ordered[i].Reversed() : ordered[i]));
  }
  // Open chains start at a free end, not in the middle.
  TopoDS_Vertex v0, v1;
  TopExp::Vertices(wire.Wire(), v0, v1);
  const bool closed = v0.IsNull() || v0.IsSame(v1);
  if (!closed) {
    for (size_t r = 0; r < chain.size(); ++r) {
      TopoDS_Vertex a = TopExp::FirstVertex(chain[0], Standard_True);
      if (a.IsSame(v0) || a.IsSame(v1)) break;
      std::rotate(chain.begin(), chain.begin() + 1, chain.end());
    }
  }
  const gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(chain.front(), Standard_True));
  const gp_Pnt b = BRep_Tool::Pnt(TopExp::LastVertex(chain.back(), Standard_True));
  BRepAdaptor_Curve c0(chain.front());
  bool all_circles = true, all_lines = true;
  for (const TopoDS_Edge &e : chain) {
    BRepAdaptor_Curve c(e);
    if (c.GetType() != GeomAbs_Circle || c0.GetType() != GeomAbs_Circle ||
        c.Circle().Location().Distance(c0.Circle().Location()) > 1e-9 ||
        std::abs(c.Circle().Radius() - c0.Circle().Radius()) > 1e-9 ||
        !c.Circle().Axis().Direction().IsParallel(c0.Circle().Axis().Direction(), 1e-9))
      all_circles = false;
    if (c.GetType() != GeomAbs_Line || c0.GetType() != GeomAbs_Line ||
        !c.Line().Direction().IsParallel(c0.Line().Direction(), 1e-9) || c0.Line().Distance(c.Line().Location()) > 1e-9)
      all_lines = false;
  }
  if (all_circles) {
    // The circle turned to run the way the chain does at its start.
    gp_Circ circle = c0.Circle();
    gp_Pnt p;
    gp_Vec t;
    c0.D1(c0.FirstParameter() + 1e-6 * (c0.LastParameter() - c0.FirstParameter()), p, t);
    if (chain.front().Orientation() == TopAbs_REVERSED) {
      c0.D1(c0.LastParameter() - 1e-6 * (c0.LastParameter() - c0.FirstParameter()), p, t);
      t.Reverse();
    }
    const gp_Vec radial(circle.Location(), p);
    if (gp_Vec(circle.Axis().Direction()).Crossed(radial).Dot(t) < 0) circle.SetAxis(circle.Axis().Reversed());
    double u0 = ElCLib::Parameter(circle, a), u1 = ElCLib::Parameter(circle, b);
    if (closed) u1 = u0 + 2 * M_PI;
    else if (u1 <= u0) u1 += 2 * M_PI;
    return wrap(BRepBuilderAPI_MakeEdge(circle, u0, u1).Edge());
  }
  if (all_lines) return wrap(BRepBuilderAPI_MakeEdge(a, b).Edge());
  GeomConvert_CompCurveToBSplineCurve joined;
  for (const TopoDS_Edge &e : chain) {
    double f, l;
    Handle(Geom_Curve) c = BRep_Tool::Curve(e, f, l);
    if (c.IsNull()) throw std::runtime_error("joined edge: a piece has no curve");
    Handle(Geom_TrimmedCurve) trimmed = new Geom_TrimmedCurve(c, f, l);
    if (e.Orientation() == TopAbs_REVERSED) trimmed->Reverse();
    if (!joined.Add(GeomConvert::CurveToBSplineCurve(trimmed), 1e-6)) throw std::runtime_error("joined edge: can't join");
  }
  return wrap(BRepBuilderAPI_MakeEdge(joined.BSplineCurve()).Edge());
}

// A shape with edges fused as edges_fused does.
std::unique_ptr<Shape> shape_edges_fused(const Shape &s) {
  Handle(BRepTools_History) history = new BRepTools_History;
  return wrap(edges_fused(s.shape, history));
}

// kind: 0 union, 1 subtraction, 2 intersection. `unify` merges faces and edges left on the same surface or curve.
// `fuzzy`, if positive, treats geometry closer than that as coincident.
std::unique_ptr<Operation> boolean_op(int32_t kind, const ShapeList &objects, const ShapeList &tools, bool unify,
                                      double fuzzy) {
  std::unique_ptr<BRepAlgoAPI_BooleanOperation> algo;
  switch (kind) {
    case 0: algo = std::make_unique<BRepAlgoAPI_Fuse>(); break;
    case 1: algo = std::make_unique<BRepAlgoAPI_Cut>(); break;
    case 2: algo = std::make_unique<BRepAlgoAPI_Common>(); break;
    default: throw std::invalid_argument("bad boolean kind");
  }
  TopTools_ListOfShape arguments = to_list(objects), tool_list = to_list(tools);
  algo->SetArguments(arguments);
  algo->SetTools(tool_list);
  algo->SetNonDestructive(Standard_True);
  algo->SetRunParallel(Standard_False);
  if (fuzzy > 0) algo->SetFuzzyValue(fuzzy);
  algo->Build();
  if (algo->HasErrors()) {
    std::ostringstream message;
    algo->DumpErrors(message);
    throw std::runtime_error("boolean: " + message.str());
  }
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape all = arguments;
  for (const TopoDS_Shape &s : tool_list) all.Append(s);
  op->history = new BRepTools_History(all, *algo);
  op->result = algo->Shape();
  if (unify) {
    // Faces merge only within shells: a boolean of sheets gives loose faces, so join those that share edges first.
    if (!TopExp_Explorer(op->result, TopAbs_SOLID).More() && TopExp_Explorer(op->result, TopAbs_FACE).More()) {
      std::unique_ptr<ShapeList> shells = connected_shells(Shape(op->result));
      BRep_Builder builder;
      TopoDS_Compound compound;
      builder.MakeCompound(compound);
      for (const TopoDS_Shape &shell : shells->shapes) builder.Add(compound, shell);
      op->result = compound;
    }
    ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_True, Standard_False);
    unifier.Build();
    op->history->Merge(unifier.History());
    op->result = unifier.Shape();
    op->result = edges_fused(op->result, op->history);
  }
  return op;
}

std::unique_ptr<Shape> operation_shape(const Operation &op) { return wrap(op.result); }

// One operation after another: the second's result, and what the first's inputs became through both.
std::unique_ptr<Operation> operation_then(const Operation &first, const Operation &second) {
  auto op = std::make_unique<Operation>();
  op->result = second.result;
  op->history = new BRepTools_History();
  if (!first.history.IsNull()) op->history->Merge(first.history);
  if (!second.history.IsNull()) op->history->Merge(second.history);
  return op;
}

std::unique_ptr<ShapeList> operation_modified(const Operation &op, const Shape &s) {
  return wrap_list(op.history->Modified(s.shape));
}

std::unique_ptr<ShapeList> operation_generated(const Operation &op, const Shape &s) {
  return wrap_list(op.history->Generated(s.shape));
}

bool operation_is_deleted(const Operation &op, const Shape &s) { return op.history->IsRemoved(s.shape); }

std::unique_ptr<ShapeList> operation_first(const Operation &op) {
  auto out = std::make_unique<ShapeList>();
  out->shapes = op.first;
  return out;
}

std::unique_ptr<ShapeList> operation_last(const Operation &op) {
  auto out = std::make_unique<ShapeList>();
  out->shapes = op.last;
  return out;
}

// The box around a shape. `precise` is tight to the geometry; otherwise the box may be looser, as for curved
// faces. Neither includes the shapes' tolerances.
namespace {

bool has_free_form_faces(const TopoDS_Shape &shape) {
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    switch (BRepAdaptor_Surface(TopoDS::Face(it.Current()), Standard_False).GetType()) {
      case GeomAbs_Plane:
      case GeomAbs_Cylinder:
      case GeomAbs_Cone:
      case GeomAbs_Sphere:
      case GeomAbs_Torus:
        break;
      default:
        return true;
    }
  }
  return false;
}

// Whether a face lies on a spindle torus (its tube wider than its distance from the axis), whose analytic box OCCT
// gets wrong (the guided loft's torus, batch 174 c007, boxed as if it bulged out only at its seam).
bool has_spindle_torus(const TopoDS_Shape &shape) {
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    BRepAdaptor_Surface surface(TopoDS::Face(it.Current()), Standard_False);
    if (surface.GetType() == GeomAbs_Torus && surface.Torus().MinorRadius() > surface.Torus().MajorRadius())
      return true;
  }
  return false;
}

}  // namespace

Box3 bounding_box(const Shape &s, bool precise) {
  // OCCT's boxes of B-spline faces can miss part of the face (a scaled sphere moved away from the origin loses a
  // sliver), even loose ones, so each side of a shape with such faces is measured exactly below: the distance from
  // a plane just outside the box to the shape. Its box here only has to say roughly where it is, so it's the quick
  // one: the optimal box of a B-spline face takes OCCT longer than measuring the six sides does.
  const bool measured = has_free_form_faces(s.shape) || has_spindle_torus(s.shape);
  const auto optimal = [&](Bnd_Box &box) {
    BRepBndLib::AddOptimal(s.shape, box, precise ? Standard_False : Standard_True, Standard_False);
  };
  Bnd_Box box;
  if (measured)
    BRepBndLib::Add(s.shape, box, Standard_False);
  else
    optimal(box);
  if (box.IsVoid()) return Box3{Vec3{0, 0, 0}, Vec3{0, 0, 0}, true};
  double lo[3], hi[3];
  box.Get(lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
  if (measured) {
    const double from[3] = {lo[0], lo[1], lo[2]}, to[3] = {hi[0], hi[1], hi[2]};
    double size = std::max({hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], 1e-6});
    double margin = size;
    // A side that can't be measured is the optimal box's.
    Bnd_Box fallback;
    for (int axis = 0; axis < 3; ++axis) {
      for (int side = 0; side < 2; ++side) {
        gp_Dir normal(axis == 0, axis == 1, axis == 2);
        double at = side == 0 ? from[axis] - margin : to[axis] + margin;
        gp_Pnt origin((from[0] + to[0]) / 2, (from[1] + to[1]) / 2, (from[2] + to[2]) / 2);
        origin.SetCoord(axis + 1, at);
        double reach = 4 * size;
        TopoDS_Face plane = BRepBuilderAPI_MakeFace(gp_Pln(origin, normal), -reach, reach, -reach, reach).Face();
        BRepExtrema_DistShapeShape extrema(plane, s.shape);
        if (!extrema.IsDone() || extrema.NbSolution() == 0) {
          if (fallback.IsVoid()) optimal(fallback);
          if (fallback.IsVoid()) continue;
          const gp_Pnt corner = side == 0 ? fallback.CornerMin() : fallback.CornerMax();
          (side == 0 ? lo : hi)[axis] = corner.Coord(axis + 1);
          continue;
        }
        if (side == 0)
          lo[axis] = at + extrema.Value();
        else
          hi[axis] = at - extrema.Value();
      }
    }
  }
  return Box3{Vec3{lo[0], lo[1], lo[2]}, Vec3{hi[0], hi[1], hi[2]}, false};
}

// A box found quickly from the geometry: B-splines' poles and samples of other curved faces, which can leave it a
// little short of them. It includes the shapes' tolerances.
Box3 quick_box(const Shape &s) {
  Bnd_Box box;
  BRepBndLib::Add(s.shape, box, Standard_False);
  if (box.IsVoid()) return Box3{Vec3{0, 0, 0}, Vec3{0, 0, 0}, true};
  double lo[3], hi[3];
  box.Get(lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
  return Box3{Vec3{lo[0], lo[1], lo[2]}, Vec3{hi[0], hi[1], hi[2]}, false};
}

// The box around a shape in the coordinates of `frame` (origin, z = normal, x).
Box3 bounding_box_in(const Shape &s, Frame frame, bool precise) {
  gp_Trsf to_local;
  to_local.SetTransformation(ax3(frame));
  return bounding_box(Shape(s.shape.Moved(TopLoc_Location(to_local))), precise);
}

// OCCT integrates some free-form faces badly: extrusions and revolutions of B-splines by 6e-4, closed B-spline
// faces (a scaled sphere) by 1e-5. Converting swept faces to NURBS and splitting closed ones into open patches
// first makes areas and volumes accurate to about 1e-8 (see `faces_for_integration`).
// GProp integrates over each face's curves on the face, which a boolean can leave less accurate than the edges'
// 3D curves (edges 2e-5 tolerant where a boss meets a slanted plane, 0.2 mm³ off 430 mm from the origin, batch
// 026). Those curves are projected again from the 3D ones, on a copy, as reading the shape back from STEP would.
static TopoDS_Shape with_accurate_pcurves(const TopoDS_Shape &shape) {
  bool tolerant = false;
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More() && !tolerant; e.Next())
    tolerant = BRep_Tool::Tolerance(TopoDS::Edge(e.Current())) > 1e-6;
  if (!tolerant) return shape;
  TopoDS_Shape copy = BRepBuilderAPI_Copy(shape, Standard_True).Shape();
  ShapeFix_Edge fix;
  for (TopExp_Explorer f(copy, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Edge &edge = TopoDS::Edge(e.Current());
      if (BRep_Tool::Tolerance(edge) <= 1e-6 || BRep_Tool::IsClosed(edge, face) || BRep_Tool::Degenerated(edge)) continue;
      // FixRemovePCurve only removes curves that miss the vertices.
      BRep_Builder().UpdateEdge(edge, Handle(Geom2d_Curve)(), face, BRep_Tool::Tolerance(edge));
      fix.FixAddPCurve(edge, face, Standard_False, 1e-9);
    }
  }
  // The new curves only correct a face's area slightly; where one lands a period away and the face's area changes
  // outright (a closed-corner hem's bends, batch 021 c012), keep the shape as it was.
  TopExp_Explorer before(shape, TopAbs_FACE), after(copy, TopAbs_FACE);
  for (; before.More() && after.More(); before.Next(), after.Next()) {
    GProp_GProps a, b;
    BRepGProp::SurfaceProperties(before.Current(), a);
    BRepGProp::SurfaceProperties(after.Current(), b);
    if (std::abs(b.Mass() - a.Mass()) > 1e-3 * std::abs(a.Mass()) + 1e-15) return shape;
  }
  return copy;
}

// The faces of `shape` that have to be prepared before they are integrated over, prepared, with the rest as they
// are, in a compound: all its faces for an area (each once), or for a volume (`closed`) those of its closed
// shells, which is what BRepGProp's OnlyClosed integrates over.
//
// Faces on planes, cylinders, cones, spheres and tori are integrated as they are. The others are split into open
// patches, the extruded and revolved ones converted to NURBS first, each on its own: converting a whole body
// converts its tori too, and a torus face trimmed across its seam comes out with another area (a handle swept
// along an arc and joined to a plate, 673 mm², became 830, and the part's volume 0.4% less). Offset faces aren't
// converted: OCCT approximates them, to within 1e-4 (0.1 mm in our metres) and with patches of degree 14 that
// take five times as long to integrate over. A shelled vase's inner wall, a revolved spline's offset, came out
// 0.13 mm² and the vase 1.2 mm³ off what it's integrated to as it is, however finely it's split. Neither way is
// good past 2e-5 of the volume for the offset of a loft through circles (batch 145 c005, a shelled bottle).
static TopoDS_Shape faces_for_integration(const TopoDS_Shape &shape, bool closed) {
  BRep_Builder builder;
  TopoDS_Compound all, free_form;
  builder.MakeCompound(all);
  builder.MakeCompound(free_form);
  bool any_free_form = false;
  // A face shared by two solids is converted once, and takes each one's orientation.
  TopTools_DataMapOfShapeShape converted;
  auto as_nurbs = [&](const TopoDS_Face &face) {
    const TopoDS_Shape forward = face.Oriented(TopAbs_FORWARD);
    if (!converted.IsBound(forward)) {
      TopoDS_Shape result = forward;
      BRepBuilderAPI_NurbsConvert converter(forward, Standard_True);
      if (converter.IsDone()) {
        // A conversion that changes the face's area outright went wrong: keep the face.
        GProp_GProps before, after;
        BRepGProp::SurfaceProperties(forward, before);
        BRepGProp::SurfaceProperties(converter.Shape(), after);
        if (std::abs(after.Mass() - before.Mass()) <= 1e-2 * std::abs(before.Mass())) result = converter.Shape();
      }
      converted.Bind(forward, result);
    }
    return converted.Find(forward).Oriented(face.Orientation());
  };
  auto add = [&](const TopoDS_Face &face) {
    switch (BRepAdaptor_Surface(face, Standard_False).GetType()) {
      case GeomAbs_Plane:
      case GeomAbs_Cylinder:
      case GeomAbs_Cone:
      case GeomAbs_Sphere:
      case GeomAbs_Torus:
        builder.Add(all, face);
        return;
      // Only swept faces need converting; converting some imported B-spline shapes crashes OCCT.
      case GeomAbs_SurfaceOfExtrusion:
      case GeomAbs_SurfaceOfRevolution:
        builder.Add(free_form, as_nurbs(face));
        break;
      default:
        builder.Add(free_form, face);
        break;
    }
    any_free_form = true;
  };
  if (closed) {
    for (TopExp_Explorer shells(shape, TopAbs_SHELL); shells.More(); shells.Next()) {
      if (!BRep_Tool::IsClosed(shells.Current())) continue;
      for (TopExp_Explorer faces(shells.Current(), TopAbs_FACE); faces.More(); faces.Next())
        add(TopoDS::Face(faces.Current()));
    }
  } else {
    TopTools_MapOfShape seen;
    for (TopExp_Explorer faces(shape, TopAbs_FACE); faces.More(); faces.Next())
      if (seen.Add(faces.Current())) add(TopoDS::Face(faces.Current()));
  }
  if (any_free_form) {
    TopoDS_Shape result = free_form;
    ShapeUpgrade_ShapeDivideClosed divider(result);
    divider.SetNbSplitPoints(3);
    if (divider.Perform()) result = divider.Result();
    // Adaptive integration can settle on a wrong value over a whole rational face, reporting no error: a conic
    // extruded 30 mm comes out 3.3e-5 over (batch 055 c011). Over each quarter of it in u and v it converges (to
    // 1e-11, and on a scaled sphere too).
    ShapeUpgrade_ShapeDivideArea quarters(result);
    quarters.SetSplittingByNumber(Standard_True);
    quarters.NbParts() = 16;
    quarters.SetNumbersUVSplits(4, 4);
    if (quarters.Perform()) result = quarters.Result();
    builder.Add(all, result);
  }
  return all;
}

// Volumes and areas use adaptive integration: the default Gauss rule is off by 1e-4 on rational B-spline surfaces,
// such as a scaled sphere.
constexpr double GPROP_EPS = 1e-12;

// The volume properties of a shape's closed shells, integrated until the error estimated is within `eps` of them.
static void volume_properties(const TopoDS_Shape &input, GProp_GProps &props, double eps = GPROP_EPS) {
  const TopoDS_Shape shape = with_accurate_pcurves(input);
  if (has_free_form_faces(shape))
    BRepGProp::VolumeProperties(faces_for_integration(shape, true), props, eps);
  else
    BRepGProp::VolumeProperties(shape, props, eps, Standard_True);
}

double shape_volume(const Shape &s) {
  GProp_GProps props;
  volume_properties(s.shape, props);
  return props.Mass();
}

double shape_volume_within(const Shape &s, double eps) {
  GProp_GProps props;
  volume_properties(s.shape, props, eps);
  return props.Mass();
}

double shape_area(const Shape &s) {
  GProp_GProps props;
  const TopoDS_Shape shape = with_accurate_pcurves(s.shape);
  if (has_free_form_faces(shape))
    BRepGProp::SurfaceProperties(faces_for_integration(shape, false), props, GPROP_EPS, Standard_True);
  else
    BRepGProp::SurfaceProperties(shape, props, GPROP_EPS, Standard_True);
  return props.Mass();
}

// The total length of a shape's edges, integrated adaptively: GProp's fixed rule is 0.4% off on a single-span
// cubic (batch 010, c005).
double shape_length(const Shape &s) {
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, edges);
  double total = 0;
  for (int i = 1; i <= edges.Extent(); ++i) {
    const TopoDS_Edge &edge = TopoDS::Edge(edges(i));
    if (BRep_Tool::Degenerated(edge)) continue;
    BRepAdaptor_Curve curve(edge);
    total += GCPnts_AbscissaPoint::Length(curve, 1e-12);
  }
  return total;
}

Vec3 shape_centroid(const Shape &s) {
  GProp_GProps props;
  switch (s.shape.ShapeType()) {
    case TopAbs_VERTEX: return vec3(BRep_Tool::Pnt(TopoDS::Vertex(s.shape)).XYZ());
    case TopAbs_EDGE:
    case TopAbs_WIRE: BRepGProp::LinearProperties(s.shape, props); break;
    case TopAbs_FACE:
    case TopAbs_SHELL: BRepGProp::SurfaceProperties(s.shape, props); break;
    default: BRepGProp::VolumeProperties(s.shape, props); break;
  }
  return vec3(props.CentreOfMass().XYZ());
}

// A shape's measure of the given dimension (3 volume, 2 area, 1 length), centre of mass and matrix of inertia about
// it in world axes (row by row): 13 numbers.
rust::Vec<double> shape_mass_properties(const Shape &s, int32_t dimension) {
  GProp_GProps props;
  if (dimension == 3) volume_properties(s.shape, props);
  else if (dimension == 2) BRepGProp::SurfaceProperties(s.shape, props);
  else BRepGProp::LinearProperties(s.shape, props);
  rust::Vec<double> out;
  out.push_back(props.Mass());
  gp_Pnt c = props.CentreOfMass();
  out.push_back(c.X());
  out.push_back(c.Y());
  out.push_back(c.Z());
  gp_Mat m = props.MatrixOfInertia();
  for (int i = 1; i <= 3; ++i)
    for (int j = 1; j <= 3; ++j) out.push_back(m.Value(i, j));
  return out;
}

// The principal moments of inertia about the centre of mass, smallest first.
Vec3 shape_principal_moments(const Shape &s) {
  GProp_GProps props;
  switch (s.shape.ShapeType()) {
    case TopAbs_VERTEX: return Vec3{0, 0, 0};
    case TopAbs_EDGE:
    case TopAbs_WIRE: BRepGProp::LinearProperties(s.shape, props); break;
    case TopAbs_FACE:
    case TopAbs_SHELL: BRepGProp::SurfaceProperties(s.shape, props); break;
    default: volume_properties(s.shape, props); break;
  }
  double m[3];
  props.PrincipalProperties().Moments(m[0], m[1], m[2]);
  std::sort(m, m + 3);
  return Vec3{m[0], m[1], m[2]};
}

Vec3 vertex_point(const Shape &s) { return vec3(BRep_Tool::Pnt(TopoDS::Vertex(s.shape)).XYZ()); }

int32_t surface_type(const Shape &face) {
  return static_cast<int32_t>(BRepAdaptor_Surface(TopoDS::Face(face.shape)).GetType());
}

// The face's plane, with the normal pointing out of the material.
Frame face_plane(const Shape &face) {
  BRepAdaptor_Surface surface(TopoDS::Face(face.shape));
  if (surface.GetType() != GeomAbs_Plane) throw std::runtime_error("face is not planar");
  const gp_Ax3 position = surface.Plane().Position();
  gp_Dir normal = position.Direction();
  if (face.shape.Orientation() == TopAbs_REVERSED) normal.Reverse();
  // As in Onshape, the origin is at the face's centroid, which lies on its plane.
  GProp_GProps props;
  BRepGProp::SurfaceProperties(face.shape, props);
  return Frame{vec3(props.CentreOfMass().XYZ()), vec3(normal.XYZ()), vec3(position.XDirection().XYZ())};
}

int32_t curve_type(const Shape &edge) {
  return static_cast<int32_t>(BRepAdaptor_Curve(TopoDS::Edge(edge.shape)).GetType());
}

// Whether the edge is the seam of a closed face of `root`, such as the line where a cylinder's face meets itself.
bool edge_is_seam(const Shape &root, const Shape &edge) {
  TopTools_IndexedDataMapOfShapeListOfShape map;
  TopExp::MapShapesAndUniqueAncestors(root.shape, TopAbs_EDGE, TopAbs_FACE, map);
  const TopTools_ListOfShape *faces = map.Seek(edge.shape);
  if (faces == nullptr) return false;
  for (const TopoDS_Shape &face : *faces)
    if (BRep_Tool::IsClosed(TopoDS::Edge(edge.shape), TopoDS::Face(face))) return true;
  return false;
}

// The edges (kind 6) or vertices (kind 7) Parasolid, and so Onshape, would give the shape. OCCT gives a closed face
// a seam edge, a pole a degenerate edge, and a closed edge a vertex; Parasolid has none of these, apart from a vertex
// at a pole that isn't a sphere's, such as a cone's apex.
// With `pairs`, each hidden piece of an edge goes there after the piece shown for it.
static std::unique_ptr<ShapeList> visible_sub_shapes_impl(const Shape &s, int32_t kind, ShapeList *pairs) {
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  auto visible_edge = [&](const TopoDS_Edge &edge) {
    if (BRep_Tool::Degenerated(edge)) return false;
    if (const TopTools_ListOfShape *faces = edge_faces.Seek(edge))
      for (const TopoDS_Shape &face : *faces)
        if (BRep_Tool::IsClosed(edge, TopoDS::Face(face))) return false;
    return true;
  };
  TopTools_IndexedDataMapOfShapeListOfShape vertex_edges;
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_VERTEX, TopAbs_EDGE, vertex_edges);
  // A closed edge OCCT has split at its seam vertex, where it meets a seam edge of a face: the vertex has just the
  // two pieces of the one curve as visible edges. Onshape's edge would be whole, so the vertex is hidden and the
  // pieces count as one edge.
  auto curve_of = [](const TopoDS_Edge &edge) {
    TopLoc_Location location;
    double first, last;
    return std::make_pair(BRep_Tool::Curve(edge, location, first, last), location);
  };
  TopTools_IndexedMapOfShape seam_vertices;
  for (int i = 1; i <= vertex_edges.Extent(); ++i) {
    std::vector<TopoDS_Edge> visible;
    for (const TopoDS_Shape &e : vertex_edges(i))
      if (visible_edge(TopoDS::Edge(e))) visible.push_back(TopoDS::Edge(e));
    if (visible.size() != 2 || visible[0].IsSame(visible[1])) continue;
    // Only where a seam meets them, or where the two close the curve between them (a whole circle in two): two
    // pieces of one circle meeting elsewhere are Onshape's two edges too (a sphere's meridian where an extension
    // met the old edge, batch 039 c004).
    bool seam = false;
    for (const TopoDS_Shape &e : vertex_edges(i))
      if (!visible_edge(TopoDS::Edge(e)) && !BRep_Tool::Degenerated(TopoDS::Edge(e))) seam = true;
    TopoDS_Vertex a0, a1, b0, b1;
    TopExp::Vertices(visible[0], a0, a1);
    TopExp::Vertices(visible[1], b0, b1);
    const bool closes = (a0.IsSame(b0) && a1.IsSame(b1)) || (a0.IsSame(b1) && a1.IsSame(b0));
    if (!seam && !closes) continue;
    auto a = curve_of(visible[0]), b = curve_of(visible[1]);
    if (a.first.IsNull() || b.first.IsNull()) continue;
    bool same = a.first == b.first && a.second.IsEqual(b.second) && a.first->IsPeriodic();
    if (!same) {
      // A copy (a moved body, say) gives each piece its own copy of the curve: compare circles and ellipses.
      BRepAdaptor_Curve ca(visible[0]), cb(visible[1]);
      const double tol = 1e-9;
      if (ca.GetType() == GeomAbs_Circle && cb.GetType() == GeomAbs_Circle) {
        gp_Circ x = ca.Circle(), y = cb.Circle();
        same = x.Location().Distance(y.Location()) < tol && std::abs(x.Radius() - y.Radius()) < tol &&
               x.Axis().Direction().IsParallel(y.Axis().Direction(), 1e-9);
      } else if (ca.GetType() == GeomAbs_Ellipse && cb.GetType() == GeomAbs_Ellipse) {
        gp_Elips x = ca.Ellipse(), y = cb.Ellipse();
        same = x.Location().Distance(y.Location()) < tol && std::abs(x.MajorRadius() - y.MajorRadius()) < tol &&
               std::abs(x.MinorRadius() - y.MinorRadius()) < tol &&
               x.Axis().Direction().IsParallel(y.Axis().Direction(), 1e-9) &&
               x.XAxis().Direction().IsParallel(y.XAxis().Direction(), 1e-9);
      }
    }
    // Two pieces of an intersection curve (B-splines) a seam meets, between the same two faces and running on
    // smoothly: one edge in Parasolid (crossing pipes' joints cross both pipes' seams, batch 128 c007).
    if (!same && seam) {
      const TopTools_ListOfShape *fa = edge_faces.Seek(visible[0]), *fb = edge_faces.Seek(visible[1]);
      bool faces_same = fa && fb && fa->Extent() == 2 && fb->Extent() == 2;
      if (faces_same)
        for (const TopoDS_Shape &f : *fa) {
          bool found = false;
          for (const TopoDS_Shape &g : *fb) found = found || f.IsSame(g);
          faces_same = faces_same && found;
        }
      if (faces_same) {
        const TopoDS_Vertex &v = TopoDS::Vertex(vertex_edges.FindKey(i));
        auto tangent = [&](const TopoDS_Edge &e) {
          BRepAdaptor_Curve c(e);
          double t = BRep_Tool::Parameter(v, e);
          gp_Pnt p;
          gp_Vec d;
          c.D1(t, p, d);
          return d;
        };
        gp_Vec ta = tangent(visible[0]), tb = tangent(visible[1]);
        // (Within a thousandth of a radian: a blend lofted round a loop meets itself a few ten-thousandths askew.)
        same = ta.Magnitude() > 1e-12 && tb.Magnitude() > 1e-12 && ta.IsParallel(tb, 1e-3);
      }
    }
    if (same) seam_vertices.Add(vertex_edges.FindKey(i));
  }
  auto out = std::make_unique<ShapeList>();
  if (kind == TopAbs_EDGE) {
    TopTools_IndexedMapOfShape edges;
    TopExp::MapShapes(s.shape, TopAbs_EDGE, edges);
    // Pieces joined at a seam vertex: only the first of them is shown.
    TopTools_IndexedMapOfShape hidden;
    for (int i = 1; i <= edges.Extent(); ++i) {
      const TopoDS_Edge &edge = TopoDS::Edge(edges(i));
      if (hidden.Contains(edge) || !visible_edge(edge)) continue;
      // Each edge in its own direction, as a Parasolid edge has one, not in the direction a face first used it.
      out->shapes.push_back(edge.Oriented(TopAbs_FORWARD));
      std::vector<TopoDS_Edge> stack{edge};
      while (!stack.empty()) {
        TopoDS_Edge current = stack.back();
        stack.pop_back();
        for (TopoDS_Iterator it(current); it.More(); it.Next()) {
          if (!seam_vertices.Contains(it.Value())) continue;
          for (const TopoDS_Shape &other : vertex_edges.FindFromKey(it.Value()))
            if (!other.IsSame(edge) && !hidden.Contains(other) && visible_edge(TopoDS::Edge(other))) {
              hidden.Add(other);
              stack.push_back(TopoDS::Edge(other));
              if (pairs) {
                pairs->shapes.push_back(edge.Oriented(TopAbs_FORWARD));
                pairs->shapes.push_back(other);
              }
            }
        }
      }
    }
    return out;
  }
  if (kind != TopAbs_VERTEX) throw std::invalid_argument("visible_sub_shapes: edges or vertices only");
  for (int i = 1; i <= vertex_edges.Extent(); ++i) {
    if (seam_vertices.Contains(vertex_edges.FindKey(i))) continue;
    const TopTools_ListOfShape &edges = vertex_edges(i);
    bool keep = edges.IsEmpty();  // a point
    for (const TopoDS_Shape &e : edges) {
      const TopoDS_Edge &edge = TopoDS::Edge(e);
      if (BRep_Tool::Degenerated(edge)) {
        if (const TopTools_ListOfShape *faces = edge_faces.Seek(edge))
          for (const TopoDS_Shape &face : *faces)
            if (BRepAdaptor_Surface(TopoDS::Face(face)).GetType() != GeomAbs_Sphere) keep = true;
      } else if (visible_edge(edge)) {
        TopoDS_Vertex first, last;
        TopExp::Vertices(edge, first, last);
        if (!first.IsSame(last)) keep = true;
      }
      if (keep) break;
    }
    if (keep) out->shapes.push_back(vertex_edges.FindKey(i));
  }
  return out;
}

std::unique_ptr<ShapeList> visible_sub_shapes(const Shape &s, int32_t kind) {
  return visible_sub_shapes_impl(s, kind, nullptr);
}

std::unique_ptr<ShapeList> hidden_edge_pieces(const Shape &s) {
  auto pairs = std::make_unique<ShapeList>();
  visible_sub_shapes_impl(s, TopAbs_EDGE, pairs.get());
  return pairs;
}

bool shape_valid(const Shape &s) { return BRepCheck_Analyzer(s.shape).IsValid(); }

// The largest tolerance of a shape's edges and vertices (0 for a shape with none).
double shape_max_tolerance(const Shape &s) {
  double out = 0;
  for (TopExp_Explorer e(s.shape, TopAbs_EDGE); e.More(); e.Next())
    out = std::max(out, BRep_Tool::Tolerance(TopoDS::Edge(e.Current())));
  for (TopExp_Explorer v(s.shape, TopAbs_VERTEX); v.More(); v.Next())
    out = std::max(out, BRep_Tool::Tolerance(TopoDS::Vertex(v.Current())));
  return out;
}

// Writes AP242 in metres, naming each shape's product and, as Onshape does, its MANIFOLD_SOLID_BREP. The solids
// are named in the order they were transferred, so each shape must be a single solid for its name to land on it.
// A shape's faces meshed to within `deflection`, each triangle's corners in turn, wound out of the material.
rust::Vec<double> mesh_triangles(const Shape &s, double deflection) {
  TopoDS_Shape shape = s.shape;
  BRepMesh_IncrementalMesh mesher(shape, deflection, Standard_False, 0.3, Standard_False);
  rust::Vec<double> out;
  for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
    const TopoDS_Face &face = TopoDS::Face(ex.Current());
    TopLoc_Location location;
    Handle(Poly_Triangulation) mesh = BRep_Tool::Triangulation(face, location);
    if (mesh.IsNull()) continue;
    const gp_Trsf trsf = location.Transformation();
    const bool reversed = face.Orientation() == TopAbs_REVERSED;
    for (int i = 1; i <= mesh->NbTriangles(); ++i) {
      int a, b, c;
      mesh->Triangle(i).Get(a, b, c);
      if (reversed) std::swap(b, c);
      for (int n : {a, b, c}) {
        gp_Pnt p = mesh->Node(n).Transformed(trsf);
        out.push_back(p.X());
        out.push_back(p.Y());
        out.push_back(p.Z());
      }
    }
  }
  return out;
}

void write_step(const ShapeList &shapes, const rust::Vec<rust::String> &names, rust::Str path) {
  if (shapes.shapes.size() != names.size()) throw std::invalid_argument("one name per shape");
  quiet();
  Handle(TDocStd_Document) document;
  XCAFApp_Application::GetApplication()->NewDocument("MDTV-XCAF", document);
  // Our shapes are in metres; without this the writer takes them as millimetres and scales them down.
  XCAFDoc_DocumentTool::SetLengthUnit(document, 1.0);
  Handle(XCAFDoc_ShapeTool) tool = XCAFDoc_DocumentTool::ShapeTool(document->Main());
  for (size_t i = 0; i < shapes.shapes.size(); ++i) {
    TDF_Label label = tool->AddShape(shapes.shapes[i], Standard_False);
    TDataStd_Name::Set(label, TCollection_ExtendedString(std::string(names[i]).c_str(), Standard_True));
  }
  STEPCAFControl_Writer writer;
  Interface_Static::SetCVal("write.step.schema", "AP242DIS");
  Interface_Static::SetCVal("write.step.unit", "M");
  writer.SetNameMode(Standard_True);
  if (!writer.Transfer(document, STEPControl_AsIs)) throw std::runtime_error("STEP transfer failed");
  Handle(StepData_StepModel) model = writer.ChangeWriter().Model();
  std::vector<Handle(StepShape_ManifoldSolidBrep)> solids;
  for (int i = 1; i <= model->NbEntities(); ++i) {
    Handle(StepShape_ManifoldSolidBrep) brep = Handle(StepShape_ManifoldSolidBrep)::DownCast(model->Value(i));
    if (!brep.IsNull()) solids.push_back(brep);
  }
  if (solids.size() == names.size())
    for (size_t i = 0; i < solids.size(); ++i) solids[i]->SetName(new TCollection_HAsciiString(std::string(names[i]).c_str()));
  if (writer.Write(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("STEP write failed");
  XCAFApp_Application::GetApplication()->Close(document);
}

// Reads every MANIFOLD_SOLID_BREP of a STEP file, in metres, with the name written on it.
std::unique_ptr<ShapeList> read_step(rust::Str path, rust::Vec<rust::String> &names) {
  quiet();
  STEPControl_Reader reader;
  if (reader.ReadFile(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("STEP read failed");
  // The system unit is in millimetres, and setting it only takes effect once a file is loaded.
  reader.SetSystemLengthUnit(1000.0);
  reader.TransferRoots();
  Handle(Transfer_TransientProcess) process = reader.WS()->TransferReader()->TransientProcess();
  Handle(Interface_InterfaceModel) model = reader.Model();
  auto out = std::make_unique<ShapeList>();
  for (int i = 1; i <= model->NbEntities(); ++i) {
    Handle(StepShape_ManifoldSolidBrep) brep = Handle(StepShape_ManifoldSolidBrep)::DownCast(model->Value(i));
    if (brep.IsNull()) continue;
    TopoDS_Shape shape = TransferBRep::ShapeResult(process, brep);
    if (shape.IsNull()) continue;
    out->shapes.push_back(shape);
    names.push_back(brep->Name().IsNull() ? "" : brep->Name()->ToCString());
  }
  return out;
}

}  // namespace occt
