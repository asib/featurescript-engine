// Modelling operations beyond extrude and booleans: revolve, primitives, splitting and general transforms.

#include "occt/cpp/occt.h"

#include <BRepGProp.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <ElSLib.hxx>
#include <BRepGProp_Face.hxx>
#include <IntTools_Context.hxx>
#include <BOPTools_AlgoTools3D.hxx>
#include <BOPAlgo_MakerVolume.hxx>
#include <BRepFill.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <GProp_GProps.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopoDS_Compound.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepOffsetAPI_MakePipe.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BezierCurve.hxx>
#include <TColStd_Array2OfReal.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <BSplCLib.hxx>
#include <math_Gauss.hxx>
#include <math_Vector.hxx>
#include <math_Matrix.hxx>
#include <array>
#include <TColStd_Array1OfInteger.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <Extrema_ExtPC.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <GeomConvert.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRepTools_ReShape.hxx>
#include <ShapeAnalysis_CanonicalRecognition.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Defeaturing.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <Geom_OffsetSurface.hxx>
#include <GeomLProp_SLProps.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <gp_Pnt2d.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <Law_Linear.hxx>
#include <Law_S.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepFeat_SplitShape.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <ShapeFix_Solid.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Shell.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepLib.hxx>
#include <GeomProjLib.hxx>
#include <Geom2d_Curve.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <BRepAdaptor_CompCurve.hxx>
#include <TColStd_HArray1OfReal.hxx>
#include <TColgp_HArray1OfPnt.hxx>
#include <GeomFill.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <TopTools_DataMapOfShapeInteger.hxx>
#include <Standard_Failure.hxx>
#include <BRepLProp_CLProps.hxx>
#include <BRepBuilderAPI_FindPlane.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <Geom_Plane.hxx>
#include <ShapeFix_Edge.hxx>
#include <ShapeFix_ShapeTolerance.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_NurbsConvert.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepOffsetAPI_DraftAngle.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <ShapeFix_Shape.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_Pln.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepTools_History.hxx>
#include <BRepTools_Modifier.hxx>
#include <ShapeCustom_DirectModification.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_MapOfShape.hxx>
#include <gp_Ax1.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Trsf.hxx>

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <sstream>

#include "occt/src/lib.rs.h"
#include "occt/cpp/internal.h"

namespace occt {

// Revolves a profile `angle` radians about an axis, right-handed. For less than a full turn, `first` and `last`
// hold the faces at the start and end of the sweep.
std::unique_ptr<Operation> revolve(const Shape &profile, Axis axis, double angle) {
  gp_Ax1 ax(pnt(axis.origin), dir(axis.direction));
  BRepPrimAPI_MakeRevol maker(profile.shape, ax, angle, Standard_False);
  maker.Build();
  check_done(maker, "revolve");
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape arguments;
  arguments.Append(profile.shape);
  op->history = new BRepTools_History(arguments, maker);
  op->result = maker.Shape();
  // Edges left on one curve, such as the two halves of a circle where a semicircle revolved half a turn meets
  // itself, become one edge, as in Onshape.
  {
    ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_False, Standard_False);
    unifier.Build();
    op->history->Merge(unifier.History());
    op->result = unifier.Shape();
  }
  if (std::abs(angle) < 2 * M_PI - 1e-12) {
    TopTools_IndexedMapOfShape faces;
    TopExp::MapShapes(profile.shape, TopAbs_FACE, faces);
    for (int i = 1; i <= faces.Extent(); ++i) {
      op->first.push_back(maker.FirstShape(faces(i)));
      op->last.push_back(maker.LastShape(faces(i)));
    }
  }
  return op;
}

std::unique_ptr<Shape> make_sphere(Vec3 center, double radius) {
  BRepPrimAPI_MakeSphere maker(pnt(center), radius);
  maker.Build();
  check_done(maker, "sphere");
  return wrap(maker.Solid());
}

// Splits `objects` with `tools` (faces, sheets or solids). The result holds every piece of every object.
std::unique_ptr<Operation> split_op(const ShapeList &objects, const ShapeList &tools) {
  BRepAlgoAPI_Splitter splitter;
  TopTools_ListOfShape arguments = to_list(objects), tool_list = to_list(tools);
  splitter.SetArguments(arguments);
  splitter.SetTools(tool_list);
  splitter.SetNonDestructive(Standard_True);
  splitter.SetRunParallel(Standard_False);
  splitter.Build();
  if (splitter.HasErrors()) {
    std::ostringstream message;
    splitter.DumpErrors(message);
    throw std::runtime_error("split: " + message.str());
  }
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape all = arguments;
  for (const TopoDS_Shape &s : tool_list) all.Append(s);
  op->history = new BRepTools_History(all, splitter);
  op->result = splitter.Shape();
  // A seam of an object's closed face that the split left between two faces (a cylinder cut through its axis: the
  // half holding the seam in two faces) is merged away, as Parasolid has no seams (batch 122 c019).
  TopTools_IndexedMapOfShape seams;
  for (const TopoDS_Shape &o : arguments)
    for (TopExp_Explorer f(o, TopAbs_FACE); f.More(); f.Next())
      for (TopExp_Explorer e(f.Current(), TopAbs_EDGE); e.More(); e.Next())
        if (BRep_Tool::IsClosed(TopoDS::Edge(e.Current()), TopoDS::Face(f.Current()))) seams.Add(e.Current());
  if (seams.IsEmpty()) return op;
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(op->result, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  TopTools_MapOfShape removable;
  for (int i = 1; i <= seams.Extent(); ++i) {
    TopTools_ListOfShape images = op->history->Modified(seams(i));
    if (images.IsEmpty() && !op->history->IsRemoved(seams(i))) images.Append(seams(i));
    for (const TopoDS_Shape &image : images) {
      if (!edge_faces.Contains(image)) continue;
      const TopTools_ListOfShape &faces = edge_faces.FindFromKey(image);
      if (faces.Extent() != 2) continue;
      const TopoDS_Face &a = TopoDS::Face(faces.First()), &b = TopoDS::Face(faces.Last());
      if (BRep_Tool::Surface(a) == BRep_Tool::Surface(b)) removable.Add(image);
    }
  }
  if (removable.IsEmpty()) return op;
  TopTools_MapOfShape keep;
  for (TopExp_Explorer it(op->result, TopAbs_EDGE); it.More(); it.Next())
    if (!removable.Contains(it.Current())) keep.Add(it.Current());
  ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_True, Standard_False);
  unifier.KeepShapes(keep);
  unifier.Build();
  op->history->Merge(unifier.History());
  op->result = unifier.Shape();
  return op;
}

namespace {

// Whether the 3×3 part of `t` is a rotation (or reflection) times a uniform scale.
bool is_similarity(const Transform &t) {
  double m[3][3];
  for (int i = 0; i < 3; ++i)
    for (int j = 0; j < 3; ++j) m[i][j] = t.m[i * 4 + j];
  double scale2 = m[0][0] * m[0][0] + m[1][0] * m[1][0] + m[2][0] * m[2][0];
  for (int i = 0; i < 3; ++i)
    for (int j = 0; j < 3; ++j) {
      double dot = m[0][i] * m[0][j] + m[1][i] * m[1][j] + m[2][i] * m[2][j];
      double expected = i == j ? scale2 : 0.0;
      if (std::abs(dot - expected) > 1e-12 * std::max(1.0, scale2)) return false;
    }
  return scale2 > 0;
}

}  // namespace

// Transforms a copy of a shape. Similarities (rotations, reflections, translations, uniform scales) keep the
// geometry exact; other affine maps, such as a non-uniform scale, turn it into B-splines.
namespace {
TopoDS_Shape planes_recognized(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history);
TopoDS_Shape pcurves_reprojected(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history);
}

std::unique_ptr<Operation> transform_op(const Shape &s, const Transform &t) {
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape arguments;
  arguments.Append(s.shape);
  if (is_similarity(t)) {
    gp_Trsf trsf;
    trsf.SetValues(t.m[0], t.m[1], t.m[2], t.m[3], t.m[4], t.m[5], t.m[6], t.m[7], t.m[8], t.m[9], t.m[10], t.m[11]);
    BRepBuilderAPI_Transform maker(s.shape, trsf, Standard_True);
    check_done(maker, "transform");
    op->history = new BRepTools_History(arguments, maker);
    op->result = maker.Shape();
    if (trsf.IsNegative()) {
      // A mirror leaves surfaces with left-handed axes, which won't merge with coplanar neighbours (a sheet
      // metal tab turned over onto its wall, batch 013): make them right-handed, keeping where everything went.
      BRepTools_Modifier modifier(op->result, new ShapeCustom_DirectModification);
      if (modifier.IsDone()) {
        Handle(BRepTools_History) direct = new BRepTools_History;
        TopTools_IndexedMapOfShape subs;
        TopExp::MapShapes(op->result, subs);
        for (int i = 1; i <= subs.Extent(); ++i) {
          const TopoDS_Shape &image = modifier.ModifiedShape(subs(i));
          if (!image.IsSame(subs(i))) direct->AddModified(subs(i), image);
        }
        op->history->Merge(direct);
        op->result = modifier.ModifiedShape(op->result);
      }
    }
  } else {
    gp_GTrsf gtrsf;
    for (int i = 0; i < 3; ++i) {
      for (int j = 0; j < 3; ++j) gtrsf.SetValue(i + 1, j + 1, t.m[i * 4 + j]);
      gtrsf.SetValue(i + 1, 4, t.m[i * 4 + 3]);
    }
    BRepBuilderAPI_GTransform maker(s.shape, gtrsf, Standard_True);
    check_done(maker, "transform");
    op->history = new BRepTools_History(arguments, maker);
    // A general transform turns every surface into a B-spline; planes stay planes in Parasolid.
    op->result = pcurves_reprojected(planes_recognized(maker.Shape(), op->history), op->history);
  }
  return op;
}

namespace {

std::unique_ptr<Operation> finish(const TopTools_ListOfShape &arguments, BRepBuilderAPI_MakeShape &maker,
                                  const char *what) {
  if (!maker.IsDone()) throw std::runtime_error(std::string(what) + " failed");
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History(arguments, maker);
  op->result = maker.Shape();
  if (op->result.IsNull()) throw std::runtime_error(std::string(what) + " made nothing");
  return op;
}

TopTools_ListOfShape one(const TopoDS_Shape &s) {
  TopTools_ListOfShape l;
  l.Append(s);
  return l;
}

// The outward normal of a face at the middle of its parameter range.
gp_Dir face_normal(const TopoDS_Face &face) {
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  BRepAdaptor_Surface surface(face);
  BRepLProp_SLProps props(surface, (u0 + u1) / 2, (v0 + v1) / 2, 1, 1e-9);
  if (!props.IsNormalDefined()) throw std::runtime_error("no normal");
  gp_Dir n = props.Normal();
  if (face.Orientation() == TopAbs_REVERSED) n.Reverse();
  return n;
}

}  // namespace

namespace {

double largest_tolerance(const TopoDS_Shape &s) {
  double t = 0;
  for (TopExp_Explorer e(s, TopAbs_EDGE); e.More(); e.Next()) t = std::max(t, BRep_Tool::Tolerance(TopoDS::Edge(e.Current())));
  for (TopExp_Explorer v(s, TopAbs_VERTEX); v.More(); v.Next())
    t = std::max(t, BRep_Tool::Tolerance(TopoDS::Vertex(v.Current())));
  return t;
}

// A circular fillet as part of the solid's morphological opening (eroded by the radius, then grown back by it with
// round joins: every ball of that radius inside it): the slivers the opening takes off the solid, those along
// the edges, cut away. Only for closed edges, whose slivers are rings of their own. Null if it can't be made.
std::unique_ptr<Operation> fillet_by_opening(const TopoDS_Shape &solid, const TopTools_ListOfShape &edges, double r) {
  // The edges must close up into loops: every vertex met an even number of times.
  TopTools_IndexedDataMapOfShapeListOfShape vertex_edges;
  for (const TopoDS_Shape &e : edges) TopExp::MapShapesAndAncestors(e, TopAbs_VERTEX, TopAbs_EDGE, vertex_edges);
  {
    TopTools_DataMapOfShapeInteger count;
    for (const TopoDS_Shape &e : edges) {
      TopoDS_Vertex a, b;
      TopExp::Vertices(TopoDS::Edge(e), a, b);
      if (a.IsNull() || b.IsNull()) return nullptr;
      for (const TopoDS_Vertex &v : {a, b}) {
        if (count.IsBound(v)) count.ChangeFind(v) += 1;
        else count.Bind(v, 1);
      }
    }
    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(count); it.More(); it.Next())
      if (it.Value() % 2 != 0) return nullptr;
  }
  try {
    // The slivers a solid's opening takes off it, those touching the edges.
    auto slivers_along = [&](const TopoDS_Shape &shape) -> TopTools_ListOfShape {
      TopTools_ListOfShape chosen;
      // (Intersection mode with round joins: a cross hole's cylinder with plain joins comes back empty.)
      BRepOffsetAPI_MakeOffsetShape eroded;
      eroded.PerformByJoin(BRepBuilderAPI_Copy(shape, Standard_True).Shape(), -r, 1e-7, BRepOffset_Skin, Standard_True,
                           Standard_False, GeomAbs_Arc);
      if (!eroded.IsDone() || eroded.Shape().IsNull() || !BRepCheck_Analyzer(eroded.Shape()).IsValid()) return chosen;
      // Its intersection curves one edge each, so each round grown back along one is one face.
      Handle(BRepTools_History) scratch = new BRepTools_History;
      const TopoDS_Shape core = edges_fused(eroded.Shape(), scratch);
      BRepOffsetAPI_MakeOffsetShape opened;
      opened.PerformByJoin(core, r, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc);
      if (!opened.IsDone() || opened.Shape().IsNull() || !BRepCheck_Analyzer(opened.Shape()).IsValid()) return chosen;
      BRepAlgoAPI_Cut slivers(shape, opened.Shape());
      slivers.Build();
      if (!slivers.IsDone()) return chosen;
      for (TopExp_Explorer it(slivers.Shape(), TopAbs_SOLID); it.More(); it.Next()) {
        for (const TopoDS_Shape &e : edges) {
          BRepExtrema_DistShapeShape distance(it.Current(), e);
          if (distance.IsDone() && distance.Value() < 1e-6) {
            chosen.Append(it.Current());
            break;
          }
        }
      }
      return chosen;
    };
    // Convex edges lose what the solid's opening takes off along them; concave ones gain what its closing adds
    // there (grown by the radius with sharp joins, so the edge stays an edge, then shrunk back with round ones, which
    // round it).
    bool concave = false;
    TopTools_ListOfShape chosen = slivers_along(solid);
    if (chosen.IsEmpty()) {
      concave = true;
      BRepOffsetAPI_MakeOffsetShape grown;
      grown.PerformByJoin(BRepBuilderAPI_Copy(solid, Standard_True).Shape(), r, 1e-7, BRepOffset_Skin, Standard_True,
                          Standard_False, GeomAbs_Intersection);
      if (!grown.IsDone() || grown.Shape().IsNull()) return nullptr;
      TopoDS_Shape grown_shape = grown.Shape();
      // (Crossing pipes grown come out with a face's wires out of order: mended.)
      if (!BRepCheck_Analyzer(grown_shape).IsValid()) {
        Handle(ShapeFix_Shape) fix = new ShapeFix_Shape(grown_shape);
        fix->Perform();
        grown_shape = fix->Shape();
        if (!BRepCheck_Analyzer(grown_shape).IsValid()) return nullptr;
      }
      Handle(BRepTools_History) scratch = new BRepTools_History;
      const TopoDS_Shape hull = edges_fused(grown_shape, scratch);
      BRepOffsetAPI_MakeOffsetShape closed;
      closed.PerformByJoin(hull, -r, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc);
      if (!closed.IsDone() || closed.Shape().IsNull() || !BRepCheck_Analyzer(closed.Shape()).IsValid()) return nullptr;
      BRepAlgoAPI_Cut added(closed.Shape(), solid);
      added.Build();
      if (!added.IsDone()) return nullptr;
      for (TopExp_Explorer it(added.Shape(), TopAbs_SOLID); it.More(); it.Next()) {
        for (const TopoDS_Shape &e : edges) {
          BRepExtrema_DistShapeShape distance(it.Current(), e);
          if (distance.IsDone() && distance.Value() < 1e-6) {
            chosen.Append(it.Current());
            break;
          }
        }
      }
    }
    if (chosen.IsEmpty()) return nullptr;
    // A sliver running along an edge not given rounds that edge too: the slivers along a knob's notched top joined
    // through its notches' corners down to its bottom (batch 145 c008). Not this way, then.
    {
      TopTools_MapOfShape given;
      for (const TopoDS_Shape &e : edges) given.Add(e);
      for (TopExp_Explorer it(solid, TopAbs_EDGE); it.More(); it.Next()) {
        if (given.Contains(it.Current()) || BRep_Tool::Degenerated(TopoDS::Edge(it.Current()))) continue;
        BRepAdaptor_Curve curve(TopoDS::Edge(it.Current()));
        const TopoDS_Vertex middle =
            BRepBuilderAPI_MakeVertex(curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2));
        for (const TopoDS_Shape &sliver : chosen) {
          BRepExtrema_DistShapeShape distance(middle, sliver);
          if (distance.IsDone() && distance.Value() < 1e-6) return nullptr;
        }
      }
    }
    std::unique_ptr<BRepAlgoAPI_BooleanOperation> boolean;
    if (concave) boolean = std::make_unique<BRepAlgoAPI_Fuse>(); else boolean = std::make_unique<BRepAlgoAPI_Cut>();
    boolean->SetArguments(one(solid));
    boolean->SetTools(chosen);
    boolean->Build();
    if (!boolean->IsDone() || !BRepCheck_Analyzer(boolean->Shape()).IsValid()) return nullptr;
    auto op = finish(one(solid), *boolean, "fillet");
    ShapeUpgrade_UnifySameDomain unify(op->result, Standard_True, Standard_True, Standard_False);
    unify.Build();
    op->history->Merge(unify.History());
    op->result = unify.Shape();
    return op;
  } catch (const Standard_Failure &) {
    return nullptr;
  }
}

}  // namespace

namespace {
std::unique_ptr<Operation> fillet_by_sections(const TopoDS_Shape &solid, const TopTools_ListOfShape &edges, double r);
}

namespace {

// A circular fillet of straight edges between planes, each edge's rounding a tool of its own: the region between its
// two faces and the ball rolled along it (its cross section the corner between the faces' feet and the ball's arc),
// carried along the edge and past its ends, then cut back by the planes of the other faces at each end, on the side
// the edge is. The tools are cut from the solid (convex edges) or added to it (concave ones), so roundings meeting at
// a corner meet along where they cross, as Onshape's do (batch 147 c004: a drafted pocket's floor, where OCCT patches
// some corners). Null if an edge isn't straight between planes, three edges given meet at a vertex, or it fails.
std::unique_ptr<Operation> fillet_by_tools(const TopoDS_Shape &solid, const TopTools_ListOfShape &edges, double r) {
  try {
    TopTools_IndexedDataMapOfShapeListOfShape edge_faces, vertex_faces;
    TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_VERTEX, TopAbs_FACE, vertex_faces);
    {
      TopTools_DataMapOfShapeInteger count;
      for (const TopoDS_Shape &e : edges)
        for (TopExp_Explorer v(e, TopAbs_VERTEX); v.More(); v.Next()) {
          if (count.IsBound(v.Current())) count.ChangeFind(v.Current()) += 1;
          else count.Bind(v.Current(), 1);
          if (count.Find(v.Current()) > 2) return nullptr;
        }
    }
    auto plane_of = [](const TopoDS_Shape &f, gp_Pln &out) {
      BRepAdaptor_Surface surface(TopoDS::Face(f));
      if (surface.GetType() != GeomAbs_Plane) return false;
      out = surface.Plane();
      return true;
    };
    TopTools_ListOfShape cuts, fills;
    for (const TopoDS_Shape &e : edges) {
      const TopoDS_Edge &edge = TopoDS::Edge(e);
      BRepAdaptor_Curve curve(edge);
      if (curve.GetType() != GeomAbs_Line) return nullptr;
      const TopTools_ListOfShape *faces = edge_faces.Seek(edge);
      if (faces == nullptr || faces->Size() != 2) return nullptr;
      const TopoDS_Face f1 = TopoDS::Face(faces->First()), f2 = TopoDS::Face(faces->Last());
      gp_Pln p1, p2;
      if (!plane_of(f1, p1) || !plane_of(f2, p2)) return nullptr;
      const gp_Pnt a = curve.Value(curve.FirstParameter()), b = curve.Value(curve.LastParameter());
      const gp_Pnt m((a.XYZ() + b.XYZ()) / 2);
      const gp_Vec d = gp_Vec(a, b).Normalized();
      const gp_Vec n1(face_normal(f1)), n2(face_normal(f2));
      const gp_Vec sum = n1 + n2;
      if (sum.Magnitude() < 1e-9) return nullptr;
      // Convex where just behind one face and in front of the other is outside the solid (the solid there is what's
      // behind both faces; at a concave edge, what's behind either).
      BRepClass3d_SolidClassifier classifier(solid, m.Translated((n2 - n1) * (1e-4 * r)), 1e-9);
      const bool convex = classifier.State() == TopAbs_OUT;
      const double half = n1.Angle(n2) / 2;
      const gp_Vec towards = (convex ? -1.0 : 1.0) * sum.Normalized();
      const gp_Pnt centre = m.Translated(towards * (r / std::cos(half)));
      const double sign = convex ? 1.0 : -1.0;
      const gp_Pnt foot1 = centre.Translated(n1 * (sign * r)), foot2 = centre.Translated(n2 * (sign * r));
      // The cross section at the edge's middle, carried back past its start and on past its end.
      const double margin = 4 * r + gp_Vec(a, b).Magnitude();
      const gp_Trsf back = [&] { gp_Trsf t; t.SetTranslation(-d * margin); return t; }();
      const gp_Pnt e0 = m.Transformed(back), g1 = foot1.Transformed(back), g2 = foot2.Transformed(back);
      const gp_Pnt mid = centre.Translated(-towards * r).Transformed(back);
      Handle(Geom_TrimmedCurve) arc = GC_MakeArcOfCircle(g1, mid, g2).Value();
      BRepBuilderAPI_MakeWire wire;
      wire.Add(BRepBuilderAPI_MakeEdge(e0, g1).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(arc).Edge());
      wire.Add(BRepBuilderAPI_MakeEdge(g2, e0).Edge());
      if (!wire.IsDone()) return nullptr;
      BRepBuilderAPI_MakeFace profile(wire.Wire(), Standard_True);
      if (!profile.IsDone()) return nullptr;
      TopoDS_Shape tool = BRepPrimAPI_MakePrism(profile.Face(), d * (2 * margin)).Shape();
      // Cut back by the other faces' planes at each end, keeping the side the edge is on.
      for (const gp_Pnt &end : {a, b}) {
        TopoDS_Vertex vertex;
        for (TopExp_Explorer v(edge, TopAbs_VERTEX); v.More(); v.Next())
          if (BRep_Tool::Pnt(TopoDS::Vertex(v.Current())).Distance(end) < 1e-9) vertex = TopoDS::Vertex(v.Current());
        const TopTools_ListOfShape *around = vertex.IsNull() ? nullptr : vertex_faces.Seek(vertex);
        if (around == nullptr) return nullptr;
        for (const TopoDS_Shape &g : *around) {
          if (g.IsSame(f1) || g.IsSame(f2)) continue;
          gp_Pln pg;
          if (!plane_of(g, pg)) return nullptr;
          TopoDS_Face big = BRepBuilderAPI_MakeFace(pg, -1e3 * margin, 1e3 * margin, -1e3 * margin, 1e3 * margin).Face();
          BRepAlgoAPI_Common common(tool, BRepPrimAPI_MakeHalfSpace(big, m).Solid());
          common.SetRunParallel(Standard_False);
          common.Build();
          if (!common.IsDone()) return nullptr;
          tool = common.Shape();
        }
      }
      (convex ? cuts : fills).Append(tool);
    }
    TopoDS_Shape current = solid;
    auto op = std::make_unique<Operation>();
    op->history = new BRepTools_History();
    for (int pass = 0; pass < 2; ++pass) {
      const TopTools_ListOfShape &tools = pass == 0 ? cuts : fills;
      if (tools.IsEmpty()) continue;
      std::unique_ptr<BRepAlgoAPI_BooleanOperation> boolean;
      if (pass == 0) boolean = std::make_unique<BRepAlgoAPI_Cut>(); else boolean = std::make_unique<BRepAlgoAPI_Fuse>();
      boolean->SetArguments(one(current));
      boolean->SetTools(tools);
      boolean->SetRunParallel(Standard_False);
      boolean->Build();
      if (!boolean->IsDone()) return nullptr;
      TopTools_ListOfShape arguments = one(current);
      for (const TopoDS_Shape &t : tools) arguments.Append(t);
      op->history->Merge(arguments, *boolean);
      current = boolean->Shape();
    }
    ShapeUpgrade_UnifySameDomain unify(current, Standard_True, Standard_True, Standard_False);
    unify.Build();
    op->history->Merge(unify.History());
    // Where two roundings cross, one edge: the tools' cut-backs can leave it in pieces.
    op->result = edges_fused(unify.Shape(), op->history);
    TopExp_Explorer only(op->result, TopAbs_SOLID);
    if (!only.More()) return nullptr;
    op->result = only.Current();
    only.Next();
    if (only.More() || !BRepCheck_Analyzer(op->result).IsValid()) return nullptr;
    return op;
  } catch (const Standard_Failure &) {
    return nullptr;
  }
}

}  // namespace

// Rounds edges of a solid with a constant radius. Where OCCT's fillet comes out with edges far out of tolerance
// (round a cross hole's mouth, two lopsided faces with edges 0.7 mm out, batch 125 c008), closed edges are rounded
// as part of the solid's morphological opening instead.
std::unique_ptr<Operation> fillet_op(const Shape &solid, const ShapeList &edges, double radius) {
  // A copy first: a fillet gone wrong can leave the solid it was given damaged.
  BRepBuilderAPI_Copy copier(solid.shape, Standard_True);
  BRepFilletAPI_MakeFillet maker(solid.shape);
  for (const TopoDS_Shape &e : edges.shapes) maker.Add(radius, TopoDS::Edge(e));
  maker.Build();
  // Also where it rounds an edge with more than one face (crossing pipes' joints in three each, batch 128 c007),
  // as Onshape rounds a closed edge with one.
  int made = 0;
  if (maker.IsDone())
    for (const TopoDS_Shape &e : edges.shapes)
      for (const TopoDS_Shape &g : maker.Generated(e))
        if (g.ShapeType() == TopAbs_FACE) ++made;
  // More faces than the solid had and one per edge: OCCT has patched corners where roundings meet; built from a
  // tool per edge, they meet along where they cross instead.
  if (maker.IsDone()) {
    int before = 0, after = 0;
    for (TopExp_Explorer f(solid.shape, TopAbs_FACE); f.More(); f.Next()) ++before;
    for (TopExp_Explorer f(maker.Shape(), TopAbs_FACE); f.More(); f.Next()) ++after;
    if (after > before + (int)edges.shapes.size()) {
      TopTools_ListOfShape copied;
      for (const TopoDS_Shape &e : edges.shapes) {
        const TopTools_ListOfShape &images = copier.Modified(e);
        copied.Append(images.IsEmpty() ? e : images.First());
      }
      if (std::unique_ptr<Operation> op = fillet_by_tools(copier.Shape(), copied, radius)) {
        Handle(BRepTools_History) history = new BRepTools_History(one(solid.shape), copier);
        history->Merge(op->history);
        op->history = history;
        return op;
      }
    }
  }
  if (maker.IsDone() && (largest_tolerance(maker.Shape()) > 1e-5 || made > (int)edges.shapes.size())) {
    TopTools_ListOfShape copied;
    for (const TopoDS_Shape &e : edges.shapes) {
      const TopTools_ListOfShape &images = copier.Modified(e);
      copied.Append(images.IsEmpty() ? e : images.First());
    }
    std::unique_ptr<Operation> op = fillet_by_opening(copier.Shape(), copied, radius);
    if (!op) op = fillet_by_sections(copier.Shape(), copied, radius);
    if (op) {
      Handle(BRepTools_History) history = new BRepTools_History(one(solid.shape), copier);
      history->Merge(op->history);
      op->history = history;
      return op;
    }
  }
  return finish(one(solid.shape), maker, "fillet");
}

// Rounds edges with a radius that varies from one end to the other: edge i goes from radius `r0[i]` at the end
// nearest `p0[i]` to `r1[i]` at the other, smoothly (an S curve, level at both ends) or linearly.
std::unique_ptr<Operation> variable_fillet_op(const Shape &solid, const ShapeList &edges, rust::Slice<const Vec3> p0,
                                              rust::Slice<const double> r0, rust::Slice<const double> r1, bool smooth) {
  BRepFilletAPI_MakeFillet maker(solid.shape);
  for (size_t i = 0; i < edges.shapes.size(); ++i) {
    const TopoDS_Edge &edge = TopoDS::Edge(edges.shapes[i]);
    TopoDS_Vertex first = TopExp::FirstVertex(edge, Standard_True);
    bool from_first = BRep_Tool::Pnt(first).Distance(pnt(p0[i])) < 1e-9;
    double start = from_first ? r0[i] : r1[i], end = from_first ? r1[i] : r0[i];
    // The radius along the spine (relative parameter 0 to 1), sampled from the law, which OCCT interpolates: giving
    // it the law itself (Add with a Law_Function) fails.
    const int n = smooth ? 129 : 2;
    TColgp_Array1OfPnt2d radii(1, n);
    for (int k = 0; k < n; ++k) {
      double t = double(k) / (n - 1);
      double f = smooth ? t * t * (3 - 2 * t) : t;
      radii.SetValue(k + 1, gp_Pnt2d(t, start + (end - start) * f));
    }
    maker.Add(radii, edge);
  }
  maker.Build();
  return finish(one(solid.shape), maker, "variable fillet");
}

// Rounds edges with radii sampled along each: edge i's `count` radii `radii[i * count ..]` at even relative
// parameters from its end nearest `p0[i]` to the other, which OCCT interpolates.
std::unique_ptr<Operation> sampled_fillet_op(const Shape &solid, const ShapeList &edges, rust::Slice<const Vec3> p0,
                                             rust::Slice<const double> radii, size_t count) {
  BRepFilletAPI_MakeFillet maker(solid.shape);
  for (size_t i = 0; i < edges.shapes.size(); ++i) {
    const TopoDS_Edge &edge = TopoDS::Edge(edges.shapes[i]);
    TopoDS_Vertex first = TopExp::FirstVertex(edge, Standard_True);
    bool from_first = BRep_Tool::Pnt(first).Distance(pnt(p0[i])) < 1e-9;
    TColgp_Array1OfPnt2d law(1, (int)count);
    for (size_t k = 0; k < count; ++k) {
      double t = double(k) / (count - 1);
      double r = radii[i * count + (from_first ? k : count - 1 - k)];
      law.SetValue((int)k + 1, gp_Pnt2d(t, r));
    }
    maker.Add(law, edge);
  }
  maker.Build();
  return finish(one(solid.shape), maker, "variable fillet");
}

// Chamfers edges: kind 0 equal distances `d1`; kind 1 distances `d1` on `faces[i]` and `d2` on the other face;
// kind 2 distance `d1` on `faces[i]` and angle `d2`. `faces` has a face next to each edge (unused for kind 0).
// An equal chamfer as Onshape makes one along an intersection curve (its "offset" method): its edges where a ball
// of its width touching both faces touches them, the fillet's edges, and between them the straight lines joining
// each pair of points one ball touches (batch 129: a tee's joint, a cross hole's mouth). Built from the fillet of
// that radius, its face swapped for that ruled surface. Null if it can't be made.
namespace {

// The closed chains the edges make, each in order round it.
std::vector<std::vector<TopoDS_Edge>> closed_chains(const TopTools_ListOfShape &edges) {
  std::vector<TopoDS_Edge> left;
  for (const TopoDS_Shape &e : edges) left.push_back(TopoDS::Edge(e));
  std::vector<std::vector<TopoDS_Edge>> chains;
  while (!left.empty()) {
    std::vector<TopoDS_Edge> chain{left.front()};
    left.erase(left.begin());
    TopoDS_Vertex first = TopExp::FirstVertex(chain[0], Standard_True), end = TopExp::LastVertex(chain[0], Standard_True);
    while (!first.IsSame(end)) {
      bool found = false;
      for (size_t i = 0; i < left.size() && !found; ++i) {
        TopoDS_Vertex a = TopExp::FirstVertex(left[i], Standard_True), b = TopExp::LastVertex(left[i], Standard_True);
        if (a.IsSame(end) || b.IsSame(end)) {
          TopoDS_Edge next = a.IsSame(end) ? left[i] : TopoDS::Edge(left[i].Reversed());
          chain.push_back(next);
          end = TopExp::LastVertex(next, Standard_True);
          left.erase(left.begin() + i);
          found = true;
        }
      }
      if (!found) return {};
    }
    chains.push_back(chain);
  }
  return chains;
}

// A blend's cross section between its feet `a` on the first face and `b` on the second, round the corner `c` where
// the faces' tangent lines across meet: a straight line (a chamfer); the ball's arc (a fillet; at uneven offsets the
// conic through the feet tangent to the faces, weighted as a circle's arc, sin of half the corner's angle); or a
// B-spline whose poles are `c + s (a − c) + t (b − c)` for each (s, t) of `coefficients`, with `weights` (none: not
// rational), `knots` (flat) and `degree` (a conic or a curvature section, as along a straight edge).
struct SectionShape {
  enum Kind { Line, Round, Oblique } kind = Line;
  std::vector<std::pair<double, double>> coefficients;
  std::vector<double> weights, knots;
  int degree = 0;
  // Uneven offsets swapped at a concave edge (an asymmetric fillet's radii, batch 052).
  bool swap_concave = false;
};

// The tool that blends a closed chain of edges between faces `f1` and `f2`: at each of 96 points round it, the point
// `off1` from `f1` and `off2` from `f2` on the side a ball would roll, its feet on the two faces, and between them
// the section `shape`; each section closed off past the faces (out of the material at a convex edge, into it at a
// concave one), lofted round the chain, the loft's sides sewn into a ring. `convex` says which: cut it away, or fill
// it.
TopoDS_Solid sectioned_tool(const TopoDS_Shape &solid, const std::vector<TopoDS_Edge> &chain, const TopoDS_Face &f1,
                            const TopoDS_Face &f2, double off1, double off2, const SectionShape &shape,
                            bool &convex_out) {
  const bool round = shape.kind == SectionShape::Round && std::abs(off1 - off2) < 1e-12;
  BRepBuilderAPI_MakeWire wire;
  for (const TopoDS_Edge &e : chain) wire.Add(e);
  if (!wire.IsDone()) throw std::runtime_error("chain");
  BRepAdaptor_CompCurve curve(wire.Wire());
  Handle(Geom_Surface) s1 = BRep_Tool::Surface(f1), s2 = BRep_Tool::Surface(f2);
  auto normal = [](const Handle(Geom_Surface) &surface, const TopoDS_Face &face, const gp_Pnt &p, gp_Pnt &foot) {
    GeomAPI_ProjectPointOnSurf on(p, surface);
    if (on.NbPoints() == 0) throw std::runtime_error("projection failed");
    double u, v;
    on.LowerDistanceParameters(u, v);
    gp_Vec du, dv;
    surface->D1(u, v, foot, du, dv);
    gp_Vec n = du.Crossed(dv);
    n.Normalize();
    if (face.Orientation() == TopAbs_REVERSED) n.Reverse();
    return n;
  };
  const int count = 96;
  int convex = -1;
  const double delta = 0.5 * std::max(off1, off2);
  // Each section's five sides as points across them: the blend (an arc or a line from the foot on the first face to
  // the one on the second), then the four lines closing it off.
  const int across = 7;
  std::vector<std::array<std::vector<gp_Pnt>, 5>> sections;
  std::vector<Handle(Geom_BSplineCurve)> sections_curves;
  for (int k = 0; k < count; ++k) {
    double t = curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * k / count;
    gp_Pnt p;
    gp_Vec tangent;
    curve.D1(t, p, tangent);
    tangent.Normalize();
    gp_Pnt foot;
    gp_Vec n1 = normal(s1, f1, p, foot), n2 = normal(s2, f2, p, foot);
    if (convex < 0) {
      // Into each face square to the edge; convex where the material lies between those directions.
      auto into = [&](const gp_Vec &n, const TopoDS_Face &face) {
        gp_Vec u = tangent.Crossed(n);
        gp_Pnt q = p.Translated(u * 1e-5);
        BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(q).Vertex(), face);
        if (!d.IsDone() || d.Value() > 1e-6) u.Reverse();
        return u;
      };
      gp_Vec u1 = into(n1, f1), u2 = into(n2, f2);
      BRepClass3d_SolidClassifier classifier(solid, p.Translated((u1 + u2).Normalized() * 1e-5), 1e-9);
      convex = classifier.State() == TopAbs_IN ? 1 : 0;
    }
    if (k == 0 && shape.swap_concave && !convex) std::swap(off1, off2);
    // The ball's side: into the material at a convex edge, into the void at a concave one.
    const double side = convex ? -1 : 1;
    gp_Pnt centre = p.Translated(n1 * (side * off1) + n2 * (side * off2));
    gp_Pnt q1, q2;
    for (int it = 0; it < 60; ++it) {
      gp_Vec m1 = normal(s1, f1, centre, q1) * side;
      double d1 = gp_Vec(q1, centre).Dot(m1);
      centre.Translate(m1 * (off1 - d1));
      gp_Vec m2 = normal(s2, f2, centre, q2) * side;
      double d2 = gp_Vec(q2, centre).Dot(m2);
      centre.Translate(m2 * (off2 - d2));
      // Kept in the plane square to the edge.
      centre.Translate(tangent * -gp_Vec(p, centre).Dot(tangent));
      if (std::abs(off1 - d1) < 1e-12 && std::abs(off2 - d2) < 1e-12) break;
    }
    gp_Vec a1 = normal(s1, f1, centre, q1), a2 = normal(s2, f2, centre, q2);
    // Past the faces: out of the material to cut, into it to fill.
    const double away = convex ? 1 : -1;
    gp_Pnt q2out = q2.Translated(a2 * (away * delta)), pout = p.Translated((n1 + n2) * (away * delta)),
           q1out = q1.Translated(a1 * (away * delta));
    auto line = [&](const gp_Pnt &a, const gp_Pnt &b) {
      std::vector<gp_Pnt> out;
      for (int j = 0; j < across; ++j) out.push_back(a.Translated(gp_Vec(a, b) * (double(j) / (across - 1))));
      return out;
    };
    std::vector<gp_Pnt> blend;
    if (shape.kind != SectionShape::Line && !round) {
      // The corner: where the faces' tangent lines across, square to the centre's feet, meet.
      gp_Vec u1(centre, q1), u2(centre, q2);
      double a11 = u1.Dot(u1), a12 = u1.Dot(u2), a22 = u2.Dot(u2);
      double det = a11 * a22 - a12 * a12;
      if (std::abs(det) < 1e-30) throw std::runtime_error("corner");
      // (x − 1) a11 + y a12 = 0 and x a12 + (y − 1) a22 = 0.
      double x = (a11 * a22 - a12 * a22) / det, y = (a11 * a22 - a11 * a12) / det;
      gp_Pnt corner = centre.Translated(u1 * x + u2 * y);
      std::vector<std::pair<double, double>> coefficients = shape.coefficients;
      std::vector<double> weights = shape.weights, knots = shape.knots;
      int degree = shape.degree;
      if (shape.kind == SectionShape::Round) {
        double angle = gp_Vec(corner, q1).Angle(gp_Vec(corner, q2));
        coefficients = {{1, 0}, {0, 0}, {0, 1}};
        weights = {1, std::sin(angle / 2), 1};
        knots = {0, 0, 0, 1, 1, 1};
        degree = 2;
      }
      TColgp_Array1OfPnt poles(1, int(coefficients.size()));
      for (size_t i = 0; i < coefficients.size(); ++i)
        poles(int(i) + 1) = corner.Translated(gp_Vec(corner, q1) * coefficients[i].first +
                                              gp_Vec(corner, q2) * coefficients[i].second);
      std::vector<double> distinct;
      std::vector<int> multiplicity;
      for (double kv : knots) {
        if (!distinct.empty() && std::abs(kv - distinct.back()) < 1e-12) ++multiplicity.back();
        else distinct.push_back(kv), multiplicity.push_back(1);
      }
      TColStd_Array1OfReal knot_values(1, int(distinct.size()));
      TColStd_Array1OfInteger mults(1, int(distinct.size()));
      for (size_t i = 0; i < distinct.size(); ++i) knot_values(int(i) + 1) = distinct[i], mults(int(i) + 1) = multiplicity[i];
      Handle(Geom_BSplineCurve) section;
      if (weights.empty()) {
        section = new Geom_BSplineCurve(poles, knot_values, mults, degree);
      } else {
        TColStd_Array1OfReal w(1, int(weights.size()));
        for (size_t i = 0; i < weights.size(); ++i) w(int(i) + 1) = weights[i];
        section = new Geom_BSplineCurve(poles, w, knot_values, mults, degree);
      }
      // Not rational where the weights differ from section to section (an uneven fillet's, its corner's angle
      // changing round a tee's joint): OCCT's loft through them comes out wrong, and the booleans with it. A cubic
      // through the section at 17 points with its end derivatives instead (Onshape's sections are cubic fits too).
      // A conic's weight is the same all round, and its loft right as it is.
      if (shape.kind == SectionShape::Round) {
        const int n = 17;
        Handle(TColgp_HArray1OfPnt) points = new TColgp_HArray1OfPnt(1, n);
        Handle(TColStd_HArray1OfReal) params = new TColStd_HArray1OfReal(1, n);
        double t0 = section->FirstParameter(), t1 = section->LastParameter();
        for (int i = 0; i < n; ++i) {
          double t = t0 + (t1 - t0) * i / (n - 1);
          points->SetValue(i + 1, section->Value(t));
          params->SetValue(i + 1, t);
        }
        GeomAPI_Interpolate interpolate(points, params, Standard_False, 1e-9);
        gp_Pnt e;
        gp_Vec d0, d1;
        section->D1(t0, e, d0);
        section->D1(t1, e, d1);
        // The derivatives as they are: by default Load rescales them, and the cubic strays 6 µm from the section near
        // its feet (batch 132 c005).
        interpolate.Load(d0, d1, Standard_False);
        interpolate.Perform();
        // A rational section among the cubics would bend the loft there.
        if (!interpolate.IsDone()) throw std::runtime_error("section fit");
        section = interpolate.Curve();
      }
      sections_curves.push_back(section);
      blend = {q1, q2};
    } else if (round) {
      // The arc about the centre from one foot to the other.
      gp_Vec from(centre, q1), to(centre, q2);
      double angle = from.Angle(to);
      gp_Vec axis = from.Crossed(to);
      if (axis.Magnitude() < 1e-15) throw std::runtime_error("arc");
      gp_Ax1 ax(centre, gp_Dir(axis));
      for (int j = 0; j < across; ++j) blend.push_back(q1.Rotated(ax, angle * j / (across - 1)));
    } else {
      blend = line(q1, q2);
    }
    sections.push_back({blend, line(q2, q2out), line(q2out, pout), line(pout, q1out), line(q1out, q1)});
  }
  // The sections lofted round the chain; its caps lie on each other where the loop closes: without them, its sides
  // sewn into a ring.
  BRepOffsetAPI_ThruSections loft(Standard_True, Standard_False, 1e-7);
  loft.CheckCompatibility(Standard_False);
  for (int k = 0; k <= count; ++k) {
    const auto &section = sections[k % count];
    BRepBuilderAPI_MakeWire wire;
    const auto &blend = section[0];
    if (!sections_curves.empty())
      wire.Add(BRepBuilderAPI_MakeEdge(Handle(Geom_Curve)(sections_curves[k % count])).Edge());
    else if (round)
      wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(blend.front(), blend[across / 2], blend.back()).Value()).Edge());
    else
      wire.Add(BRepBuilderAPI_MakeEdge(blend.front(), blend.back()).Edge());
    for (int side = 1; side < 5; ++side) wire.Add(BRepBuilderAPI_MakeEdge(section[side].front(), section[side].back()).Edge());
    if (!wire.IsDone()) throw std::runtime_error("section");
    loft.AddWire(wire.Wire());
  }
  loft.Build();
  if (!loft.IsDone()) throw std::runtime_error("loft");
  TopTools_MapOfShape caps;
  caps.Add(loft.FirstShape());
  caps.Add(loft.LastShape());
  BRepBuilderAPI_Sewing ring(1e-7);
  for (TopExp_Explorer f(loft.Shape(), TopAbs_FACE); f.More(); f.Next())
    if (!caps.Contains(f.Current())) ring.Add(f.Current());
  ring.Perform();
  TopExp_Explorer shells(ring.SewedShape(), TopAbs_SHELL);
  if (!shells.More()) throw std::runtime_error("ring");
  BRepBuilderAPI_MakeSolid make(TopoDS::Shell(shells.Current()));
  if (!make.IsDone()) throw std::runtime_error("solid");
  TopoDS_Solid piece = make.Solid();
  BRepLib::OrientClosedSolid(piece);
  if (!BRepCheck_Analyzer(piece).IsValid()) throw std::runtime_error("tool invalid");
  convex_out = convex == 1;
  return piece;
}

// The solid with the tools cut away (convex) and filled in (concave), its faces on one surface merged.
std::unique_ptr<Operation> applied_tools(const TopoDS_Shape &solid, const TopTools_ListOfShape &cut_tools,
                                         const TopTools_ListOfShape &fill_tools, const char *what) {
  TopoDS_Shape shape = solid;
  std::unique_ptr<Operation> op;
  for (int pass = 0; pass < 2; ++pass) {
    const TopTools_ListOfShape &tools = pass == 0 ? cut_tools : fill_tools;
    if (tools.IsEmpty()) continue;
    std::unique_ptr<BRepAlgoAPI_BooleanOperation> boolean;
    if (pass == 0) boolean = std::make_unique<BRepAlgoAPI_Cut>(); else boolean = std::make_unique<BRepAlgoAPI_Fuse>();
    boolean->SetArguments(one(shape));
    boolean->SetTools(tools);
    boolean->Build();
    if (!boolean->IsDone()) return nullptr;
    auto step = finish(one(shape), *boolean, what);
    if (op) {
      op->history->Merge(step->history);
      op->result = step->result;
    } else {
      op = std::move(step);
    }
    shape = op->result;
  }
  if (!op || !BRepCheck_Analyzer(op->result).IsValid()) return nullptr;
  ShapeUpgrade_UnifySameDomain unify(op->result, Standard_True, Standard_True, Standard_False);
  unify.Build();
  op->history->Merge(unify.History());
  op->result = unify.Shape();
  return op;
}

// The two faces every edge of a chain lies between, or false.
bool chain_faces(const TopTools_IndexedDataMapOfShapeListOfShape &edge_faces, const std::vector<TopoDS_Edge> &chain,
                 TopoDS_Face &a, TopoDS_Face &b) {
  for (size_t i = 0; i < chain.size(); ++i) {
    if (!edge_faces.Contains(chain[i])) return false;
    const TopTools_ListOfShape &faces = edge_faces.FindFromKey(chain[i]);
    if (faces.Extent() != 2) return false;
    if (i == 0) {
      a = TopoDS::Face(faces.First());
      b = TopoDS::Face(faces.Last());
    } else {
      bool same = (faces.First().IsSame(a) && faces.Last().IsSame(b)) || (faces.First().IsSame(b) && faces.Last().IsSame(a));
      if (!same) return false;
    }
  }
  return true;
}

// A fillet of closed chains of edges built from sections (the ball's arc at each of 96 points round): where OCCT's
// fillet goes wrong and the solid's opening or closing can't be made (crossing pipes' joints, batch 128 c007).
std::unique_ptr<Operation> fillet_by_sections(const TopoDS_Shape &solid, const TopTools_ListOfShape &edges, double r) {
  try {
    TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
    TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    auto chains = closed_chains(edges);
    if (chains.empty()) return nullptr;
    TopTools_ListOfShape cut_tools, fill_tools;
    for (const auto &chain : chains) {
      TopoDS_Face f1, f2;
      if (!chain_faces(edge_faces, chain, f1, f2)) return nullptr;
      bool convex = false;
      SectionShape arc;
      arc.kind = SectionShape::Round;
      TopoDS_Solid tool = sectioned_tool(solid, chain, f1, f2, r, r, arc, convex);
      (convex ? cut_tools : fill_tools).Append(tool);
    }
    return applied_tools(solid, cut_tools, fill_tools, "fillet");
  } catch (...) {
    return nullptr;
  }
}

}  // namespace

// An uneven chamfer as Onshape makes one along an intersection curve: at each point of the edge, the point `off1`
// from the first face and `off2` from the second (on the side a ball would roll), its feet on the two faces are the
// chamfer's edges, and between them it is ruled (batch 130: a tee's joint with offsets 1 and 2 has the point 2 off
// the first face and 1 off the second, the feet 1 and 2 from the edge at a square corner). Built as a tool lofted
// through the chamfer's sections, closed off past the faces, and cut away (filled at a concave edge). Null if it
// can't be made.
std::unique_ptr<Operation> chamfer_by_offsets_op(const Shape &solid, const ShapeList &edges, const ShapeList &firsts,
                                                 double off1, double off2) {
  try {
    if (edges.shapes.size() != firsts.shapes.size()) return nullptr;
    TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
    TopExp::MapShapesAndUniqueAncestors(solid.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    auto chains = closed_chains(to_list(edges));
    if (chains.empty()) return nullptr;
    TopTools_ListOfShape cut_tools, fill_tools;
    for (const auto &chain : chains) {
      TopoDS_Face a, b;
      if (!chain_faces(edge_faces, chain, a, b)) return nullptr;
      // The first face given for any edge of the chain.
      TopoDS_Face f1;
      for (size_t i = 0; i < edges.shapes.size() && f1.IsNull(); ++i)
        for (const TopoDS_Edge &e : chain)
          if (e.IsSame(edges.shapes[i])) f1 = TopoDS::Face(firsts.shapes[i]);
      if (f1.IsNull()) return nullptr;
      TopoDS_Face f2 = f1.IsSame(a) ? b : a;
      bool convex = false;
      TopoDS_Solid tool = sectioned_tool(solid.shape, chain, f1, f2, off1, off2, SectionShape(), convex);
      (convex ? cut_tools : fill_tools).Append(tool);
    }
    return applied_tools(solid.shape, cut_tools, fill_tools, "chamfer");
  } catch (...) {
    return nullptr;
  }
}

// A fillet of closed chains of edges with a section other than the ball's arc, built from sections round each chain
// (a tee's joint filleted with a conic, a curvature section or uneven radii, batch 132): at each point the point
// `off1` from the first face (`firsts`, one an edge) and `off2` from the other, its feet on them the fillet's edges,
// the section between them round the corner where the faces' tangent lines meet. `kind` 1 the ball's arc (uneven:
// the conic weighted as a circle's), 2 the B-spline of `coefficients` (pairs, flat), `weights`, `knots` and `degree`
// in the corner's oblique frame. `swap_concave` swaps the offsets at a concave edge. Null where it can't be made.
std::unique_ptr<Operation> section_blend_op(const Shape &solid, const ShapeList &edges, const ShapeList &firsts,
                                            double off1, double off2, int32_t kind,
                                            rust::Slice<const double> coefficients, rust::Slice<const double> weights,
                                            rust::Slice<const double> knots, int32_t degree, bool swap_concave) {
  try {
    if (edges.shapes.size() != firsts.shapes.size()) return nullptr;
    SectionShape shape;
    shape.kind = kind == 2 ? SectionShape::Oblique : SectionShape::Round;
    for (size_t i = 0; i + 1 < coefficients.size(); i += 2) shape.coefficients.push_back({coefficients[i], coefficients[i + 1]});
    shape.weights.assign(weights.begin(), weights.end());
    shape.knots.assign(knots.begin(), knots.end());
    shape.degree = degree;
    shape.swap_concave = swap_concave;
    TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
    TopExp::MapShapesAndUniqueAncestors(solid.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    auto chains = closed_chains(to_list(edges));
    if (chains.empty()) return nullptr;
    TopTools_ListOfShape cut_tools, fill_tools;
    for (const auto &chain : chains) {
      TopoDS_Face a, b;
      if (!chain_faces(edge_faces, chain, a, b)) return nullptr;
      TopoDS_Face f1;
      for (size_t i = 0; i < edges.shapes.size() && f1.IsNull(); ++i)
        for (const TopoDS_Edge &e : chain)
          if (e.IsSame(edges.shapes[i])) f1 = TopoDS::Face(firsts.shapes[i]);
      if (f1.IsNull()) return nullptr;
      TopoDS_Face f2 = f1.IsSame(a) ? b : a;
      bool convex = false;
      TopoDS_Solid tool = sectioned_tool(solid.shape, chain, f1, f2, off1, off2, shape, convex);
      (convex ? cut_tools : fill_tools).Append(tool);
    }
    auto op = applied_tools(solid.shape, cut_tools, fill_tools, "fillet");
    // The blend's edges come out in pieces along the faces: one edge each, as Onshape's.
    if (op) op->result = edges_fused(op->result, op->history);
    return op;
  } catch (...) {
    return nullptr;
  }
}

static std::unique_ptr<Operation> chamfer_by_ball_impl(const Shape &solid, const ShapeList &edges, double width);
std::unique_ptr<Operation> chamfer_by_ball_op(const Shape &solid, const ShapeList &edges, double width) {
  try {
    return chamfer_by_ball_impl(solid, edges, width);
  } catch (const Standard_Failure &) {
    return nullptr;
  } catch (const std::exception &) {
    return nullptr;
  }
}
static std::unique_ptr<Operation> chamfer_by_ball_impl(const Shape &solid, const ShapeList &edges, double width) {
  std::unique_ptr<Operation> fillet;
  try {
    fillet = fillet_op(solid, edges, width);
  } catch (...) {
    return nullptr;
  }
  if (!fillet) return nullptr;
  const TopoDS_Shape filleted = fillet->result;
  // The fillet's faces: those the input had nothing of.
  TopTools_IndexedMapOfShape kept;
  for (TopExp_Explorer f(solid.shape, TopAbs_FACE); f.More(); f.Next()) {
    kept.Add(f.Current());
    for (const TopoDS_Shape &m : fillet->history->Modified(f.Current())) kept.Add(m);
  }
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(filleted, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  BRepBuilderAPI_Sewing sewing(1e-6);
  int swapped = 0;
  for (TopExp_Explorer f(filleted, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    if (kept.Contains(face)) {
      sewing.Add(face);
      continue;
    }
    // Its two edges along the faces it joins (not its seam), and those faces.
    std::vector<TopoDS_Edge> sides;
    std::vector<TopoDS_Face> neighbours;
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Edge &edge = TopoDS::Edge(e.Current());
      if (BRep_Tool::IsClosed(edge, face) || BRep_Tool::Degenerated(edge)) continue;
      bool seen = false;
      for (const TopoDS_Edge &s : sides) seen = seen || s.IsSame(edge);
      if (seen) continue;
      for (const TopoDS_Shape &g : edge_faces.FindFromKey(edge))
        if (!g.IsSame(face)) {
          sides.push_back(edge);
          neighbours.push_back(TopoDS::Face(g));
        }
    }
    if (sides.size() != 2) return nullptr;
    // From the longer side: the order the fillet lists its edges in varies from run to run, and the rulings taken
    // from the other side are a little off Onshape's (batch 129 c004: 0.54 mm² of a cross hole's mouth).
    {
      GProp_GProps p0, p1;
      BRepGProp::LinearProperties(sides[0], p0);
      BRepGProp::LinearProperties(sides[1], p1);
      if (p1.Mass() > p0.Mass() + 1e-9) {
        std::swap(sides[0], sides[1]);
        std::swap(neighbours[0], neighbours[1]);
      }
    }
    // Along the first side: each point, the ball's centre `width` off its face towards the fillet, and the point
    // that ball touches on the other face.
    BRepAdaptor_Curve first(sides[0]);
    Handle(Geom_Surface) other = BRep_Tool::Surface(neighbours[1]);
    BRepAdaptor_Surface near(neighbours[0]);
    const int n = 96;
    TColgp_Array1OfPnt a(1, n), b(1, n);
    // Which way off the first face the centre lies: towards the fillet face's middle.
    BRepAdaptor_Surface middle(face);
    gp_Pnt inside = middle.Value(0.5 * (middle.FirstUParameter() + middle.LastUParameter()),
                                 0.5 * (middle.FirstVParameter() + middle.LastVParameter()));
    double sign = 0;  // which way along the first face's own normal the centres lie: set at the first point
    for (int k = 0; k < n; ++k) {
      double t = first.FirstParameter() + (first.LastParameter() - first.FirstParameter()) * k / n;
      gp_Pnt p = first.Value(t);
      GeomAPI_ProjectPointOnSurf on(p, BRep_Tool::Surface(neighbours[0]));
      if (on.NbPoints() == 0) return nullptr;
      double u, v;
      on.LowerDistanceParameters(u, v);
      gp_Pnt q;
      gp_Vec du, dv;
      near.D1(u, v, q, du, dv);
      gp_Vec normal = du.Crossed(dv);
      if (normal.Magnitude() < 1e-12) return nullptr;
      normal.Normalize();
      if (sign == 0) sign = normal.Dot(gp_Vec(p, inside)) >= 0 ? 1 : -1;
      gp_Pnt centre = p.Translated(normal * (sign * width));
      GeomAPI_ProjectPointOnSurf touch(centre, other);
      if (touch.NbPoints() == 0) return nullptr;
      a(k + 1) = p;
      b(k + 1) = touch.NearestPoint();
    }
    // Both curves through those points on the same parameters, periodic round a closed edge.
    bool closed = BRep_Tool::IsClosed(sides[0]) || first.Value(first.FirstParameter()).Distance(first.Value(first.LastParameter())) < 1e-7;
    if (!closed) return nullptr;
    Handle(TColgp_HArray1OfPnt) ha = new TColgp_HArray1OfPnt(1, n), hb = new TColgp_HArray1OfPnt(1, n);
    // (Periodic: one parameter more than points, the last closing the curve.)
    Handle(TColStd_HArray1OfReal) params = new TColStd_HArray1OfReal(1, n + 1);
    for (int k = 1; k <= n; ++k) {
      ha->SetValue(k, a(k));
      hb->SetValue(k, b(k));
      params->SetValue(k, double(k - 1) / n);
    }
    params->SetValue(n + 1, 1.0);
    GeomAPI_Interpolate ia(ha, params, Standard_True, 1e-9), ib(hb, params, Standard_True, 1e-9);
    ia.Perform();
    ib.Perform();
    if (!ia.IsDone() || !ib.IsDone()) return nullptr;
    Handle(Geom_Surface) ruled = GeomFill::Surface(ia.Curve(), ib.Curve());
    if (ruled.IsNull()) return nullptr;
    double u0, u1, v0, v1;
    ruled->Bounds(u0, u1, v0, v1);
    BRepBuilderAPI_MakeFace maker(ruled, u0, u1, v0, v1, 1e-7);
    if (!maker.IsDone()) return nullptr;
    sewing.Add(maker.Face());
    ++swapped;
  }
  if (swapped == 0) return nullptr;
  sewing.Perform();
  TopExp_Explorer shells(sewing.SewedShape(), TopAbs_SHELL);
  if (!shells.More()) return nullptr;
  BRepBuilderAPI_MakeSolid make(TopoDS::Shell(shells.Current()));
  if (!make.IsDone()) return nullptr;
  TopoDS_Solid result = make.Solid();
  BRepLib::OrientClosedSolid(result);
  if (!BRepCheck_Analyzer(result).IsValid()) return nullptr;
  // The history: through the fillet, then the sewing (the swapped faces new).
  auto op = std::make_unique<Operation>();
  op->history = fillet->history;
  Handle(BRepTools_History) sewn = new BRepTools_History;
  for (TopExp_Explorer f(filleted, TopAbs_FACE); f.More(); f.Next())
    if (sewing.IsModified(f.Current())) sewn->AddModified(f.Current(), sewing.Modified(f.Current()));
  for (TopExp_Explorer e(filleted, TopAbs_EDGE); e.More(); e.Next())
    if (sewing.IsModified(e.Current())) sewn->AddModified(e.Current(), sewing.Modified(e.Current()));
  op->history->Merge(sewn);
  op->result = result;
  return op;
}

std::unique_ptr<Operation> chamfer_op(const Shape &solid, const ShapeList &edges, const ShapeList &faces,
                                      int32_t kind, double d1, double d2) {
  BRepFilletAPI_MakeChamfer maker(solid.shape);
  for (size_t i = 0; i < edges.shapes.size(); ++i) {
    const TopoDS_Edge &edge = TopoDS::Edge(edges.shapes[i]);
    switch (kind) {
      case 0: maker.Add(d1, edge); break;
      case 1: maker.Add(d1, d2, edge, TopoDS::Face(faces.shapes.at(i))); break;
      case 2: maker.AddDA(d1, d2, edge, TopoDS::Face(faces.shapes.at(i))); break;
      default: throw std::invalid_argument("bad chamfer kind");
    }
  }
  maker.Build();
  return finish(one(solid.shape), maker, "chamfer");
}

namespace {

// The cavity a shell of an all-planar solid leaves inside it, `thickness` (negative) in from each face and out past
// each removed face: every vertex moved to where its faces' planes, so offset, meet, the faces rebuilt through the
// moved vertices. Null if a face isn't planar, an edge isn't straight, or a vertex's planes don't meet in a point.
TopoDS_Shape planar_cavity(const TopoDS_Shape &solid, const TopTools_ListOfShape &removed, double thickness) {
  TopTools_MapOfShape removing;
  for (const TopoDS_Shape &f : removed) removing.Add(f);
  TopTools_IndexedDataMapOfShapeListOfShape vertex_faces;
  TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_VERTEX, TopAbs_FACE, vertex_faces);
  // Each face's outward plane, as (normal, offset distance along it).
  auto plane_of = [&](const TopoDS_Face &face, gp_Dir &normal, double &d) -> bool {
    Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
    Handle(Geom_Plane) plane = Handle(Geom_Plane)::DownCast(surface);
    if (plane.IsNull()) return false;
    gp_Pln pln = plane->Pln();
    // The surface's normal is x × y, against the axis where the plane's frame is left-handed (a chamfer's).
    normal = pln.Axis().Direction();
    if (!pln.Position().Direct()) normal.Reverse();
    if (face.Orientation() == TopAbs_REVERSED) normal.Reverse();
    double shift = removing.Contains(face) ? -thickness : thickness;
    d = gp_Vec(normal).Dot(gp_Vec(pln.Location().XYZ())) + shift;
    return true;
  };
  for (TopExp_Explorer e(solid, TopAbs_EDGE); e.More(); e.Next()) {
    BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
    if (curve.GetType() != GeomAbs_Line) return TopoDS_Shape();
  }
  std::vector<gp_Pnt> moved(vertex_faces.Extent() + 1);
  for (int i = 1; i <= vertex_faces.Extent(); ++i) {
    std::vector<std::pair<gp_Dir, double>> planes;
    for (const TopoDS_Shape &f : vertex_faces(i)) {
      gp_Dir n;
      double d;
      if (!plane_of(TopoDS::Face(f), n, d)) return TopoDS_Shape();
      planes.push_back({n, d});
    }
    // Three planes that cross well fix the point; the rest must pass through it.
    bool found = false;
    gp_Pnt point;
    for (size_t a = 0; a < planes.size() && !found; ++a)
      for (size_t b = a + 1; b < planes.size() && !found; ++b)
        for (size_t c = b + 1; c < planes.size() && !found; ++c) {
          gp_Vec na(planes[a].first), nb(planes[b].first), nc(planes[c].first);
          double det = na.Dot(nb.Crossed(nc));
          if (std::abs(det) < 1e-6) continue;
          gp_Vec x = (nb.Crossed(nc) * planes[a].second + nc.Crossed(na) * planes[b].second +
                      na.Crossed(nb) * planes[c].second) / det;
          point = gp_Pnt(x.XYZ());
          found = true;
        }
    if (!found) return TopoDS_Shape();
    for (auto &[n, d] : planes)
      if (std::abs(gp_Vec(n).Dot(gp_Vec(point.XYZ())) - d) > 1e-9) return TopoDS_Shape();
    moved[i] = point;
  }
  BRepBuilderAPI_Sewing sewing(1e-7);
  for (TopExp_Explorer f(solid, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    TopoDS_Wire outer = BRepTools::OuterWire(face);
    std::vector<TopoDS_Wire> wires;
    for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next()) {
      BRepBuilderAPI_MakePolygon polygon;
      for (BRepTools_WireExplorer it(TopoDS::Wire(w.Current()), face); it.More(); it.Next())
        polygon.Add(moved[vertex_faces.FindIndex(it.CurrentVertex())]);
      polygon.Close();
      if (!polygon.IsDone()) return TopoDS_Shape();
      if (w.Current().IsSame(outer)) wires.insert(wires.begin(), polygon.Wire());
      else wires.push_back(polygon.Wire());
    }
    gp_Dir n;
    double d;
    plane_of(face, n, d);
    gp_Pln plane(gp_Pnt((gp_Vec(n) * d).XYZ()), n);
    BRepBuilderAPI_MakeFace maker(plane, wires[0], Standard_True);
    for (size_t k = 1; k < wires.size(); ++k) maker.Add(TopoDS::Wire(wires[k].Reversed()));
    if (!maker.IsDone()) return TopoDS_Shape();
    sewing.Add(maker.Face());
  }
  sewing.Perform();
  TopoDS_Shape sewn = sewing.SewedShape();
  TopExp_Explorer shells(sewn, TopAbs_SHELL);
  if (!shells.More()) return TopoDS_Shape();
  BRepBuilderAPI_MakeSolid make(TopoDS::Shell(shells.Current()));
  if (!make.IsDone()) return TopoDS_Shape();
  TopoDS_Solid cavity = make.Solid();
  BRepLib::OrientClosedSolid(cavity);
  if (!BRepCheck_Analyzer(cavity).IsValid()) return TopoDS_Shape();
  return cavity;
}

}  // namespace

namespace {

// Whether a shell came out whole: a point on each face kept, moved the thickness in from it, lies on the result's
// boundary (a shell that lost a face's offset leaves material there instead: a tee's branch bore, batch 128 c003).
// The surface of `face` bounded to cover `box`: planes, cylinders and cones cut to its extent (a cone not past its
// apex), closed surfaces whole. Null for other surfaces.
TopoDS_Face covering_face(const Handle(Geom_Surface) &surface, const TopoDS_Face &face, const Bnd_Box &box) {
  double x0, y0, z0, x1, y1, z1;
  box.Get(x0, y0, z0, x1, y1, z1);
  std::vector<gp_Pnt> corners;
  for (double x : {x0, x1})
    for (double y : {y0, y1})
      for (double z : {z0, z1}) corners.push_back(gp_Pnt(x, y, z));
  GeomAdaptor_Surface adaptor(surface);
  double u0 = 1e100, u1 = -1e100, v0 = 1e100, v1 = -1e100;
  auto widen = [&](double u, double v) {
    u0 = std::min(u0, u), u1 = std::max(u1, u), v0 = std::min(v0, v), v1 = std::max(v1, v);
  };
  switch (adaptor.GetType()) {
    case GeomAbs_Plane:
      for (const gp_Pnt &c : corners) {
        double u, v;
        ElSLib::Parameters(adaptor.Plane(), c, u, v);
        widen(u, v);
      }
      break;
    case GeomAbs_Cylinder:
      for (const gp_Pnt &c : corners) {
        double u, v;
        ElSLib::Parameters(adaptor.Cylinder(), c, u, v);
        widen(u, v);
      }
      u0 = 0, u1 = 2 * M_PI;
      break;
    case GeomAbs_Cone: {
      const gp_Cone cone = adaptor.Cone();
      const double along = 1 / std::cos(cone.SemiAngle());
      for (const gp_Pnt &c : corners) {
        double h = gp_Vec(cone.Location(), c).Dot(gp_Vec(cone.Axis().Direction()));
        widen(0, h * along);
      }
      u0 = 0, u1 = 2 * M_PI;
      // Not past the apex: kept to the side the face is on.
      const double apex = -cone.RefRadius() / std::sin(cone.SemiAngle());
      BRepAdaptor_Surface own(face);
      double side = 0.5 * (own.FirstVParameter() + own.LastVParameter());
      if (side > apex) v0 = std::max(v0, apex);
      else v1 = std::min(v1, apex);
      if (v1 <= v0) return TopoDS_Face();
      break;
    }
    case GeomAbs_Sphere:
    case GeomAbs_Torus: {
      BRepBuilderAPI_MakeFace whole(surface, Precision::Confusion());
      return whole.IsDone() ? whole.Face() : TopoDS_Face();
    }
    default:
      return TopoDS_Face();
  }
  BRepBuilderAPI_MakeFace bounded(surface, u0, u1, v0, v1, Precision::Confusion());
  return bounded.IsDone() ? bounded.Face() : TopoDS_Face();
}

// A point strictly inside `cell`: a little in from a point inside one of its faces.
bool point_in_cell(const TopoDS_Shape &cell, double step, gp_Pnt &out) {
  Handle(IntTools_Context) context = new IntTools_Context;
  BRepClass3d_SolidClassifier classifier(cell);
  for (TopExp_Explorer f(cell, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    gp_Pnt p;
    gp_Pnt2d uv;
    if (BOPTools_AlgoTools3D::PointInFace(face, p, uv, context) != 0) continue;
    BRepGProp_Face props(face);
    gp_Pnt q;
    gp_Vec n;
    props.Normal(uv.X(), uv.Y(), q, n);
    if (n.Magnitude() < 1e-12) continue;
    n.Normalize();
    for (double sign : {-1.0, 1.0}) {
      gp_Pnt inside = p.Translated(n * (sign * step));
      classifier.Perform(inside, 1e-9);
      if (classifier.State() == TopAbs_IN) {
        out = inside;
        return true;
      }
    }
  }
  return false;
}

// The cavity of a solid shelled inwards, built from the surfaces: each kept face's offset a thickness in and each
// removed face's own, bounded to cover the solid, split against each other into cells, and the cells inside the
// solid farther than the thickness from every kept face taken. For where OCCT's offset loses faces (a tee of pipes
// loses its branch, batch 128 c003). Only planes, cylinders, cones, spheres and tori; null otherwise.
TopTools_ListOfShape arranged_cavity(const TopoDS_Shape &solid, const TopTools_ListOfShape &removed, double thickness) {
  TopTools_ListOfShape cells;
  TopTools_MapOfShape gone;
  for (const TopoDS_Shape &f : removed) gone.Add(f);
  Bnd_Box box;
  BRepBndLib::Add(solid, box);
  box.Enlarge(std::sqrt(box.SquareExtent()) * 0.1 + 4 * std::abs(thickness));
  BOPAlgo_MakerVolume maker;
  TopTools_ListOfShape arguments;
  TopoDS_Compound kept;
  BRep_Builder builder;
  builder.MakeCompound(kept);
  std::vector<std::tuple<Handle(Geom_Surface), TopLoc_Location, bool>> seen;
  for (TopExp_Explorer f(solid, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    const bool keep = !gone.Contains(face);
    if (keep) builder.Add(kept, face);
    TopLoc_Location location;
    Handle(Geom_Surface) raw = BRep_Tool::Surface(face, location);
    // Faces on one surface (a face split in two) give it once.
    bool repeated = false;
    for (auto &[s, l, k] : seen) repeated = repeated || (s == raw && l.IsEqual(location) && k == keep);
    if (repeated) continue;
    seen.push_back({raw, location, keep});
    Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
    if (keep) {
      double d = face.Orientation() == TopAbs_REVERSED ? -thickness : thickness;
      Handle(Geom_OffsetSurface) offset = new Geom_OffsetSurface(surface, d);
      Handle(Geom_Surface) plain = offset->Surface();
      if (plain.IsNull()) return TopTools_ListOfShape();
      surface = plain;
    }
    TopoDS_Face covering = covering_face(surface, face, box);
    if (covering.IsNull()) return TopTools_ListOfShape();
    arguments.Append(covering);
  }
  maker.SetArguments(arguments);
  maker.SetIntersect(Standard_True);
  maker.Perform();
  if (maker.HasErrors()) return TopTools_ListOfShape();
  const double depth = std::abs(thickness);
  BRepClass3d_SolidClassifier inside(solid);
  for (TopExp_Explorer c(maker.Shape(), TopAbs_SOLID); c.More(); c.Next()) {
    gp_Pnt p;
    if (!point_in_cell(c.Current(), depth * 0.01, p)) return TopTools_ListOfShape();
    inside.Perform(p, 1e-9);
    if (inside.State() != TopAbs_IN) continue;
    BRepExtrema_DistShapeShape distance(BRepBuilderAPI_MakeVertex(p).Vertex(), kept);
    if (!distance.IsDone()) return TopTools_ListOfShape();
    if (distance.Value() >= depth) cells.Append(c.Current());
  }
  return cells;
}

bool shell_whole(const TopoDS_Shape &solid, const TopTools_ListOfShape &removed, const TopoDS_Shape &result, double thickness) {
  TopTools_MapOfShape gone;
  for (const TopoDS_Shape &f : removed) gone.Add(f);
  for (TopExp_Explorer f(solid, TopAbs_FACE); f.More(); f.Next()) {
    if (gone.Contains(f.Current())) continue;
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    BRepAdaptor_Surface surface(face);
    double u0 = surface.FirstUParameter(), u1 = surface.LastUParameter(), v0 = surface.FirstVParameter(), v1 = surface.LastVParameter();
    // A point inside the face: tried at a few places in its parameter box.
    bool checked = false;
    for (double fu : {0.5, 0.3, 0.7}) {
      for (double fv : {0.5, 0.3, 0.7}) {
        double u = u0 + fu * (u1 - u0), v = v0 + fv * (v1 - v0);
        gp_Pnt p;
        gp_Vec du, dv;
        surface.D1(u, v, p, du, dv);
        BRepExtrema_DistShapeShape on(BRepBuilderAPI_MakeVertex(p).Vertex(), face);
        if (!on.IsDone() || on.Value() > 1e-7) continue;
        gp_Vec n = du.Crossed(dv);
        if (n.Magnitude() < 1e-12) continue;
        n.Normalize();
        if (face.Orientation() == TopAbs_REVERSED) n.Reverse();
        // Inwards for an inward shell (`thickness` negative), outwards otherwise.
        gp_Pnt moved = p.Translated(n * thickness);
        BRepExtrema_DistShapeShape distance(BRepBuilderAPI_MakeVertex(moved).Vertex(), result);
        if (!distance.IsDone()) return false;
        // On the boundary, not inside the material.
        BRepClass3d_SolidClassifier classifier(result, moved, 1e-6);
        if (classifier.State() == TopAbs_IN) return false;
        checked = true;
        break;
      }
      if (checked) break;
    }
  }
  return true;
}

}  // namespace

// Shells a solid, removing `faces`: `thickness` positive outward, negative inward. Corners are sharp, as in Onshape.
// Where OCCT's thick solid comes out invalid (vertices where four planes meet, as two chamfers do at a block's
// corner, batch 122 c008), an all-planar solid shelled inwards is cut by its cavity built plane by plane.
std::unique_ptr<Operation> shell_op(const Shape &solid, const ShapeList &faces, double thickness) {
  // On a copy: a thick solid that fails can leave the solid it was given damaged (its edges' tolerances and curves
  // changed in place), and the fallbacks below need it whole (batch 125 c007).
  BRepBuilderAPI_Copy copier(solid.shape, Standard_True);
  const TopoDS_Shape copy = copier.Shape();
  // The thick solid is made from the copy's periodic B-splines (a loft's sides round circles) unwrapped and clamped
  // to their ranges, as the measures take them (batch 145 c005).
  for (TopExp_Explorer f(copy, TopAbs_FACE); f.More(); f.Next()) {
    Handle(Geom_Surface) g = BRep_Tool::Surface(TopoDS::Face(f.Current()));
    Handle(Geom_RectangularTrimmedSurface) trimmed = Handle(Geom_RectangularTrimmedSurface)::DownCast(g);
    if (!trimmed.IsNull()) g = trimmed->BasisSurface();
    Handle(Geom_BSplineSurface) s = Handle(Geom_BSplineSurface)::DownCast(g);
    if (s.IsNull() || !(s->IsUPeriodic() || s->IsVPeriodic())) continue;
    double u1, u2, v1, v2;
    s->Bounds(u1, u2, v1, v2);
    if (s->IsUPeriodic()) s->SetUNotPeriodic();
    if (s->IsVPeriodic()) s->SetVNotPeriodic();
    s->Segment(u1, u2, v1, v2);
  }
  TopTools_ListOfShape copied_faces;
  for (const TopoDS_Shape &f : faces.shapes) {
    const TopTools_ListOfShape &images = copier.Modified(f);
    copied_faces.Append(images.IsEmpty() ? f : images.First());
  }
  BRepOffsetAPI_MakeThickSolid maker;
  maker.MakeThickSolidByJoin(copy, copied_faces, thickness, 1e-7, BRepOffset_Skin, Standard_False, Standard_False,
                             GeomAbs_Intersection);
  maker.Build();
  // Every face kept must have its offset in the result: OCCT's can come out valid with one missing (a tee of
  // pipes, its branch's bore, batch 128 c003).
  bool complete = maker.IsDone();
  if (complete) {
    TopTools_MapOfShape removed;
    for (const TopoDS_Shape &f : copied_faces) removed.Add(f);
    TopTools_IndexedMapOfShape result_faces;
    TopExp::MapShapes(maker.Shape(), TopAbs_FACE, result_faces);
    for (TopExp_Explorer f(copy, TopAbs_FACE); f.More() && complete; f.Next()) {
      if (removed.Contains(f.Current())) continue;
      bool imaged = false;
      for (const TopoDS_Shape &g : maker.Generated(f.Current())) imaged = imaged || result_faces.Contains(g);
      for (const TopoDS_Shape &g : maker.Modified(f.Current()))
        imaged = imaged || (result_faces.Contains(g) && !g.IsSame(f.Current()));
      if (!imaged) complete = false;
    }
  }
  if (complete && BRepCheck_Analyzer(maker.Shape()).IsValid()) {
    auto op = std::make_unique<Operation>();
    op->history = new BRepTools_History(one(solid.shape), copier);
    op->history->Merge(new BRepTools_History(one(copy), maker));
    op->result = maker.Shape();
    return op;
  }
  if (thickness < 0 && !faces.shapes.empty()) {
    TopoDS_Shape cavity = planar_cavity(solid.shape, to_list(faces), thickness);
    if (!cavity.IsNull()) {
      BRepAlgoAPI_Cut cut(solid.shape, cavity);
      cut.Build();
      if (cut.IsDone() && BRepCheck_Analyzer(cut.Shape()).IsValid()) return finish(one(solid.shape), cut, "shell");
    }
    // Otherwise: the solid offset inwards with its removed faces offset outwards instead (every other face's
    // offset carried on past where they were), on a copy, and that cavity cut away (a hemisphere open at its flat
    // face, batch 125 c007; a tee of pipes open at its ends, batch 124 c009).
    {
      BRepBuilderAPI_Copy again(solid.shape, Standard_True);
      BRepOffset_MakeOffset offset;
      offset.Initialize(again.Shape(), thickness, 1e-7, BRepOffset_Skin, Standard_False, Standard_False,
                        GeomAbs_Intersection, Standard_False);
      for (const TopoDS_Shape &f : faces.shapes) {
        const TopTools_ListOfShape &images = again.Modified(f);
        offset.SetOffsetOnFace(TopoDS::Face(images.IsEmpty() ? f : images.First()), -thickness);
      }
      try {
        offset.MakeOffsetShape();
        if (offset.IsDone() && BRepCheck_Analyzer(offset.Shape()).IsValid()) {
          BRepAlgoAPI_Cut cut(solid.shape, offset.Shape());
          cut.Build();
          if (cut.IsDone() && BRepCheck_Analyzer(cut.Shape()).IsValid() &&
              shell_whole(solid.shape, to_list(faces), cut.Shape(), thickness))
            return finish(one(solid.shape), cut, "shell");
        }
      } catch (const Standard_Failure &) {
      }
    }
    // Otherwise, where each removed face is planar and its neighbours run straight along its normal from it
    // (planes square to it, cylinders along it): the whole solid offset inwards, its images of the removed faces
    // run out past the solid, and that cavity cut away (a tee of pipes, batch 128 c003).
    try {
      BRepOffsetAPI_MakeOffsetShape inner;
      inner.PerformByJoin(BRepBuilderAPI_Copy(solid.shape, Standard_True).Shape(), thickness, 1e-7, BRepOffset_Skin,
                          Standard_False, Standard_False, GeomAbs_Intersection);
      bool ok = inner.IsDone() && !inner.Shape().IsNull() && BRepCheck_Analyzer(inner.Shape()).IsValid();
      TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
      TopExp::MapShapesAndUniqueAncestors(solid.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
      std::vector<std::pair<gp_Dir, TopoDS_Face>> removed;
      for (const TopoDS_Shape &f : faces.shapes) {
        if (!ok) break;
        Handle(Geom_Plane) plane = Handle(Geom_Plane)::DownCast(BRep_Tool::Surface(TopoDS::Face(f)));
        if (plane.IsNull()) { ok = false; break; }
        gp_Dir normal = plane->Pln().Axis().Direction();
        if (!plane->Pln().Position().Direct()) normal.Reverse();
        if (f.Orientation() == TopAbs_REVERSED) normal.Reverse();
        for (TopExp_Explorer e(f, TopAbs_EDGE); e.More() && ok; e.Next()) {
          if (!edge_faces.Contains(e.Current())) continue;
          for (const TopoDS_Shape &g : edge_faces.FindFromKey(e.Current())) {
            if (g.IsSame(f)) continue;
            BRepAdaptor_Surface surface(TopoDS::Face(g));
            if (surface.GetType() == GeomAbs_Plane) {
              if (std::abs(gp_Vec(surface.Plane().Axis().Direction()).Dot(gp_Vec(normal))) > 1e-9) ok = false;
            } else if (surface.GetType() == GeomAbs_Cylinder) {
              if (!surface.Cylinder().Axis().Direction().IsParallel(normal, 1e-9)) ok = false;
            } else {
              ok = false;
            }
          }
        }
        removed.push_back({normal, TopoDS::Face(f)});
      }
      // The images of the removed faces in the offset: its planar faces parallel to them, a thickness in.
      TopoDS_Shape cavity = ok ? inner.Shape() : TopoDS_Shape();
      for (auto &[normal, face] : removed) {
        if (!ok) break;
        gp_Pln original = Handle(Geom_Plane)::DownCast(BRep_Tool::Surface(face))->Pln();
        bool found = false;
        for (TopExp_Explorer g(inner.Shape(), TopAbs_FACE); g.More(); g.Next()) {
          Handle(Geom_Plane) p = Handle(Geom_Plane)::DownCast(BRep_Tool::Surface(TopoDS::Face(g.Current())));
          if (p.IsNull() || !p->Pln().Axis().Direction().IsParallel(normal, 1e-9)) continue;
          if (std::abs(original.Distance(p->Pln().Location()) - std::abs(thickness)) > 1e-9) continue;
          // Swept back from beyond the solid to it, its walls' cylinders running as the offset's next to it.
          TopoDS_Shape start = BRepBuilderAPI_Copy(g.Current(), Standard_True).Shape();
          bool backwards = false;
          {
            TopTools_IndexedDataMapOfShapeListOfShape inner_edges;
            TopExp::MapShapesAndUniqueAncestors(inner.Shape(), TopAbs_EDGE, TopAbs_FACE, inner_edges);
            for (TopExp_Explorer e(g.Current(), TopAbs_EDGE); e.More(); e.Next()) {
              if (!inner_edges.Contains(e.Current())) continue;
              for (const TopoDS_Shape &h : inner_edges.FindFromKey(e.Current())) {
                BRepAdaptor_Surface surface(TopoDS::Face(h), Standard_False);
                if (surface.GetType() == GeomAbs_Cylinder &&
                    gp_Vec(surface.Cylinder().Axis().Direction()).Dot(gp_Vec(normal)) < 0)
                  backwards = true;
              }
            }
          }
          gp_Vec sweep = gp_Vec(normal) * (-3 * thickness);
          if (backwards) {
            gp_Trsf out;
            out.SetTranslation(sweep);
            start = BRepBuilderAPI_Transform(start, out, Standard_True).Shape();
            sweep.Reverse();
          }
          BRepPrimAPI_MakePrism prism(start, sweep);
          if (!prism.IsDone()) { ok = false; break; }
          TopExp_Explorer solids(prism.Shape(), TopAbs_SOLID);
          if (!solids.More()) { ok = false; break; }
          TopoDS_Solid run = TopoDS::Solid(solids.Current());
          BRepLib::OrientClosedSolid(run);
          BRepAlgoAPI_Fuse fuse(cavity, run);
          fuse.Build();
          if (!fuse.IsDone()) { ok = false; break; }
          cavity = fuse.Shape();
          found = true;
          break;
        }
        if (!found) ok = false;
      }
      if (ok) {
        BRepAlgoAPI_Cut cut(solid.shape, cavity);
        cut.Build();
        if (cut.IsDone() && BRepCheck_Analyzer(cut.Shape()).IsValid() &&
            shell_whole(solid.shape, to_list(faces), cut.Shape(), thickness)) {
          // The run-outs' walls carry on the offset's: one face each, as Onshape's.
          auto op = finish(one(solid.shape), cut, "shell");
          ShapeUpgrade_UnifySameDomain unify(op->result, Standard_True, Standard_True, Standard_False);
          unify.Build();
          op->history->Merge(unify.History());
          op->result = unify.Shape();
          return op;
        }
      }
    } catch (const Standard_Failure &) {
    }
    // Otherwise the cavity built from the faces' surfaces, cell by cell.
    try {
      TopTools_ListOfShape cells = arranged_cavity(solid.shape, to_list(faces), thickness);
      if (!cells.IsEmpty()) {
        BRepAlgoAPI_Cut cut;
        cut.SetArguments(one(solid.shape));
        cut.SetTools(cells);
        cut.Build();
        if (cut.IsDone() && BRepCheck_Analyzer(cut.Shape()).IsValid() &&
            shell_whole(solid.shape, to_list(faces), cut.Shape(), thickness)) {
          // The cells split the cavity's walls and the curves where they meet: one face and one edge each, as
          // Onshape's.
          auto op = finish(one(solid.shape), cut, "shell");
          ShapeUpgrade_UnifySameDomain unify(op->result, Standard_True, Standard_True, Standard_False);
          unify.Build();
          op->history->Merge(unify.History());
          op->result = edges_fused(unify.Shape(), op->history);
          return op;
        }
      }
    } catch (const Standard_Failure &) {
    }
  }
  throw std::runtime_error("shell failed");
}

// An offset of a whole shape: `distance` positive outward. Corners are sharp.
std::unique_ptr<Shape> offset_shape(const Shape &s, double distance) {
  BRepOffsetAPI_MakeOffsetShape maker;
  maker.PerformByJoin(s.shape, distance, 1e-7, BRepOffset_Skin, Standard_False, Standard_False,
                      GeomAbs_Intersection);
  if (!maker.IsDone()) throw std::runtime_error("offset failed");
  return wrap(maker.Shape());
}

// Drafts faces of a solid by `angle` about where they meet the neutral plane, relative to the pull direction.
std::unique_ptr<Operation> draft_op(const Shape &solid, const ShapeList &faces, Vec3 pull, double angle,
                                    rust::Slice<const Frame> neutrals) {
  BRepOffsetAPI_DraftAngle maker(solid.shape);
  if (neutrals.empty()) throw std::runtime_error("draft: no neutral plane");
  for (size_t i = 0; i < faces.shapes.size(); ++i) {
    const Frame &neutral = neutrals[std::min(i, neutrals.size() - 1)];
    gp_Pln plane(pnt(neutral.origin), dir(neutral.normal));
    maker.Add(TopoDS::Face(faces.shapes[i]), dir(pull), angle, plane);
    if (!maker.AddDone()) throw std::runtime_error("draft: a face can't be drafted");
  }
  maker.Build();
  auto op = finish(one(solid.shape), maker, "draft");
  // A face next to drafted ones comes out with a strip added round it on its own plane: one face, as in Onshape (a
  // body draft's top, batch 050).
  ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_True, Standard_False);
  unifier.Build();
  op->history->Merge(unifier.History());
  op->result = unifier.Shape();
  return op;
}

// As draft_op, each face at its own angle (a symmetric or two sided parting line draft, batch 049).
std::unique_ptr<Operation> draft_angles_op(const Shape &solid, const ShapeList &faces, Vec3 pull,
                                           rust::Slice<const double> angles, rust::Slice<const Frame> neutrals) {
  if (neutrals.empty() || angles.empty()) throw std::runtime_error("draft: no neutral plane or angle");
  // Faces split from one face share its surface, and drafting one would turn them all: on a copy of the solid,
  // each face to draft gets a surface of its own first (a parting line splitting a block's sides, batch 049).
  BRepBuilderAPI_Copy copier(solid.shape, Standard_True);
  const TopoDS_Shape copy = copier.Shape();
  std::vector<TopoDS_Face> targets;
  BRep_Builder builder;
  for (const TopoDS_Shape &f : faces.shapes) {
    const TopTools_ListOfShape &images = copier.Modified(f);
    TopoDS_Face face = TopoDS::Face(images.IsEmpty() ? f : images.First());
    TopLoc_Location location;
    Handle(Geom_Surface) surface = BRep_Tool::Surface(face, location);
    builder.UpdateFace(face, Handle(Geom_Surface)::DownCast(surface->Copy()), location, BRep_Tool::Tolerance(face));
    targets.push_back(face);
  }
  BRepOffsetAPI_DraftAngle maker(copy);
  for (size_t i = 0; i < targets.size(); ++i) {
    const Frame &neutral = neutrals[std::min(i, neutrals.size() - 1)];
    gp_Pln plane(pnt(neutral.origin), dir(neutral.normal));
    maker.Add(targets[i], dir(pull), angles[std::min(i, angles.size() - 1)], plane);
    if (!maker.AddDone()) throw std::runtime_error("draft: a face can't be drafted");
  }
  maker.Build();
  if (!maker.IsDone()) throw std::runtime_error("draft failed");
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History(one(solid.shape), copier);
  op->history->Merge(one(copy), maker);
  op->result = maker.Shape();
  ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_True, Standard_False);
  unifier.Build();
  op->history->Merge(unifier.History());
  op->result = unifier.Shape();
  return op;
}

namespace {
// The face with its straight B-spline and Bézier edges remade as lines between the same vertices, so a prism of it
// has planes there, not surfaces of extrusion (a sheet metal jog's middle wall, batch 131 c008). The history is
// merged into `history`.
TopoDS_Face straightened(const TopoDS_Face &face, BRepTools_History &history) {
  Handle(BRepTools_ReShape) reshape = new BRepTools_ReShape();
  bool changed = false;
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
    const TopoDS_Edge edge = TopoDS::Edge(it.Current().Oriented(TopAbs_FORWARD));
    if (BRep_Tool::Degenerated(edge) || reshape->IsRecorded(edge)) continue;
    BRepAdaptor_Curve curve(edge);
    std::vector<gp_Pnt> poles;
    if (curve.GetType() == GeomAbs_BSplineCurve) {
      Handle(Geom_BSplineCurve) b = curve.BSpline();
      for (int i = 1; i <= b->NbPoles(); ++i) poles.push_back(b->Pole(i));
    } else if (curve.GetType() == GeomAbs_BezierCurve) {
      Handle(Geom_BezierCurve) b = curve.Bezier();
      for (int i = 1; i <= b->NbPoles(); ++i) poles.push_back(b->Pole(i));
    } else {
      continue;
    }
    TopoDS_Vertex v1, v2;
    TopExp::Vertices(edge, v1, v2);
    if (v1.IsNull() || v2.IsNull() || v1.IsSame(v2)) continue;
    const gp_Pnt a = BRep_Tool::Pnt(v1), b = BRep_Tool::Pnt(v2);
    const gp_Vec chord(a, b);
    if (chord.Magnitude() < 1e-9) continue;
    const gp_Dir along(chord);
    bool straight = true;
    for (const gp_Pnt &p : poles) {
      const gp_Vec offset(a, p);
      if (offset.Crossed(gp_Vec(along)).Magnitude() > 1e-10) straight = false;
    }
    // Poles along the chord but doubling back would make a line that isn't the curve.
    for (size_t i = 1; straight && i < poles.size(); ++i)
      if (gp_Vec(poles[i - 1], poles[i]).Dot(gp_Vec(along)) < -1e-12) straight = false;
    if (!straight) continue;
    BRepBuilderAPI_MakeEdge maker(v1, v2);
    if (!maker.IsDone()) continue;
    reshape->Replace(edge, maker.Edge());
    changed = true;
  }
  if (!changed) return face;
  const TopoDS_Face result = TopoDS::Face(reshape->Apply(face));
  history.Merge(reshape->History());
  return result;
}
}  // namespace

// Thickens a face or sheet `t1` along its normal and `t2` against it. Planar faces are swept exactly; curved ones
// are offset.
std::unique_ptr<Operation> thicken_op(const Shape &input, double t1, double t2) {
  // Work on a copy: a face of a solid must not share topology with the new body. The history runs from the input,
  // through the copy, to the result. The caps are the faces at the thickness2 side (first) and thickness1 side (last).
  BRepBuilderAPI_Copy copier(input.shape);
  const TopoDS_Shape sheet = copier.Shape();
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History(one(input.shape), copier);
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(sheet, TopAbs_FACE, faces);
  bool planar = faces.Extent() > 0;
  for (int i = 1; i <= faces.Extent(); ++i)
    if (BRepAdaptor_Surface(TopoDS::Face(faces(i))).GetType() != GeomAbs_Plane) planar = false;
  if (planar && faces.Extent() == 1) {
    const TopoDS_Face face = straightened(TopoDS::Face(faces(1)), *op->history);
    gp_Vec n(face_normal(face));
    gp_Trsf back;
    back.SetTranslation(n * -t2);
    BRepBuilderAPI_Transform mover(face, back, Standard_True);
    op->history->Merge(one(face), mover);
    const TopoDS_Shape start = mover.Shape();
    BRepPrimAPI_MakePrism maker(start, n * (t1 + t2), Standard_False, Standard_True);
    maker.Build();
    if (!maker.IsDone()) throw std::runtime_error("thicken failed");
    op->history->Merge(one(start), maker);
    op->result = maker.Shape();
    op->first.push_back(maker.FirstShape(start));
    op->last.push_back(maker.LastShape(start));
    return op;
  }
  // BRepOffset_MakeOffset in thickening mode keeps analytic surfaces exact (a cylinder offsets to a cylinder);
  // the simpler offset turns them into B-splines.
  TopoDS_Shape base = sheet;
  if (t2 != 0) {
    BRepOffset_MakeOffset back;
    back.Initialize(sheet, -t2, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Intersection);
    back.MakeOffsetShape();
    if (!back.IsDone()) throw std::runtime_error("thicken: offset failed");
    op->history->Merge(one(sheet), back);
    base = back.Shape();
  }
  // Offset faces meet where they intersect, not round an arc: a thin loft between squares has sharp corners in
  // Onshape (batch 028).
  BRepOffset_MakeOffset maker;
  maker.Initialize(base, t1 + t2, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Intersection,
                   Standard_True);
  maker.MakeOffsetShape();
  if (!maker.IsDone()) throw std::runtime_error("thicken failed");
  op->history->Merge(one(base), maker);
  op->result = maker.Shape();
  TopTools_IndexedMapOfShape result_faces;
  TopExp::MapShapes(op->result, TopAbs_FACE, result_faces);
  TopTools_IndexedMapOfShape base_faces;
  TopExp::MapShapes(base, TopAbs_FACE, base_faces);
  for (int i = 1; i <= base_faces.Extent(); ++i) {
    if (result_faces.Contains(base_faces(i))) op->first.push_back(base_faces(i));
    for (const TopoDS_Shape &image : maker.Modified(base_faces(i))) op->last.push_back(image);
    for (const TopoDS_Shape &image : maker.Generated(base_faces(i)))
      if (image.ShapeType() == TopAbs_FACE) op->last.push_back(image);
  }
  // BRepOffset_MakeOffset doesn't report the walls as generated from the boundary edges they run along: record each
  // wall as generated from every input edge lying wholly on it (at its ends and middle).
  TopTools_IndexedMapOfShape caps;
  for (const TopoDS_Shape &f : op->first) caps.Add(f);
  for (const TopoDS_Shape &f : op->last) caps.Add(f);
  TopTools_IndexedMapOfShape input_edges;
  TopExp::MapShapes(input.shape, TopAbs_EDGE, input_edges);
  for (int j = 1; j <= input_edges.Extent(); ++j) {
    const TopoDS_Edge &edge = TopoDS::Edge(input_edges(j));
    if (BRep_Tool::Degenerated(edge)) continue;
    BRepAdaptor_Curve curve(edge);
    std::vector<TopoDS_Shape> samples;
    for (double f : {0.0, 0.5, 1.0})
      samples.push_back(BRepBuilderAPI_MakeVertex(
                            curve.Value(curve.FirstParameter() + f * (curve.LastParameter() - curve.FirstParameter())))
                            .Shape());
    for (int i = 1; i <= result_faces.Extent(); ++i) {
      const TopoDS_Shape &wall = result_faces(i);
      if (caps.Contains(wall)) continue;
      bool on = true;
      for (const TopoDS_Shape &sample : samples) {
        BRepExtrema_DistShapeShape distance(sample, wall);
        if (!distance.IsDone() || distance.Value() > 1e-7) {
          on = false;
          break;
        }
      }
      if (on) op->history->AddGenerated(edge, wall);
    }
  }
  // Walls on one plane, run from neighbouring input edges, are one face, as Onshape's (a top and its fillet
  // thickened together: each end one wall, batch 127 c006); the caps stay as they are.
  {
    TopTools_MapOfShape keep;
    for (const std::vector<TopoDS_Shape> *list : {&op->first, &op->last})
      for (const TopoDS_Shape &f : *list)
        for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next()) keep.Add(e.Current());
    ShapeUpgrade_UnifySameDomain unify(op->result, Standard_True, Standard_True, Standard_False);
    unify.KeepShapes(keep);
    unify.Build();
    op->history->Merge(unify.History());
    op->result = unify.Shape();
    for (std::vector<TopoDS_Shape> *list : {&op->first, &op->last})
      for (TopoDS_Shape &f : *list) {
        const TopTools_ListOfShape &images = unify.History()->Modified(f);
        if (!images.IsEmpty()) f = images.First();
      }
  }
  // Thickening a face of a solid along its outward normal can come out inside out.
  GProp_GProps props;
  BRepGProp::VolumeProperties(op->result, props);
  if (props.Mass() < 0) op->result.Reverse();
  return op;
}

namespace {

// A sheet offset by `distance` along its faces' normals, as a shell (the sheet itself, copied, for no distance).
TopoDS_Shape offset_sheet(const TopoDS_Shape &sheet, double distance) {
  if (std::abs(distance) < 1e-12) return BRepBuilderAPI_Copy(sheet).Shape();
  BRepOffset_MakeOffset maker;
  maker.Initialize(sheet, distance, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc);
  maker.MakeOffsetShape();
  if (!maker.IsDone()) throw std::runtime_error("thicken sheet: offset failed");
  return maker.Shape();
}

// The midpoint of an edge.
gp_Pnt edge_middle(const TopoDS_Edge &edge) {
  BRepAdaptor_Curve curve(edge);
  return curve.Value(0.5 * (curve.FirstParameter() + curve.LastParameter()));
}

}  // namespace

// A sheet with its faces turned to agree with each other across their shared edges (filleting an open shell can
// leave the new faces turned the other way).
std::unique_ptr<Shape> nurbs_converted(const Shape &input) {
  BRepBuilderAPI_NurbsConvert nurbs(input.shape, Standard_True);
  if (!nurbs.IsDone()) throw std::runtime_error("nurbs conversion failed");
  Handle(BRepTools_History) history = new BRepTools_History;
  return wrap(planes_recognized(nurbs.Shape(), history));
}

std::unique_ptr<Shape> orient_sheet(const Shape &input) {
  BRep_Builder builder;
  TopoDS_Compound out;
  builder.MakeCompound(out);
  int shells = 0;
  TopoDS_Shape last;
  for (TopExp_Explorer s(input.shape, TopAbs_SHELL); s.More(); s.Next()) {
    ShapeFix_Shell fixer;
    fixer.FixFaceOrientation(TopoDS::Shell(s.Current()), Standard_True, Standard_False);
    last = fixer.Shell();
    builder.Add(out, last);
    ++shells;
  }
  if (shells == 0) return wrap(input.shape);
  return wrap(shells == 1 ? last : TopoDS_Shape(out));
}

// A smooth sheet thickened `t1` along its normals and `t2` against them: the sheet offset both ways, each pair of
// matching boundary edges joined by a face across the thickness, all sewn into one solid. Sheet metal's sheets are
// smooth once their bends are rounded, and OCCT's own thickening of such a sheet can come out invalid (batch 012).
std::unique_ptr<Shape> thicken_sheet(const Shape &input, double t1, double t2) {
  const TopoDS_Shape front = offset_sheet(input.shape, t1);
  const TopoDS_Shape back = offset_sheet(input.shape, -t2);
  auto free_edges = [](const TopoDS_Shape &shape) {
    std::vector<TopoDS_Edge> out;
    TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
    TopExp::MapShapesAndAncestors(shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    for (int i = 1; i <= edge_faces.Extent(); ++i)
      if (edge_faces(i).Extent() == 1 && !BRep_Tool::Degenerated(TopoDS::Edge(edge_faces.FindKey(i))))
        out.push_back(TopoDS::Edge(edge_faces.FindKey(i)));
    return out;
  };
  const std::vector<TopoDS_Edge> front_edges = free_edges(front), back_edges = free_edges(back);

  const double reach = t1 + t2 + 1e-6;
  BRepBuilderAPI_Sewing sewing(1e-6);
  for (TopExp_Explorer f(front, TopAbs_FACE); f.More(); f.Next()) sewing.Add(f.Current());
  for (TopExp_Explorer f(back, TopAbs_FACE); f.More(); f.Next()) sewing.Add(f.Current().Reversed());
  std::vector<bool> used(back_edges.size(), false);
  for (const TopoDS_Edge &e1 : front_edges) {
    // The back edge across from this one: the nearest, with matching ends.
    const gp_Pnt m1 = edge_middle(e1);
    int best = -1;
    double best_distance = reach;
    for (size_t j = 0; j < back_edges.size(); ++j) {
      if (used[j]) continue;
      const double d = m1.Distance(edge_middle(back_edges[j]));
      if (d < best_distance) {
        best = static_cast<int>(j);
        best_distance = d;
      }
    }
    if (best < 0) throw std::runtime_error("thicken sheet: no edge across the thickness");
    used[best] = true;
    const TopoDS_Edge &e2 = back_edges[best];
    TopoDS_Vertex a1, b1, a2, b2;
    TopExp::Vertices(e1, a1, b1);
    TopExp::Vertices(e2, a2, b2);
    const gp_Pnt pa1 = BRep_Tool::Pnt(a1), pb1 = BRep_Tool::Pnt(b1);
    gp_Pnt pa2 = BRep_Tool::Pnt(a2), pb2 = BRep_Tool::Pnt(b2);
    TopoDS_Edge across = e2;
    if (pa1.Distance(pa2) + pb1.Distance(pb2) > pa1.Distance(pb2) + pb1.Distance(pa2)) {
      across = TopoDS::Edge(e2.Reversed());
      std::swap(pa2, pb2);
    }
    // A planar face where the two edges lie in one plane (always, for planar walls and round bends), else ruled.
    BRepBuilderAPI_MakeWire wire;
    wire.Add(e1);
    if (pb1.Distance(pb2) > 1e-9) wire.Add(BRepBuilderAPI_MakeEdge(pb1, pb2).Edge());
    wire.Add(TopoDS::Edge(across.Reversed()));
    if (pa2.Distance(pa1) > 1e-9) wire.Add(BRepBuilderAPI_MakeEdge(pa2, pa1).Edge());
    TopoDS_Face side;
    if (wire.IsDone()) {
      BRepBuilderAPI_MakeFace planar(wire.Wire(), Standard_True);
      if (planar.IsDone()) side = planar.Face();
    }
    if (side.IsNull()) side = BRepFill::Face(e1, across);
    sewing.Add(side);
  }
  sewing.Perform();
  TopoDS_Shape sewn = sewing.SewedShape();
  TopoDS_Shell shell;
  for (TopExp_Explorer s(sewn, TopAbs_SHELL); s.More(); s.Next()) {
    shell = TopoDS::Shell(s.Current());
    break;
  }
  if (shell.IsNull()) throw std::runtime_error("thicken sheet: sewing failed");
  ShapeFix_Solid fixer;
  TopoDS_Shape solid = fixer.SolidFromShell(shell);
  GProp_GProps props;
  BRepGProp::VolumeProperties(solid, props);
  if (props.Mass() < 0) solid.Reverse();
  return wrap(solid);
}

// Offsets some faces of a solid by `distance` along their normals, extending or trimming the faces around them.
std::unique_ptr<Operation> offset_faces_op(const Shape &solid, const ShapeList &faces, double distance) {
  BRepOffset_MakeOffset maker;
  maker.Initialize(solid.shape, 0.0, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Intersection);
  for (const TopoDS_Shape &f : faces.shapes) maker.SetOffsetOnFace(TopoDS::Face(f), distance);
  maker.MakeOffsetShape();
  if (!maker.IsDone()) throw std::runtime_error("offset faces failed");
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History();
  op->result = maker.Shape();
  // BRepOffset_MakeOffset has no history interface: record each input face's and edge's image in the result.
  TopTools_IndexedMapOfShape result_faces, result_edges;
  TopExp::MapShapes(op->result, TopAbs_FACE, result_faces);
  TopExp::MapShapes(op->result, TopAbs_EDGE, result_edges);
  auto record = [&](TopAbs_ShapeEnum kind, const BRepAlgo_Image &image_of, const TopTools_IndexedMapOfShape &result) {
    TopTools_IndexedMapOfShape inputs;
    TopExp::MapShapes(solid.shape, kind, inputs);
    for (int i = 1; i <= inputs.Extent(); ++i) {
      if (!image_of.HasImage(inputs(i))) continue;
      TopTools_ListOfShape images;
      image_of.LastImage(inputs(i), images);
      for (const TopoDS_Shape &image : images)
        if (!image.IsSame(inputs(i)) && result.Contains(image)) op->history->AddModified(inputs(i), image);
    }
  };
  record(TopAbs_FACE, maker.OffsetFacesFromShapes(), result_faces);
  record(TopAbs_EDGE, maker.OffsetEdgesFromShapes(), result_edges);
  return op;
}

// Removes faces of a solid and extends the faces around them to close the gap.
std::unique_ptr<Operation> delete_faces_op(const Shape &solid, const ShapeList &faces) {
  BRepAlgoAPI_Defeaturing maker;
  maker.SetShape(solid.shape);
  maker.AddFacesToRemove(to_list(faces));
  maker.SetRunParallel(Standard_False);
  maker.Build();
  if (maker.HasErrors() || !maker.IsDone()) throw std::runtime_error("delete faces failed");
  auto op = std::make_unique<Operation>();
  op->history = maker.History();
  op->result = maker.Shape();
  return op;
}

// Replaces a planar face of a solid with a plane: the solid is extended past the face along its normal and cut by
// the plane, and faces left on one surface are merged. The faces next to `face` must be perpendicular to it, so
// that extending the solid extends them; otherwise this throws.
std::unique_ptr<Operation> replace_planar_face_op(const Shape &solid, const Shape &face, Frame target) {
  const TopoDS_Face &f = TopoDS::Face(face.shape);
  if (BRepAdaptor_Surface(f).GetType() != GeomAbs_Plane) throw std::runtime_error("replace face: not planar");
  gp_Dir n = face_normal(f);
  // Replacing a face with (nearly) its own plane changes nothing: frame mitres a tangent joint that way, and a joint
  // drawn tangent to a few digits is still one.
  {
    GProp_GProps props;
    BRepGProp::SurfaceProperties(f, props);
    gp_Pln plane(pnt(target.origin), dir(target.normal));
    if (n.IsParallel(dir(target.normal), 1e-5) && plane.Distance(props.CentreOfMass()) < 1e-7) {
      auto op = std::make_unique<Operation>();
      op->history = new BRepTools_History();
      op->result = solid.shape;
      return op;
    }
  }
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(solid.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  // The faces round it: perpendicular to it (planes or cylinders along its normal), or planes leaning; or, for the
  // end of a body revolved about an axis lying in its plane, surfaces of revolution about that axis and planes
  // square to it (a frame's segment round a cylinder, batch 138 c005).
  std::vector<TopoDS_Face> neighbours;
  bool square = true;
  bool revolved = false;
  gp_Ax1 axis;
  const gp_Pln face_plane = BRepAdaptor_Surface(f).Plane();
  auto revolution_axis = [](const BRepAdaptor_Surface &surface, gp_Ax1 &out) {
    switch (surface.GetType()) {
      case GeomAbs_Cylinder: out = surface.Cylinder().Axis(); return true;
      case GeomAbs_Cone: out = surface.Cone().Axis(); return true;
      case GeomAbs_Torus: out = surface.Torus().Axis(); return true;
      case GeomAbs_Sphere: out = surface.Sphere().Position().Axis(); return true;
      default: return false;
    }
  };
  for (TopExp_Explorer it(f, TopAbs_EDGE); it.More(); it.Next()) {
    const TopTools_ListOfShape *faces = edge_faces.Seek(it.Current());
    if (faces == nullptr) continue;
    for (const TopoDS_Shape &g : *faces) {
      if (g.IsSame(f)) continue;
      BRepAdaptor_Surface surface(TopoDS::Face(g));
      gp_Ax1 a;
      if (surface.GetType() == GeomAbs_Plane) {
        if (std::abs(surface.Plane().Axis().Direction().Dot(n)) > 1e-9) square = false;
        neighbours.push_back(TopoDS::Face(g));
      } else if (surface.GetType() == GeomAbs_Cylinder && surface.Cylinder().Axis().Direction().IsParallel(n, 1e-9)) {
        square = false;
        neighbours.clear();
        break;
      } else if (revolution_axis(surface, a) && face_plane.Contains(a.Location(), 1e-9) &&
                 std::abs(a.Direction().Dot(n)) < 1e-9 &&
                 (!revolved || (axis.Direction().IsParallel(a.Direction(), 1e-9) &&
                                gp_Lin(axis).Distance(a.Location()) < 1e-9))) {
        revolved = true;
        axis = a;
      } else {
        throw std::runtime_error("replace face: a neighbouring face isn't planar, cylindrical or revolved");
      }
    }
  }
  if (revolved) {
    // Its planar neighbours must be square to the axis, carried round by the revolution.
    for (const TopoDS_Face &g : neighbours)
      if (!BRepAdaptor_Surface(g).Plane().Axis().Direction().IsParallel(axis.Direction(), 1e-9))
        throw std::runtime_error("replace face: a plane next to a revolved end isn't square to its axis");
  }
  Bnd_Box bounds;
  BRepBndLib::Add(solid.shape, bounds);
  double x0, y0, z0, x1, y1, z1;
  bounds.Get(x0, y0, z0, x1, y1, z1);
  double size = gp_Pnt(x0, y0, z0).Distance(gp_Pnt(x1, y1, z1));
  gp_Pnt origin = pnt(target.origin);
  double reach = 10 * size + 10 * origin.Distance(gp_Pnt((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2)) + 1;
  // The face's own column, beyond it (to grow into) and behind it (to cut back): along the face's normal when the
  // faces round it are square to it; otherwise, where leaning planes are round it, the region inside all their
  // planes on either side of the face's (batch 058 c003, a trapezoid prism's top); for a revolved end, turned on
  // round the axis either way. Only that column changes: what stands on the face past the target plane stays
  // (batch 144 c006, a boss on a block's top replaced by a plane below the boss's top).
  auto column = [&](bool beyond) -> TopoDS_Shape {
    if (revolved) {
      // Turned round the axis the way the face's normal points (or against it), a third of a turn.
      GProp_GProps props;
      BRepGProp::SurfaceProperties(f, props);
      const gp_Vec radial(axis.Location(), props.CentreOfMass());
      double sign = gp_Vec(axis.Direction()).Crossed(radial).Dot(gp_Vec(n)) >= 0 ? 1 : -1;
      if (!beyond) sign = -sign;
      TopoDS_Shape copy = BRepBuilderAPI_Copy(f).Shape();
      return BRepPrimAPI_MakeRevol(copy, axis, sign * 2 * M_PI / 3, Standard_True).Shape();
    }
    if (square || neighbours.empty()) {
      TopoDS_Shape copy = BRepBuilderAPI_Copy(f).Shape();
      return BRepPrimAPI_MakePrism(copy, gp_Vec(n) * (beyond ? reach : -reach), Standard_True).Shape();
    }
    const gp_Pnt c((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
    TopoDS_Shape region = BRepPrimAPI_MakeBox(gp_Pnt(c.X() - reach, c.Y() - reach, c.Z() - reach), 2 * reach, 2 * reach, 2 * reach).Shape();
    auto keep_side = [&](const TopoDS_Face &plane_face, const gp_Pnt &inside) {
      BRepAdaptor_Surface surface(plane_face);
      TopoDS_Face big = BRepBuilderAPI_MakeFace(surface.Plane(), -4 * reach, 4 * reach, -4 * reach, 4 * reach).Face();
      BRepAlgoAPI_Common common(region, BRepPrimAPI_MakeHalfSpace(big, inside).Solid());
      common.SetRunParallel(Standard_False);
      common.Build();
      if (!common.IsDone()) throw std::runtime_error("replace face: extending failed");
      region = common.Shape();
    };
    for (const TopoDS_Face &g : neighbours) {
      // A point just inside the solid from the neighbour.
      const gp_Dir gn = face_normal(g);
      GProp_GProps props;
      BRepGProp::SurfaceProperties(g, props);
      keep_side(g, props.CentreOfMass().Translated(gp_Vec(gn) * (-1e-3 * size)));
    }
    GProp_GProps props;
    BRepGProp::SurfaceProperties(f, props);
    keep_side(f, props.CentreOfMass().Translated(gp_Vec(n) * ((beyond ? 1e-3 : -1e-3) * size)));
    return region;
  };
  gp_Dir tn = dir(target.normal);
  if (tn.Dot(n) < 0) tn.Reverse();
  // A region's part behind the target plane (against its normal turned to agree with the face's), or beyond it.
  auto clipped = [&](const TopoDS_Shape &region, bool behind) -> TopoDS_Shape {
    TopoDS_Face big = BRepBuilderAPI_MakeFace(gp_Pln(origin, tn), -4 * reach, 4 * reach, -4 * reach, 4 * reach).Face();
    const gp_Pnt inside = origin.Translated(gp_Vec(tn) * (behind ? -reach : reach));
    BRepAlgoAPI_Common common(region, BRepPrimAPI_MakeHalfSpace(big, inside).Solid());
    common.SetRunParallel(Standard_False);
    common.Build();
    if (!common.IsDone()) throw std::runtime_error("replace face: extending failed");
    GProp_GProps props;
    BRepGProp::VolumeProperties(common.Shape(), props);
    return props.Mass() > 1e-18 ? common.Shape() : TopoDS_Shape();
  };
  const TopoDS_Shape grown = clipped(column(true), true);
  const TopoDS_Shape cut_back = clipped(column(false), false);
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History();
  TopoDS_Shape current = solid.shape;
  if (!grown.IsNull()) {
    BRepAlgoAPI_Fuse fuse(current, grown);
    fuse.SetRunParallel(Standard_False);
    fuse.Build();
    if (!fuse.IsDone()) throw std::runtime_error("replace face: extending failed");
    TopTools_ListOfShape arguments = one(current);
    arguments.Append(grown);
    op->history->Merge(arguments, fuse);
    current = fuse.Shape();
  }
  if (!cut_back.IsNull()) {
    BRepAlgoAPI_Cut cut(current, cut_back);
    cut.SetRunParallel(Standard_False);
    cut.Build();
    if (!cut.IsDone()) throw std::runtime_error("replace face: cutting failed");
    TopTools_ListOfShape arguments = one(current);
    arguments.Append(cut_back);
    op->history->Merge(arguments, cut);
    current = cut.Shape();
  }
  int count = 0;
  for (TopExp_Explorer it(current, TopAbs_SOLID); it.More(); it.Next()) ++count;
  if (count == 0) throw std::runtime_error("replace face: nothing left");
  ShapeUpgrade_UnifySameDomain unifier(current, Standard_True, Standard_True, Standard_False);
  unifier.Build();
  op->history->Merge(unifier.History());
  // An edge carried on through the column is one edge: the column's arcs round a revolved end can run the other
  // way round their circle than the solid's, which the merge above leaves in two (a frame's mitre, batch 138 c005).
  const TopoDS_Shape fused = edges_fused(unifier.Shape(), op->history);
  TopExp_Explorer first(fused, TopAbs_SOLID);
  op->result = count == 1 && first.More() ? first.Current() : fused;
  return op;
}

// Replaces a planar face of a solid with the surface of another face (offset by `offset` along its normal): the
// solid is extended past the face along its normal, split by the whole surface, and what's behind the surface
// (against its normal turned to agree with the face's) is kept; faces left on one surface are merged.
std::unique_ptr<Operation> replace_face_with_surface_op(const Shape &solid, const Shape &face, const Shape &template_face, double offset) {
  const TopoDS_Face &f = TopoDS::Face(face.shape);
  if (BRepAdaptor_Surface(f).GetType() != GeomAbs_Plane) throw std::runtime_error("replace face: not planar");
  gp_Dir n = face_normal(f);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(TopoDS::Face(template_face.shape));
  if (std::abs(offset) > 0) surface = new Geom_OffsetSurface(surface, offset);
  Bnd_Box bounds;
  BRepBndLib::Add(solid.shape, bounds);
  BRepBndLib::Add(template_face.shape, bounds);
  double x0, y0, z0, x1, y1, z1;
  bounds.Get(x0, y0, z0, x1, y1, z1);
  double reach = 10 * gp_Pnt(x0, y0, z0).Distance(gp_Pnt(x1, y1, z1)) + 1;
  TopoDS_Shape copy = BRepBuilderAPI_Copy(f).Shape();
  BRepPrimAPI_MakePrism prism(copy, gp_Vec(n) * reach, Standard_True);
  BRepAlgoAPI_Fuse fuse(solid.shape, prism.Shape());
  fuse.SetRunParallel(Standard_False);
  fuse.Build();
  if (!fuse.IsDone()) throw std::runtime_error("replace face: extending failed");
  // The whole surface, its infinite directions cut off far away.
  double u0, u1, v0, v1;
  surface->Bounds(u0, u1, v0, v1);
  auto clamp = [&](double x) { return std::max(-reach * 4, std::min(reach * 4, x)); };
  TopoDS_Face cutter = BRepBuilderAPI_MakeFace(surface, clamp(u0), clamp(u1), clamp(v0), clamp(v1), Precision::Confusion()).Face();
  BRepAlgoAPI_Splitter splitter;
  splitter.SetArguments(one(fuse.Shape()));
  splitter.SetTools(one(cutter));
  splitter.SetRunParallel(Standard_False);
  splitter.Build();
  if (splitter.HasErrors()) throw std::runtime_error("replace face: cutting failed");
  BRep_Builder builder;
  TopoDS_Compound kept;
  builder.MakeCompound(kept);
  int count = 0;
  for (TopExp_Explorer it(splitter.Shape(), TopAbs_SOLID); it.More(); it.Next()) {
    GProp_GProps props;
    BRepGProp::VolumeProperties(it.Current(), props);
    gp_Pnt q = props.CentreOfMass();
    GeomAPI_ProjectPointOnSurf projection(q, surface);
    if (projection.NbPoints() == 0) continue;
    double u, v;
    projection.LowerDistanceParameters(u, v);
    GeomLProp_SLProps local(surface, u, v, 1, 1e-9);
    if (!local.IsNormalDefined()) continue;
    gp_Dir normal = local.Normal();
    if (normal.Dot(n) < 0) normal.Reverse();
    if (gp_Vec(projection.NearestPoint(), q).Dot(gp_Vec(normal)) < 0) {
      builder.Add(kept, it.Current());
      ++count;
    }
  }
  if (count == 0) throw std::runtime_error("replace face: nothing left");
  ShapeUpgrade_UnifySameDomain unifier(kept, Standard_True, Standard_True, Standard_False);
  unifier.Build();
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape arguments = one(solid.shape);
  arguments.Append(prism.Shape());
  op->history = new BRepTools_History(arguments, fuse);
  op->history->Merge(one(fuse.Shape()), splitter);
  op->history->Merge(unifier.History());
  TopExp_Explorer first(unifier.Shape(), TopAbs_SOLID);
  op->result = count == 1 && first.More() ? first.Current() : unifier.Shape();
  return op;
}

// A solid from a shape: its first solid, or one made from its closed shells.
std::unique_ptr<Shape> solid_from(const Shape &s) {
  TopExp_Explorer solids(s.shape, TopAbs_SOLID);
  if (solids.More()) return wrap(solids.Current());
  BRep_Builder builder;
  TopoDS_Solid solid;
  builder.MakeSolid(solid);
  bool any = false;
  for (TopExp_Explorer it(s.shape, TopAbs_SHELL); it.More(); it.Next()) {
    builder.Add(solid, it.Current());
    any = true;
  }
  if (!any) throw std::runtime_error("no shell to make a solid from");
  return wrap(solid);
}

// The solid a set of faces of a solid bounds once its open boundary is capped with planar faces: the material of a
// boss, or the void of a pocket, that the faces make.
std::unique_ptr<Shape> face_feature_solid(const ShapeList &faces) {
  BRep_Builder builder;
  TopoDS_Shell shell;
  builder.MakeShell(shell);
  for (const TopoDS_Shape &f : faces.shapes) builder.Add(shell, f);
  ShapeAnalysis_FreeBounds bounds(shell, 1e-7, Standard_False, Standard_False);
  BRepBuilderAPI_Sewing sewing(1e-7);
  for (const TopoDS_Shape &f : faces.shapes) sewing.Add(f);
  for (TopExp_Explorer it(bounds.GetClosedWires(), TopAbs_WIRE); it.More(); it.Next()) {
    BRepBuilderAPI_MakeFace cap(TopoDS::Wire(it.Current()), Standard_True);
    if (!cap.IsDone()) throw std::runtime_error("face feature: the boundary isn't planar");
    sewing.Add(cap.Face());
  }
  sewing.Perform();
  TopoDS_Shape sewn = sewing.SewedShape();
  TopExp_Explorer shells(sewn, TopAbs_SHELL);
  if (!shells.More()) throw std::runtime_error("face feature: no shell");
  ShapeFix_Solid fixer;
  TopoDS_Solid solid = fixer.SolidFromShell(TopoDS::Shell(shells.Current()));
  if (solid.IsNull()) throw std::runtime_error("face feature: no solid");
  return wrap(solid);
}

// Splits faces of a body where tools (sheets, faces, wires) cross them, leaving the rest of the body as it is.
// Splits `faces` of `body` where `tools` cross them. With `owners`, tool i cuts only face `owners[i]` (or every
// face if that's negative), and tools that cross nothing leave the body as it was rather than failing.
static std::unique_ptr<Operation> split_faces_impl(const Shape &body, const ShapeList &faces, const ShapeList &tools,
                                                   const rust::Slice<const int32_t> *owners) {
  BRepFeat_SplitShape splitter(body.shape);
  bool any = false;
  TopoDS_Compound tool;
  BRep_Builder builder;
  builder.MakeCompound(tool);
  for (const TopoDS_Shape &t : tools.shapes) builder.Add(tool, t);
  // One section of all the faces, so cuts meeting where faces meet share their vertices; each section edge goes to
  // the face it lies on.
  TopoDS_Compound targets;
  builder.MakeCompound(targets);
  for (const TopoDS_Shape &f : faces.shapes) builder.Add(targets, f);
  BRepAlgoAPI_Section section(targets, tool, Standard_False);
  section.ComputePCurveOn1(Standard_True);
  section.Approximation(Standard_True);
  section.Build();
  // Whether an edge runs along one of a face's own edges: the tool meets the face where it's already cut.
  auto on_boundary = [](const TopoDS_Edge &edge, const TopoDS_Shape &face) {
    BRepAdaptor_Curve curve(edge);
    double t0 = curve.FirstParameter(), t1 = curve.LastParameter();
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      bool along = true;
      for (double f : {0.0, 0.25, 0.5, 0.75, 1.0}) {
        TopoDS_Vertex v = BRepBuilderAPI_MakeVertex(curve.Value(t0 + f * (t1 - t0)));
        BRepExtrema_DistShapeShape d(v, e.Current());
        if (!d.IsDone() || d.Value() > 1e-7) {
          along = false;
          break;
        }
      }
      if (along) return true;
    }
    return false;
  };
  // Each tool's faces, to tell which tool a section edge came from.
  std::vector<TopTools_IndexedMapOfShape> tool_faces(tools.shapes.size());
  if (owners)
    for (size_t i = 0; i < tools.shapes.size(); i++) TopExp::MapShapes(tools.shapes[i], TopAbs_FACE, tool_faces[i]);
  auto allowed = [&](const TopoDS_Shape &edge, const TopoDS_Shape &face) {
    if (!owners) return true;
    TopoDS_Shape tool_face;
    if (!section.HasAncestorFaceOn2(edge, tool_face)) return false;
    for (size_t i = 0; i < tools.shapes.size(); i++) {
      if (!tool_faces[i].Contains(tool_face)) continue;
      int32_t owner = (*owners)[i];
      if (owner < 0 || faces.shapes[owner].IsSame(face)) return true;
    }
    return false;
  };
  bool crossed = false;
  if (section.IsDone())
    for (TopExp_Explorer it(section.Shape(), TopAbs_EDGE); it.More(); it.Next()) {
      // Edges where the tools meet the faces' edges have no single face to go to.
      crossed = true;
      TopoDS_Shape face;
      if (!section.HasAncestorFaceOn1(it.Current(), face)) continue;
      if (!allowed(it.Current(), face)) continue;
      if (on_boundary(TopoDS::Edge(it.Current()), face)) continue;
      splitter.Add(TopoDS::Edge(it.Current()), TopoDS::Face(face));
      any = true;
    }
  if (!crossed && !owners) throw std::runtime_error("split faces: the tools don't cross the faces");
  if (!any) {
    // The tools only meet the faces along edges they already have: nothing changes.
    auto op = std::make_unique<Operation>();
    op->history = new BRepTools_History;
    op->result = body.shape;
    return op;
  }
  splitter.Build();
  return finish(one(body.shape), splitter, "split faces");
}

std::unique_ptr<Operation> split_faces_op(const Shape &body, const ShapeList &faces, const ShapeList &tools) {
  return split_faces_impl(body, faces, tools, nullptr);
}

// Merges faces of a body that lie on one surface across the `removable` edges between them, and edges left on one
// curve where those edges met them; every other edge stays.
std::unique_ptr<Operation> merge_faces_across_op(const Shape &body, const ShapeList &removable, bool fuse_edges) {
  TopTools_IndexedMapOfShape remove;
  for (const TopoDS_Shape &e : removable.shapes) remove.Add(e);
  TopTools_MapOfShape keep;
  for (TopExp_Explorer it(body.shape, TopAbs_EDGE); it.More(); it.Next())
    if (!remove.Contains(it.Current())) keep.Add(it.Current());
  ShapeUpgrade_UnifySameDomain unifier(body.shape, Standard_True, Standard_True, Standard_False);
  unifier.KeepShapes(keep);
  unifier.Build();
  auto op = std::make_unique<Operation>();
  op->history = unifier.History();
  // With `fuse_edges`, pieces of a curve are one edge where only the same two faces meet them (a circle projected
  // onto a cylinder across its seam, batch 133 c001).
  op->result = fuse_edges ? edges_fused(unifier.Shape(), op->history) : unifier.Shape();
  return op;
}

std::unique_ptr<Operation> split_faces_paired_op(const Shape &body, const ShapeList &faces, const ShapeList &tools,
                                                 rust::Slice<const int32_t> owners) {
  if (owners.size() != tools.shapes.size()) throw std::runtime_error("split faces: one owner per tool");
  return split_faces_impl(body, faces, tools, &owners);
}

// A body of one doubly periodic face (a whole torus) split along two closed curves that each go once round it the
// first way (u) and not the second (batch 058 c005): two faces, the bands between the curves either way round in
// v, each built from a loop in the surface's parameters (one curve, the seam at the end of u, the other curve
// back, the seam at its start) and sewn together, so the seams don't cut them up as a general split does.
std::unique_ptr<Operation> split_by_bands_op(const Shape &body, const Shape &face, rust::Slice<const Vec3> first,
                                             rust::Slice<const Vec3> second) {
  const TopoDS_Face &f = TopoDS::Face(face.shape);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(f);
  const double period = 2 * M_PI;
  // Each curve's (u, v), unwrapped along it, closed by its first point a period on in u.
  auto parameters = [&](rust::Slice<const Vec3> points) {
    std::vector<gp_Pnt2d> out;
    for (const Vec3 &p : points) {
      GeomAPI_ProjectPointOnSurf projection(pnt(p), surface);
      if (projection.NbPoints() == 0) throw std::runtime_error("split by bands: a point isn't on the face");
      double u, v;
      projection.LowerDistanceParameters(u, v);
      if (!out.empty()) {
        const gp_Pnt2d &last = out.back();
        while (u - last.X() > M_PI) u -= period;
        while (last.X() - u > M_PI) u += period;
        while (v - last.Y() > M_PI) v -= period;
        while (last.Y() - v > M_PI) v += period;
      }
      out.emplace_back(u, v);
    }
    const double turn = out.back().X() - out.front().X();
    const double ahead = turn > 0 ? period : -period;
    out.emplace_back(out.front().X() + ahead, out.front().Y());
    return out;
  };
  std::vector<gp_Pnt2d> a = parameters(first), b = parameters(second);
  // b starts on the same meridian as a: put it above a in v, less than a period.
  const double du = b.front().X() - a.front().X();
  for (gp_Pnt2d &p : b) p.SetX(p.X() - std::round(du / period) * period);
  while (b.front().Y() < a.front().Y()) for (gp_Pnt2d &p : b) p.SetY(p.Y() + period);
  while (b.front().Y() > a.front().Y() + period) for (gp_Pnt2d &p : b) p.SetY(p.Y() - period);
  auto flat = [](const std::vector<gp_Pnt2d> &points, double dv, bool reverse, std::vector<double> &uv) {
    for (size_t i = 0; i < points.size(); ++i) {
      const gp_Pnt2d &p = points[reverse ? points.size() - 1 - i : i];
      uv.push_back(p.X());
      uv.push_back(p.Y() + dv);
    }
    return static_cast<uint32_t>(points.size());
  };
  auto band = [&](const std::vector<gp_Pnt2d> &low, double low_dv, const std::vector<gp_Pnt2d> &high, double high_dv) {
    std::vector<double> uv;
    std::vector<uint32_t> counts;
    counts.push_back(flat(low, low_dv, false, uv));
    const gp_Pnt2d &l1 = low.back(), &h1 = high.back(), &l0 = low.front(), &h0 = high.front();
    uv.insert(uv.end(), {l1.X(), l1.Y() + low_dv, h1.X(), h1.Y() + high_dv});
    counts.push_back(2);
    counts.push_back(flat(high, high_dv, true, uv));
    uv.insert(uv.end(), {h0.X(), h0.Y() + high_dv, l0.X(), l0.Y() + low_dv});
    counts.push_back(2);
    return face_from_uv(face, rust::Slice<const double>(uv.data(), uv.size()),
                        rust::Slice<const uint32_t>(counts.data(), counts.size()))->shape;
  };
  const TopoDS_Shape lower = band(a, 0, b, 0), upper = band(b, 0, a, period);
  BRepBuilderAPI_Sewing sewing(1e-6);
  sewing.Add(lower);
  sewing.Add(upper);
  sewing.Perform();
  TopoDS_Shell shell;
  for (TopExp_Explorer it(sewing.SewedShape(), TopAbs_SHELL); it.More(); it.Next()) shell = TopoDS::Shell(it.Current());
  if (shell.IsNull()) throw std::runtime_error("split by bands: the bands don't close");
  BRepBuilderAPI_MakeSolid made(shell);
  ShapeFix_Solid fix(made.Solid());
  fix.Perform();
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History;
  op->result = fix.Solid();
  for (TopExp_Explorer it(op->result, TopAbs_FACE); it.More(); it.Next()) op->history->AddModified(f, it.Current());
  (void)body;
  return op;
}

// A body's face on a surface periodic the second way (v) but not the first (part of a torus round its axis) split
// along two curves that each run from one end of the face in u to the other (batch 071 c010): the two bands between
// them either way round in v, each built from a loop in the surface's parameters (one curve, the face's end, the
// other curve back, its other end), so the seam doesn't cut them up; sewn with the body's other faces.
std::unique_ptr<Operation> split_by_open_bands_op(const Shape &body, const Shape &face, rust::Slice<const Vec3> first,
                                                  rust::Slice<const Vec3> second) {
  const TopoDS_Face &f = TopoDS::Face(face.shape);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(f);
  const double period = 2 * M_PI;
  auto parameters = [&](rust::Slice<const Vec3> points) {
    std::vector<gp_Pnt2d> out;
    for (const Vec3 &p : points) {
      GeomAPI_ProjectPointOnSurf projection(pnt(p), surface);
      if (projection.NbPoints() == 0) throw std::runtime_error("split by open bands: a point isn't on the face");
      double u, v;
      projection.LowerDistanceParameters(u, v);
      if (!out.empty()) {
        const gp_Pnt2d &last = out.back();
        while (v - last.Y() > M_PI) v -= period;
        while (last.Y() - v > M_PI) v += period;
      }
      out.emplace_back(u, v);
    }
    if (out.size() > 1 && out.front().X() > out.back().X()) std::reverse(out.begin(), out.end());
    return out;
  };
  std::vector<gp_Pnt2d> a = parameters(first), b = parameters(second);
  while (b.front().Y() < a.front().Y()) for (gp_Pnt2d &p : b) p.SetY(p.Y() + period);
  while (b.front().Y() > a.front().Y() + period) for (gp_Pnt2d &p : b) p.SetY(p.Y() - period);
  auto flat = [](const std::vector<gp_Pnt2d> &points, double dv, bool reverse, std::vector<double> &uv) {
    for (size_t i = 0; i < points.size(); ++i) {
      const gp_Pnt2d &p = points[reverse ? points.size() - 1 - i : i];
      uv.push_back(p.X());
      uv.push_back(p.Y() + dv);
    }
    return static_cast<uint32_t>(points.size());
  };
  auto band = [&](const std::vector<gp_Pnt2d> &low, double low_dv, const std::vector<gp_Pnt2d> &high, double high_dv) {
    std::vector<double> uv;
    std::vector<uint32_t> counts;
    counts.push_back(flat(low, low_dv, false, uv));
    const gp_Pnt2d &l1 = low.back(), &h1 = high.back(), &l0 = low.front(), &h0 = high.front();
    uv.insert(uv.end(), {l1.X(), l1.Y() + low_dv, h1.X(), h1.Y() + high_dv});
    counts.push_back(2);
    counts.push_back(flat(high, high_dv, true, uv));
    uv.insert(uv.end(), {h0.X(), h0.Y() + high_dv, l0.X(), l0.Y() + low_dv});
    counts.push_back(2);
    return face_from_uv(face, rust::Slice<const double>(uv.data(), uv.size()),
                        rust::Slice<const uint32_t>(counts.data(), counts.size()))->shape;
  };
  const TopoDS_Shape lower = band(a, 0, b, 0), upper = band(b, 0, a, period);
  BRepBuilderAPI_Sewing sewing(1e-6);
  sewing.Add(lower);
  sewing.Add(upper);
  std::vector<TopoDS_Shape> others;
  for (TopExp_Explorer it(body.shape, TopAbs_FACE); it.More(); it.Next())
    if (!it.Current().IsSame(f)) {
      others.push_back(it.Current());
      sewing.Add(it.Current());
    }
  sewing.Perform();
  TopoDS_Shell shell;
  for (TopExp_Explorer it(sewing.SewedShape(), TopAbs_SHELL); it.More(); it.Next()) shell = TopoDS::Shell(it.Current());
  if (shell.IsNull()) throw std::runtime_error("split by open bands: the faces don't close");
  BRepBuilderAPI_MakeSolid made(shell);
  ShapeFix_Solid fix(made.Solid());
  fix.Perform();
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History;
  op->result = fix.Solid();
  // The bands came from the face; each other face is its sewn copy.
  for (TopExp_Explorer it(op->result, TopAbs_FACE); it.More(); it.Next()) {
    const TopoDS_Shape &g = it.Current();
    bool from_other = false;
    for (const TopoDS_Shape &o : others)
      if (sewing.IsModified(o) ? sewing.Modified(o).IsSame(g) : o.IsSame(g)) {
        if (!o.IsSame(g)) op->history->AddModified(o, g);
        from_other = true;
      }
    if (!from_other) op->history->AddModified(f, g);
  }
  // The ends' circles keep their old vertex where they closed, between two arcs of one circle: joined (Onshape's
  // circles end only at the curves).
  ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_False, Standard_False);
  unifier.Build();
  op->history->Merge(unifier.History());
  op->result = unifier.Shape();
  return op;
}

// A body with one of its faces split along edges lying on it (a wrap's imprint).
std::unique_ptr<Operation> split_face_by_edges_op(const Shape &body, const Shape &face, const ShapeList &edges) {
  BRepFeat_SplitShape splitter(body.shape);
  for (const TopoDS_Shape &e : edges.shapes) splitter.Add(TopoDS::Edge(e), TopoDS::Face(face.shape));
  splitter.Build();
  return finish(one(body.shape), splitter, "split face by edges");
}

// Rounds faces between two parallel planar side faces into a half cylinder tangent to both, as wide as the gap
// between them: the solid is kept below the plane through the cylinder's axis, and inside the cylinder above it.
std::unique_ptr<Operation> full_round_op(const Shape &solid, const Shape &side1, const Shape &side2, const ShapeList &centre) {
  const TopoDS_Face &f1 = TopoDS::Face(side1.shape), &f2 = TopoDS::Face(side2.shape);
  if (BRepAdaptor_Surface(f1).GetType() != GeomAbs_Plane || BRepAdaptor_Surface(f2).GetType() != GeomAbs_Plane)
    throw std::runtime_error("full round: the side faces must be planar");
  gp_Pln p1 = BRepAdaptor_Surface(f1).Plane(), p2 = BRepAdaptor_Surface(f2).Plane();
  if (!p1.Axis().Direction().IsParallel(p2.Axis().Direction(), 1e-9)) throw std::runtime_error("full round: sides aren't parallel");
  gp_Dir n = p1.Axis().Direction();
  double gap = p1.Distance(p2.Location());
  double r = gap / 2;
  // The middle plane between the sides.
  gp_Vec across(p1.Location(), p2.Location());
  double side = across.Dot(gp_Vec(n));
  gp_Pnt mid = p1.Location().Translated(gp_Vec(n) * (side / 2));
  if (centre.shapes.empty()) throw std::runtime_error("full round: no faces to round");
  // The centre faces' outward direction (taken from the first), and how far they reach along it.
  const TopoDS_Face &c0 = TopoDS::Face(centre.shapes[0]);
  gp_Dir out = face_normal(c0);
  gp_Dir axis(gp_Vec(n).Crossed(gp_Vec(out)));
  out = gp_Dir(gp_Vec(axis).Crossed(gp_Vec(n)));
  if (out.Dot(face_normal(c0)) < 0) out.Reverse();
  Bnd_Box bounds;
  BRepBndLib::Add(solid.shape, bounds);
  double x0, y0, z0, x1, y1, z1;
  bounds.Get(x0, y0, z0, x1, y1, z1);
  double size = gp_Pnt(x0, y0, z0).Distance(gp_Pnt(x1, y1, z1)) + 1;
  double top = -1e300;
  for (const TopoDS_Shape &c : centre.shapes) {
    Bnd_Box b;
    BRepBndLib::AddOptimal(c, b, Standard_False, Standard_False);
    double a0, b0, d0, a1, b1, d1;
    b.Get(a0, b0, d0, a1, b1, d1);
    for (double xx : {a0, a1})
      for (double yy : {b0, b1})
        for (double zz : {d0, d1}) top = std::max(top, gp_Vec(mid, gp_Pnt(xx, yy, zz)).Dot(gp_Vec(out)));
  }
  gp_Pnt axis_point = mid.Translated(gp_Vec(out) * (top - r));
  // What's kept: the half space behind the axis plane, and the cylinder.
  gp_Ax2 frame(axis_point.Translated(gp_Vec(axis) * -size), axis);
  TopoDS_Shape cylinder = BRepPrimAPI_MakeCylinder(frame, r, 2 * size).Shape();
  gp_Ax3 below(axis_point, out);
  TopoDS_Shape half = BRepPrimAPI_MakeHalfSpace(
                          BRepBuilderAPI_MakeFace(gp_Pln(below), -2 * size, 2 * size, -2 * size, 2 * size).Face(),
                          axis_point.Translated(gp_Vec(out) * -1))
                          .Solid();
  BRepAlgoAPI_Fuse keep(half, cylinder);
  keep.Build();
  if (!keep.IsDone()) throw std::runtime_error("full round: building the tool failed");
  BRepAlgoAPI_Common common(solid.shape, keep.Shape());
  common.Build();
  if (!common.IsDone()) throw std::runtime_error("full round failed");
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History(one(solid.shape), common);
  ShapeUpgrade_UnifySameDomain unifier(common.Shape(), Standard_True, Standard_True, Standard_False);
  unifier.Build();
  op->history->Merge(unifier.History());
  op->result = unifier.Shape();
  TopExp_Explorer first(op->result, TopAbs_SOLID);
  if (first.More()) op->result = first.Current();
  return op;
}

// A rolling ball blend between two sets of faces that meet along edges: the faces (each set reversed if asked, so
// their normals point away from the blend) are sewn into one sheet and the edges where the sets meet are filleted.
std::unique_ptr<Operation> face_blend_op(const ShapeList &side1, bool reverse1, const ShapeList &side2, bool reverse2,
                                         double radius) {
  BRepBuilderAPI_Sewing sewing(1e-6);
  for (const TopoDS_Shape &f : side1.shapes) sewing.Add(reverse1 ? f.Reversed() : f);
  for (const TopoDS_Shape &f : side2.shapes) sewing.Add(reverse2 ? f.Reversed() : f);
  sewing.Perform();
  TopoDS_Shape sewn = sewing.SewedShape();
  // The edges shared by a face of each side.
  TopTools_IndexedMapOfShape faces1, faces2;
  for (const TopoDS_Shape &f : side1.shapes) faces1.Add(sewing.Modified(reverse1 ? f.Reversed() : f));
  for (const TopoDS_Shape &f : side2.shapes) faces2.Add(sewing.Modified(reverse2 ? f.Reversed() : f));
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(sewn, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  BRepFilletAPI_MakeFillet fillet(sewn);
  int added = 0;
  for (int i = 1; i <= edge_faces.Extent(); ++i) {
    bool in1 = false, in2 = false;
    for (const TopoDS_Shape &f : edge_faces(i)) {
      if (faces1.Contains(f)) in1 = true;
      if (faces2.Contains(f)) in2 = true;
    }
    if (in1 && in2) {
      fillet.Add(radius, TopoDS::Edge(edge_faces.FindKey(i)));
      ++added;
    }
  }
  if (added == 0) throw std::runtime_error("face blend: the sides don't meet");
  fillet.Build();
  if (!fillet.IsDone()) throw std::runtime_error("face blend failed");
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History(one(sewn), fillet);
  op->result = fillet.Shape();
  return op;
}

// Whether a point is inside a solid.
bool point_in_solid(const Shape &solid, Vec3 p) {
  BRepClass3d_SolidClassifier classifier(solid.shape, pnt(p), 1e-9);
  return classifier.State() == TopAbs_IN;
}

// The sheets of a shape: its faces, grouped into one shell for each set connected through shared edges.
std::unique_ptr<ShapeList> connected_shells(const Shape &s) {
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(s.shape, TopAbs_FACE, faces);
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  std::vector<bool> placed(faces.Extent() + 1, false);
  auto out = std::make_unique<ShapeList>();
  BRep_Builder builder;
  for (int i = 1; i <= faces.Extent(); ++i) {
    if (placed[i]) continue;
    TopoDS_Shell shell;
    builder.MakeShell(shell);
    std::vector<int> stack{i};
    placed[i] = true;
    while (!stack.empty()) {
      int f = stack.back();
      stack.pop_back();
      builder.Add(shell, faces(f));
      for (TopExp_Explorer it(faces(f), TopAbs_EDGE); it.More(); it.Next())
        if (const TopTools_ListOfShape *neighbours = edge_faces.Seek(it.Current()))
          for (const TopoDS_Shape &g : *neighbours) {
            int j = faces.FindIndex(g);
            if (j > 0 && !placed[j]) {
              placed[j] = true;
              stack.push_back(j);
            }
          }
    }
    shell.Closed(BRep_Tool::IsClosed(shell));
    out->shapes.push_back(shell);
  }
  return out;
}

// The faces of a solid other than `faces`, sewn into a sheet: deleting faces and leaving the hole open.
std::unique_ptr<Shape> remove_faces_open(const Shape &solid, const ShapeList &faces) {
  BRep_Builder builder;
  TopoDS_Shell shell;
  builder.MakeShell(shell);
  TopTools_IndexedMapOfShape remove;
  for (const TopoDS_Shape &f : faces.shapes) remove.Add(f);
  for (TopExp_Explorer it(solid.shape, TopAbs_FACE); it.More(); it.Next())
    if (!remove.Contains(it.Current())) builder.Add(shell, it.Current());
  shell.Closed(Standard_False);
  return wrap(shell);
}

// Sweeps a profile (a face or a wire) along a path wire. `first` and `last` hold the faces at each end for a face
// profile.
// A sweep whose profile keeps its orientation (`mode` 1: a fixed trihedron), turns only about `direction` (`mode`
// 2: the trihedron's binormal along it) or follows a path with sharp corners (`mode` 0: a corrected Frenet
// trihedron, the pieces either side of a corner extended to meet, which mitres a constant profile, batch 034). The
// caps are the faces on the first and last sections.
static std::unique_ptr<Operation> sweep_controlled_impl(const Shape &profile, const TopoDS_Wire &path, int mode,
                                                        Vec3 direction, const TopoDS_Shape *support);

std::unique_ptr<Operation> sweep_controlled_op(const Shape &profile, const Shape &path, int mode, Vec3 direction) {
  return sweep_controlled_impl(profile, TopoDS::Wire(path.shape), mode, direction, nullptr);
}

// A sweep locked to faces (`mode` 3): the profile turns with the faces' normals along the path, which lies on them
// (a helix round a cylinder or a cone, a curve a split left on a cylinder or a sphere; batch 137). Each path edge
// gets its curve on the face it lies on, projected where it has none.
std::unique_ptr<Operation> sweep_supported_op(const Shape &profile, const Shape &path, const ShapeList &faces) {
  BRepBuilderAPI_Copy copier(path.shape, Standard_False);
  TopoDS_Wire spine = TopoDS::Wire(copier.Shape());
  BRep_Builder builder;
  TopoDS_Compound support;
  builder.MakeCompound(support);
  for (const TopoDS_Shape &f : faces.shapes) builder.Add(support, f);
  for (TopExp_Explorer it(spine, TopAbs_EDGE); it.More(); it.Next()) {
    const TopoDS_Edge &edge = TopoDS::Edge(it.Current());
    BRepAdaptor_Curve curve(edge);
    const gp_Pnt mid = curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2);
    const TopoDS_Vertex probe = BRepBuilderAPI_MakeVertex(mid).Vertex();
    bool done = false;
    for (const TopoDS_Shape &f : faces.shapes) {
      const TopoDS_Face &face = TopoDS::Face(f);
      BRepExtrema_DistShapeShape d(probe, face);
      if (!d.IsDone() || d.Value() > 1e-6) continue;
      double first, last;
      if (!BRep_Tool::CurveOnSurface(edge, face, first, last).IsNull()) {
        done = true;
        break;
      }
      double a, b;
      Handle(Geom_Curve) c3d = BRep_Tool::Curve(edge, a, b);
      if (c3d.IsNull()) continue;
      TopLoc_Location location;
      Handle(Geom_Surface) surface = BRep_Tool::Surface(face, location);
      Handle(Geom2d_Curve) c2d = GeomProjLib::Curve2d(c3d, a, b, surface);
      if (c2d.IsNull()) continue;
      builder.UpdateEdge(edge, c2d, face, 1e-6);
      done = true;
      break;
    }
    if (!done) throw std::runtime_error("sweep: a path edge isn't on the faces it's locked to");
  }
  // A face profile of one loop: carried along the path in the frame of its tangent and the faces' normal, a copy at
  // each of 192 points along it (by arc length), lofted through (OCCT's own trihedron on a support drifts round a
  // cylinder: batch 137 c004 came out 7.8e-4 of its area off and lopsided). A closed path makes a ring.
  if (profile.shape.ShapeType() == TopAbs_FACE) {
    const TopoDS_Face &face = TopoDS::Face(profile.shape);
    int loops = 0;
    for (TopExp_Explorer it(face, TopAbs_WIRE); it.More(); it.Next()) ++loops;
    if (loops == 1) {
      BRepAdaptor_CompCurve curve(spine);
      const double length = GCPnts_AbscissaPoint::Length(curve, 1e-9);
      const bool closed = BRep_Tool::IsClosed(spine) || curve.Value(curve.FirstParameter()).Distance(curve.Value(curve.LastParameter())) < 1e-7;
      // The profile sits at one end of an open path: sweep from that end. On a closed path, from the point nearest it.
      bool backwards = false;
      double s_start = 0;
      if (closed) {
        GProp_GProps props;
        BRepGProp::SurfaceProperties(face, props);
        const TopoDS_Vertex centre = BRepBuilderAPI_MakeVertex(props.CentreOfMass()).Vertex();
        BRepExtrema_DistShapeShape nearest(centre, spine);
        if (nearest.IsDone() && nearest.NbSolution() > 0) {
          const gp_Pnt q = nearest.PointOnShape2(1);
          // Its arc length along the path, sampled finely then refined.
          double best = 1e300;
          const int coarse = 720;
          for (int k = 0; k < coarse; ++k) {
            GCPnts_AbscissaPoint along(1e-10, curve, length * k / coarse, curve.FirstParameter());
            if (!along.IsDone()) continue;
            const double d = curve.Value(along.Parameter()).Distance(q);
            if (d < best) best = d, s_start = length * k / coarse;
          }
          double lo = s_start - length / coarse, hi = s_start + length / coarse;
          for (int it = 0; it < 60; ++it) {
            const double m1 = lo + (hi - lo) / 3, m2 = hi - (hi - lo) / 3;
            auto at = [&](double s) {
              double w = std::fmod(s + length, length);
              GCPnts_AbscissaPoint along(1e-10, curve, w, curve.FirstParameter());
              return along.IsDone() ? curve.Value(along.Parameter()).Distance(q) : 1e300;
            };
            if (at(m1) < at(m2)) hi = m2; else lo = m1;
          }
          s_start = std::fmod((lo + hi) / 2 + length, length);
        }
      } else {
        BRepExtrema_DistShapeShape to_first(face, BRepBuilderAPI_MakeVertex(curve.Value(curve.FirstParameter())).Vertex());
        BRepExtrema_DistShapeShape to_last(face, BRepBuilderAPI_MakeVertex(curve.Value(curve.LastParameter())).Vertex());
        backwards = to_first.IsDone() && to_last.IsDone() && to_last.Value() + 1e-9 < to_first.Value();
      }
      auto frame_at = [&](double fraction) {
        double s = (backwards ? 1 - fraction : fraction) * length;
        if (closed) s = std::fmod(s_start + s, length);
        GCPnts_AbscissaPoint along(1e-10, curve, s, curve.FirstParameter());
        if (!along.IsDone()) throw std::runtime_error("sweep: along the path");
        gp_Pnt p;
        gp_Vec t;
        curve.D1(along.Parameter(), p, t);
        if (backwards) t.Reverse();
        gp_Vec n;
        double best = 1e300;
        for (const TopoDS_Shape &f : faces.shapes) {
          const TopoDS_Face &lock = TopoDS::Face(f);
          GeomAPI_ProjectPointOnSurf on(p, BRep_Tool::Surface(lock));
          if (on.NbPoints() == 0 || on.LowerDistance() >= best) continue;
          best = on.LowerDistance();
          double u, v;
          on.LowerDistanceParameters(u, v);
          BRepGProp_Face normals(lock);
          gp_Pnt q;
          normals.Normal(u, v, q, n);
        }
        if (best > 1e-6 || n.Magnitude() < 1e-12) throw std::runtime_error("sweep: off the faces");
        t.Normalize();
        n = n - t * n.Dot(t);
        n.Normalize();
        return gp_Ax3(p, gp_Dir(t), gp_Dir(n));
      };
      const int count = 192;
      const gp_Ax3 start = frame_at(0);
      std::vector<TopoDS_Wire> sections;
      const TopoDS_Wire outer = BRepTools::OuterWire(face);
      for (int k = 0; k <= count; ++k) {
        if (closed && k == count) {
          sections.push_back(sections.front());
          break;
        }
        // From the world into the start's frame, then out of this point's into the world.
        gp_Trsf into_start, into_here;
        into_start.SetTransformation(start);
        into_here.SetTransformation(frame_at(double(k) / count));
        const gp_Trsf carry = into_here.Inverted() * into_start;
        sections.push_back(TopoDS::Wire(BRepBuilderAPI_Transform(outer, carry, Standard_True).Shape()));
      }
      BRepOffsetAPI_ThruSections loft(Standard_True, Standard_False, 1e-7);
      loft.CheckCompatibility(Standard_False);
      for (const TopoDS_Wire &w : sections) loft.AddWire(w);
      loft.Build();
      if (!loft.IsDone()) throw std::runtime_error("sweep: loft");
      auto op = std::make_unique<Operation>();
      op->history = new BRepTools_History;
      if (closed) {
        TopTools_MapOfShape caps;
        caps.Add(loft.FirstShape());
        caps.Add(loft.LastShape());
        BRepBuilderAPI_Sewing ring(1e-7);
        for (TopExp_Explorer f(loft.Shape(), TopAbs_FACE); f.More(); f.Next())
          if (!caps.Contains(f.Current())) ring.Add(f.Current());
        ring.Perform();
        TopExp_Explorer shells(ring.SewedShape(), TopAbs_SHELL);
        if (!shells.More()) throw std::runtime_error("sweep: ring");
        BRepBuilderAPI_MakeSolid make(TopoDS::Shell(shells.Current()));
        TopoDS_Solid body = make.Solid();
        BRepLib::OrientClosedSolid(body);
        op->result = body;
      } else {
        TopoDS_Shape body = loft.Shape();
        op->result = body;
        op->first.push_back(loft.FirstShape());
        op->last.push_back(loft.LastShape());
      }
      if (!BRepCheck_Analyzer(op->result).IsValid()) throw std::runtime_error("sweep: invalid");
      return op;
    }
  }
  return sweep_controlled_impl(profile, spine, 3, Vec3{0, 0, 1}, &support);
}

static std::unique_ptr<Operation> sweep_controlled_impl(const Shape &profile, const TopoDS_Wire &path, int mode,
                                                        Vec3 direction, const TopoDS_Shape *support) {
  TopoDS_Wire spine = path;
  {
    TopoDS_Vertex first, last;
    TopExp::Vertices(spine, first, last);
    if (!first.IsNull() && !last.IsNull() && !first.IsSame(last)) {
      BRepExtrema_DistShapeShape to_first(profile.shape, first), to_last(profile.shape, last);
      if (to_first.IsDone() && to_last.IsDone() && to_last.Value() + 1e-9 < to_first.Value())
        spine = TopoDS::Wire(spine.Reversed());
    }
  }
  bool solid = profile.shape.ShapeType() == TopAbs_FACE;
  if (mode == 1 && solid) {
    // Kept as it is, a profile of straight edges sweeps exactly: each edge makes the path moved to it and
    // extruded along the edge, and the profile caps both ends. Curved faces become B-splines, as Onshape's are.
    const TopoDS_Face &face = TopoDS::Face(profile.shape);
    TopoDS_Vertex v0, v1;
    TopExp::Vertices(spine, v0, v1);
    gp_Pnt start = BRep_Tool::Pnt(v0), end = BRep_Tool::Pnt(v1);
    bool straight = true;
    for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next())
      straight = straight && BRepAdaptor_Curve(TopoDS::Edge(it.Current())).GetType() == GeomAbs_Line;
    if (straight) {
      BRepBuilderAPI_Sewing sewing(1e-7);
      for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
        const TopoDS_Edge &e = TopoDS::Edge(it.Current());
        gp_Pnt a = BRep_Tool::Pnt(TopExp::FirstVertex(e)), b = BRep_Tool::Pnt(TopExp::LastVertex(e));
        gp_Trsf move;
        move.SetTranslation(gp_Vec(start, a));
        TopoDS_Shape moved = BRepBuilderAPI_Transform(spine, move, Standard_True).Shape();
        BRepPrimAPI_MakePrism prism(moved, gp_Vec(a, b), Standard_True);
        sewing.Add(prism.Shape());
      }
      TopoDS_Shape first_cap = BRepBuilderAPI_Copy(face).Shape();
      gp_Trsf to_end;
      to_end.SetTranslation(gp_Vec(start, end));
      TopoDS_Shape last_cap = BRepBuilderAPI_Transform(face, to_end, Standard_True).Shape();
      sewing.Add(first_cap);
      sewing.Add(last_cap);
      sewing.Perform();
      TopoDS_Shell shell;
      for (TopExp_Explorer it(sewing.SewedShape(), TopAbs_SHELL); it.More(); it.Next()) shell = TopoDS::Shell(it.Current());
      if (shell.IsNull()) throw std::runtime_error("sweep: the faces don't close");
      BRepBuilderAPI_MakeSolid made(shell);
      TopoDS_Solid body = made.Solid();
      BRepLib::OrientClosedSolid(body);
      BRepBuilderAPI_NurbsConvert nurbs(body, Standard_True);
      auto op = std::make_unique<Operation>();
      op->history = new BRepTools_History;
      op->result = planes_recognized(nurbs.Shape(), op->history);
      TopoDS_Shape ends[2] = {first_cap, last_cap};
      for (int k = 0; k < 2; ++k) {
        GProp_GProps props;
        BRepGProp::SurfaceProperties(ends[k], props);
        gp_Pnt centre = props.CentreOfMass();
        for (TopExp_Explorer it(op->result, TopAbs_FACE); it.More(); it.Next()) {
          BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(centre).Vertex(), it.Current());
          if (d.IsDone() && d.Value() < 1e-7 && BRepAdaptor_Surface(TopoDS::Face(it.Current())).GetType() == GeomAbs_Plane)
            (k == 0 ? op->first : op->last).push_back(it.Current());
        }
      }
      return op;
    }
  }
  // Each of the profile's loops swept, the inner ones then cut from the outer (a square tube's profile, batch 030).
  std::vector<TopoDS_Wire> sections;
  if (solid) {
    const TopoDS_Face &face = TopoDS::Face(profile.shape);
    const TopoDS_Wire outer = BRepTools::OuterWire(face);
    sections.push_back(outer);
    for (TopExp_Explorer it(face, TopAbs_WIRE); it.More(); it.Next())
      if (!it.Current().IsSame(outer)) sections.push_back(TopoDS::Wire(it.Current()));
  } else {
    sections.push_back(TopoDS::Wire(profile.shape));
  }
  auto op = std::make_unique<Operation>();
  TopoDS_Compound ends[2];
  BRep_Builder builder;
  builder.MakeCompound(ends[0]);
  builder.MakeCompound(ends[1]);
  TopoDS_Shape outer_shape;
  TopTools_ListOfShape inner_shapes;
  for (size_t i = 0; i < sections.size(); ++i) {
    BRepOffsetAPI_MakePipeShell maker(spine);
    if (mode == 0) {
      maker.SetMode(Standard_False);
      maker.SetTransitionMode(BRepBuilderAPI_RightCorner);
    } else if (mode == 1) {
      maker.SetMode(gp_Ax2(gp::Origin(), gp::DZ(), gp::DX()));
    } else if (mode == 3 && support != nullptr) {
      maker.SetMode(*support);
    } else {
      maker.SetMode(gp_Dir(direction.x, direction.y, direction.z));
    }
    maker.Add(sections[i], Standard_False, Standard_False);
    maker.Build();
    if (!maker.IsDone()) throw std::runtime_error("sweep failed");
    if (solid && !maker.MakeSolid()) throw std::runtime_error("sweep: no solid");
    if (solid) {
      builder.Add(ends[0], maker.FirstShape());
      builder.Add(ends[1], maker.LastShape());
    }
    if (i == 0) {
      op->history = new BRepTools_History(one(sections[i]), maker);
      outer_shape = maker.Shape();
    } else {
      inner_shapes.Append(maker.Shape());
    }
  }
  op->result = outer_shape;
  if (!inner_shapes.IsEmpty()) {
    BRepAlgoAPI_Cut cut;
    cut.SetArguments(one(outer_shape));
    cut.SetTools(inner_shapes);
    cut.SetRunParallel(Standard_False);
    cut.Build();
    if (!cut.IsDone() || cut.HasErrors()) throw std::runtime_error("sweep: cutting the inner loops failed");
    op->history->Merge(one(outer_shape), cut);
    op->result = cut.Shape();
  }
  // Faces swept along consecutive path edges onto one surface are one face in Onshape (batch 034).
  {
    ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_True, Standard_False);
    unifier.Build();
    op->history->Merge(unifier.History());
    op->result = unifier.Shape();
  }
  if (solid) {
    for (int k = 0; k < 2; ++k) {
      for (TopExp_Explorer it(op->result, TopAbs_FACE); it.More(); it.Next()) {
        bool on_section = true;
        for (TopExp_Explorer e(it.Current(), TopAbs_EDGE); e.More() && on_section; e.Next()) {
          BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
          gp_Pnt mid = curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2);
          BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(mid).Vertex(), ends[k]);
          on_section = d.IsDone() && d.Value() < 1e-7;
        }
        if (on_section) (k == 0 ? op->first : op->last).push_back(it.Current());
      }
    }
  }
  return op;
}

std::unique_ptr<Operation> sweep_op(const Shape &profile, const Shape &path) {
  if (!wire_smooth(path, 1e-6)) return sweep_controlled_op(profile, path, 0, Vec3{0, 0, 1});
  // The profile is swept from the end of the path it sits at: if that's the far end, sweep along the path reversed.
  TopoDS_Wire spine = TopoDS::Wire(path.shape);
  {
    TopoDS_Vertex first, last;
    TopExp::Vertices(spine, first, last);
    if (!first.IsNull() && !last.IsNull() && !first.IsSame(last)) {
      BRepExtrema_DistShapeShape to_first(profile.shape, first), to_last(profile.shape, last);
      if (to_first.IsDone() && to_last.IsDone() && to_last.Value() + 1e-9 < to_first.Value())
        spine = TopoDS::Wire(spine.Reversed());
    }
  }
  // Along curves other than lines and arcs, OpenCascade fits the swept surfaces to 1e-4 in model units (here
  // metres: 0.065 mm off along a spline, batch 072 c002); swept a thousand times larger, then shrunk back, they fit
  // to a tenth of a micrometre.
  bool spline_path = false;
  for (TopExp_Explorer e(spine, TopAbs_EDGE); e.More(); e.Next()) {
    GeomAbs_CurveType type = BRepAdaptor_Curve(TopoDS::Edge(e.Current())).GetType();
    if (type != GeomAbs_Line && type != GeomAbs_Circle) spline_path = true;
  }
  // Along a path that leaves its plane with no straight stretch (a helix), the profile turns with the path's
  // Frenet frame, as Onshape's does (a square swept along a helix stays square to its axis, batch 124 c002);
  // OCCT's corrected Frenet frame would turn it against the torsion.
  GeomFill_Trihedron trihedron = GeomFill_IsCorrectedFrenet;
  if (spline_path && !BRepBuilderAPI_FindPlane(spine, 1e-7).Found()) {
    bool curved = true;
    for (TopExp_Explorer e(spine, TopAbs_EDGE); e.More() && curved; e.Next()) {
      BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
      for (int i = 0; i <= 32 && curved; ++i) {
        double t = curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * i / 32.0;
        BRepLProp_CLProps props(curve, t, 2, 1e-9);
        if (!props.IsTangentDefined() || props.Curvature() < 1e-3) curved = false;
      }
    }
    if (curved) trihedron = GeomFill_IsFrenet;
  }
  std::unique_ptr<Operation> op;
  if (spline_path) {
    gp_Trsf up;
    up.SetScale(gp_Pnt(0, 0, 0), 1000.0);
    BRepBuilderAPI_Transform big_profile(profile.shape, up, Standard_True);
    BRepBuilderAPI_Transform big_spine(spine, up, Standard_True);
    BRepOffsetAPI_MakePipe maker(TopoDS::Wire(big_spine.Shape()), big_profile.Shape(), trihedron);
    maker.Build();
    if (!maker.IsDone()) throw std::runtime_error("sweep failed");
    BRepBuilderAPI_Transform back(maker.Shape(), up.Inverted(), Standard_True);
    op = std::make_unique<Operation>();
    op->history = new BRepTools_History(one(profile.shape), big_profile);
    op->history->Merge(new BRepTools_History(one(big_profile.Shape()), maker));
    op->history->Merge(new BRepTools_History(one(maker.Shape()), back));
    op->result = back.Shape();
    if (profile.shape.ShapeType() == TopAbs_FACE) {
      op->first.push_back(back.ModifiedShape(maker.FirstShape()));
      op->last.push_back(back.ModifiedShape(maker.LastShape()));
    }
  } else {
    BRepOffsetAPI_MakePipe maker(spine, profile.shape);
    maker.Build();
    op = finish(one(profile.shape), maker, "sweep");
    if (profile.shape.ShapeType() == TopAbs_FACE) {
      op->first.push_back(maker.FirstShape());
      op->last.push_back(maker.LastShape());
    }
  }
  // Faces swept along consecutive path edges onto one surface (the top of a rectangle swept along a line, an arc
  // and a line) are one face in Onshape.
  ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_True, Standard_False);
  unifier.Build();
  op->history->Merge(unifier.History());
  op->result = unifier.Shape();
  for (auto &cap : op->first) {
    const TopTools_ListOfShape &images = unifier.History()->Modified(cap);
    if (!images.IsEmpty()) cap = images.First();
  }
  for (auto &cap : op->last) {
    const TopTools_ListOfShape &images = unifier.History()->Modified(cap);
    if (!images.IsEmpty()) cap = images.First();
  }
  return op;
}

namespace {

// Faces on B-spline surfaces that are planes to within 1e-9 m, rebuilt on planes, as Parasolid makes them (a loft
// between parallel squares has planar sides).
TopoDS_Shape planes_recognized(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history) {
  Handle(BRepTools_ReShape) reshape = new BRepTools_ReShape();
  // Recorded apart and merged, so the history leads from the operation's inputs to the rebuilt faces.
  Handle(BRepTools_History) rebuilt_history = new BRepTools_History;
  bool changed = false;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    const TopoDS_Face &face = TopoDS::Face(it.Current());
    GeomAbs_SurfaceType type = BRepAdaptor_Surface(face, Standard_False).GetType();
    if (type != GeomAbs_BSplineSurface && type != GeomAbs_BezierSurface) continue;
    ShapeAnalysis_CanonicalRecognition recognition(face);
    gp_Pln plane;
    if (!recognition.IsPlane(1e-9, plane)) continue;
    // Keep the face's normal.
    gp_Dir before = face_normal(face);
    BRepBuilderAPI_MakeFace maker(plane, BRepTools::OuterWire(face), Standard_True);
    for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next())
      if (!w.Current().IsSame(BRepTools::OuterWire(face))) maker.Add(TopoDS::Wire(w.Current()));
    if (!maker.IsDone()) continue;
    TopoDS_Face rebuilt = maker.Face();
    if (face_normal(rebuilt).Dot(before) < 0) rebuilt.Reverse();
    reshape->Replace(face, rebuilt);
    rebuilt_history->AddModified(face, rebuilt);
    changed = true;
  }
  if (!changed) return shape;
  history->Merge(rebuilt_history);
  return reshape->Apply(shape);
}

// A general transform's result with its edges' curves on faces projected again from their 3D curves, and its
// tolerances measured again. The transform is exact on the 3D curves and surfaces (it moves the poles of their
// NURBS forms), but the curves OCCT fits on a cylinder's NURBS form leave edges up to 1e-4 m tolerant: a closed
// hem's arc sheet, scaled about its axis, then swallows the 0.014 mm sliver its corner adds (batch 021 c012).
// Where a curve lands a period away and a face's area changes outright, the shape is kept as it was.
TopoDS_Shape pcurves_reprojected(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history) {
  bool tolerant = false;
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More() && !tolerant; e.Next())
    tolerant = BRep_Tool::Tolerance(TopoDS::Edge(e.Current())) > 1e-6;
  if (!tolerant) return shape;
  TopTools_ListOfShape arguments;
  arguments.Append(shape);
  BRepBuilderAPI_Copy copier(shape, Standard_True);
  TopoDS_Shape copy = copier.Shape();
  ShapeFix_Edge fix;
  for (TopExp_Explorer f(copy, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Edge &edge = TopoDS::Edge(e.Current());
      if (BRep_Tool::Tolerance(edge) <= 1e-6 || BRep_Tool::IsClosed(edge, face) || BRep_Tool::Degenerated(edge)) continue;
      BRep_Builder().UpdateEdge(edge, Handle(Geom2d_Curve)(), face, BRep_Tool::Tolerance(edge));
      fix.FixAddPCurve(edge, face, Standard_False, 1e-9);
    }
  }
  TopExp_Explorer before(shape, TopAbs_FACE), after(copy, TopAbs_FACE);
  for (; before.More() && after.More(); before.Next(), after.Next()) {
    GProp_GProps a, b;
    BRepGProp::SurfaceProperties(before.Current(), a);
    BRepGProp::SurfaceProperties(after.Current(), b);
    if (std::abs(b.Mass() - a.Mass()) > 1e-3 * std::abs(a.Mass()) + 1e-15) return shape;
  }
  ShapeFix_ShapeTolerance().LimitTolerance(copy, 1e-7, 1e-7);
  BRepLib::SameParameter(copy, 1e-7, Standard_True);
  BRepLib::UpdateTolerances(copy, Standard_False);
  if (!BRepCheck_Analyzer(copy).IsValid()) return shape;
  history->Merge(new BRepTools_History(arguments, copier));
  return copy;
}

}  // namespace

// Lofts through wires in order: a solid through closed wires if `solid`, else a sheet.
// A loft's sections: wires, or a vertex at either end (a loft to a point).
static void add_sections(BRepOffsetAPI_ThruSections &maker, const ShapeList &sections) {
  for (const TopoDS_Shape &w : sections.shapes) {
    if (w.ShapeType() == TopAbs_VERTEX)
      maker.AddVertex(TopoDS::Vertex(w));
    else
      maker.AddWire(TopoDS::Wire(w));
  }
}

std::unique_ptr<Operation> loft_op(const ShapeList &wires, bool solid) { return loft_op_matching(wires, solid, true); }

// A loft, its sections matched by OpenCascade (`check`) or as given: each wire's start and direction already line up.
std::unique_ptr<Operation> loft_op_matching(const ShapeList &wires, bool solid, bool check) {
  BRepOffsetAPI_ThruSections maker(solid ? Standard_True : Standard_False, Standard_False, 1e-7);
  maker.CheckCompatibility(check ? Standard_True : Standard_False);
  add_sections(maker, wires);
  maker.Build();
  auto op = finish(to_list(wires), maker, "loft");
  op->result = planes_recognized(op->result, op->history);
  TopoDS_Shape first = maker.FirstShape(), last = maker.LastShape();
  if (!first.IsNull()) op->first.push_back(first);
  if (!last.IsNull()) op->last.push_back(last);
  // A sheet's caps are its edges along its first and last profiles (std trims a thin loft's ends to their planes
  // by them, batch 076 c008).
  if (!solid && !wires.shapes.empty()) {
    auto along = [&](const TopoDS_Shape &profile) {
      TopoDS_Compound found;
      BRep_Builder builder;
      builder.MakeCompound(found);
      bool any = false;
      for (TopExp_Explorer e(op->result, TopAbs_EDGE); e.More(); e.Next()) {
        BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
        TopoDS_Vertex mid = BRepBuilderAPI_MakeVertex(curve.Value(0.5 * (curve.FirstParameter() + curve.LastParameter())));
        BRepExtrema_DistShapeShape d(mid, profile);
        if (d.IsDone() && d.Value() < 1e-7) {
          builder.Add(found, e.Current());
          any = true;
        }
      }
      return any ? TopoDS_Shape(found) : TopoDS_Shape();
    };
    if (first.IsNull()) {
      TopoDS_Shape f = along(wires.shapes.front());
      if (!f.IsNull()) op->first.push_back(f);
    }
    if (last.IsNull()) {
      TopoDS_Shape l = along(wires.shapes.back());
      if (!l.IsNull()) op->last.push_back(l);
    }
  }
  return op;
}

namespace {

// A wire's edges in order, each with how far along the wire it starts: to place points on it by arc length.
struct WireRun {
  struct Piece {
    BRepAdaptor_Curve curve;
    bool reversed;
    double start, length;
  };
  std::vector<Piece> pieces;
  double total = 0;

  explicit WireRun(const TopoDS_Wire &wire) {
    for (BRepTools_WireExplorer e(wire); e.More(); e.Next()) {
      BRepAdaptor_Curve curve(e.Current());
      const double length = GCPnts_AbscissaPoint::Length(curve);
      pieces.push_back({curve, e.Current().Orientation() == TopAbs_REVERSED, total, length});
      total += length;
    }
  }
  // The point a fraction of the way along.
  gp_Pnt at(double fraction) const {
    double d = fraction * total;
    for (const Piece &p : pieces) {
      if (d <= p.start + p.length + 1e-12 || &p == &pieces.back()) {
        const double along = d - p.start;
        const double from = p.reversed ? p.curve.LastParameter() : p.curve.FirstParameter();
        GCPnts_AbscissaPoint point(p.curve, p.reversed ? -along : along, from);
        return p.curve.Value(point.Parameter());
      }
    }
    return pieces.back().curve.Value(pieces.back().curve.LastParameter());
  }
  // How far along a point on the wire is (a fraction), and the wire's unit tangent there, going along it.
  std::tuple<double, gp_Vec, size_t, double> place(const gp_Pnt &q) const {
    double best = 1e300, fraction = 0, within = 0;
    size_t index = 0;
    gp_Vec tangent;
    for (const Piece &p : pieces) {
      Extrema_ExtPC near(q, p.curve);
      if (!near.IsDone()) continue;
      for (int i = 1; i <= near.NbExt(); ++i) {
        if (near.SquareDistance(i) >= best) continue;
        best = near.SquareDistance(i);
        const double t = near.Point(i).Parameter();
        const double along = p.reversed ? GCPnts_AbscissaPoint::Length(p.curve, t, p.curve.LastParameter())
                                        : GCPnts_AbscissaPoint::Length(p.curve, p.curve.FirstParameter(), t);
        fraction = (p.start + along) / total;
        index = &p - pieces.data();
        within = along / p.length;
        gp_Pnt point;
        gp_Vec d1;
        p.curve.D1(t, point, d1);
        tangent = (p.reversed ? -d1 : d1).Normalized();
      }
      // The ends: Extrema can miss a point at a curve's end.
      for (double t : {p.curve.FirstParameter(), p.curve.LastParameter()}) {
        const double d2 = q.SquareDistance(p.curve.Value(t));
        if (d2 >= best) continue;
        best = d2;
        const bool at_start = (t == p.curve.FirstParameter()) != p.reversed;
        fraction = (p.start + (at_start ? 0 : p.length)) / total;
        index = &p - pieces.data();
        within = at_start ? 0 : 1;
        gp_Pnt point;
        gp_Vec d1;
        p.curve.D1(t, point, d1);
        tangent = (p.reversed ? -d1 : d1).Normalized();
      }
    }
    if (fraction >= 1 - 1e-12) fraction -= 1;
    return {fraction, tangent, index, within};
  }
};

// The periodic cubic spline through values at 0, 1/4, 1/2, 3/4 (period 1): its value and derivative at s.
struct QuarterSpline {
  gp_Vec y[4], m[4];  // m: derivatives per quarter
  explicit QuarterSpline(const gp_Vec values[4]) {
    // m[k-1] + 4 m[k] + m[k+1] = 3 (y[k+1] - y[k-1]), round the loop: the inverse of that circulant matrix.
    for (int k = 0; k < 4; ++k) y[k] = values[k];
    const double inv[4] = {7.0 / 24, -1.0 / 12, 1.0 / 24, -1.0 / 12};
    gp_Vec rhs[4];
    for (int k = 0; k < 4; ++k) rhs[k] = (y[(k + 1) % 4] - y[(k + 3) % 4]) * 3;
    for (int k = 0; k < 4; ++k) {
      m[k] = gp_Vec();
      for (int j = 0; j < 4; ++j) m[k] += rhs[j] * inv[(j - k + 4) % 4];
    }
  }
  std::pair<gp_Vec, gp_Vec> at(double s) const {
    s -= std::floor(s);
    int k = std::min(3, static_cast<int>(s * 4));
    const double t = s * 4 - k;
    const gp_Vec &a = y[k], &b = y[(k + 1) % 4], &ma = m[k], &mb = m[(k + 1) % 4];
    const double h00 = 2 * t * t * t - 3 * t * t + 1, h10 = t * t * t - 2 * t * t + t, h01 = -2 * t * t * t + 3 * t * t,
                 h11 = t * t * t - t * t;
    const double d00 = 6 * t * t - 6 * t, d10 = 3 * t * t - 4 * t + 1, d01 = -6 * t * t + 6 * t, d11 = 3 * t * t - 2 * t;
    return {a * h00 + ma * h10 + b * h01 + mb * h11, (a * d00 + ma * d10 + b * d01 + mb * d11) * 4};
  }
};

}  // namespace

// A loft through profiles as Onshape builds one with conditions at its ends or through more than two profiles:
// across the profiles, each side is the natural cubic spline through them (no second derivative at a free end) at
// parameters spaced as the chords between them (batch 004 c038, 057 c003). A condition at an end sets the
// derivative there instead (batches 055, 056): a chord from the first profile to the next gives it, along a vector
// v (the profile's normal, or a direction) the magnitude times 0.9 of the chord along v plus 0.1 of its length;
// square to v (tangent to the profile's plane), the magnitude times the chord's part there. On a profile of several
// edges the derivative goes from its value at one vertex to the next as 3t² − 2t³; on a profile of one closed edge
// it is the chord's at each point along v, and square to v the periodic cubic spline through its values a quarter
// of the way round each time. OCCT's loft matches the profiles' edges; along them the sides' rows are cubic Hermite
// fits of 64 spans (as Onshape's profiles are).
std::unique_ptr<Operation> loft_clamped_op(const ShapeList &wires, bool solid, Vec3 start_dir, double start_mag,
                                           bool start_tangent, const ShapeList &start_faces, bool start_curvature,
                                           Vec3 end_dir, double end_mag, bool end_tangent, const ShapeList &end_faces,
                                           bool end_curvature) {
  const size_t n = wires.shapes.size();
  if (n < 2) throw std::runtime_error("loft: two profiles at least");
  if ((start_curvature || end_curvature) && n != 2)
    throw std::runtime_error("loft: matching curvature through more than two profiles isn't supported yet");
  const bool has_start = !std::isnan(start_mag), has_end = !std::isnan(end_mag);
  if ((has_start && wires.shapes[0].ShapeType() == TopAbs_VERTEX) ||
      (has_end && wires.shapes[n - 1].ShapeType() == TopAbs_VERTEX))
    throw std::runtime_error("loft: end conditions at a point aren't supported yet");
  BRepOffsetAPI_ThruSections maker(solid ? Standard_True : Standard_False, Standard_False, 1e-7);
  maker.CheckCompatibility(Standard_True);
  add_sections(maker, wires);
  maker.Build();
  if (!maker.IsDone()) throw std::runtime_error("loft failed");
  const TopoDS_Shape lofted = maker.Shape();
  TopTools_IndexedMapOfShape caps;
  if (!maker.FirstShape().IsNull()) caps.Add(maker.FirstShape());
  if (!maker.LastShape().IsNull()) caps.Add(maker.LastShape());
  // A profile the conditions are measured along: the first, or for a condition at the end only, the last.
  auto centre = [](const TopoDS_Shape &w) {
    if (w.ShapeType() == TopAbs_VERTEX) return BRep_Tool::Pnt(TopoDS::Vertex(w));
    GProp_GProps props;
    BRepGProp::LinearProperties(w, props);
    return props.CentreOfMass();
  };
  std::vector<std::unique_ptr<BRepExtrema_DistShapeShape>> near(n);
  for (size_t k = 0; k < n; ++k) {
    near[k] = std::make_unique<BRepExtrema_DistShapeShape>();
    near[k]->LoadS1(wires.shapes[k]);
  }
  auto distance_to = [&](size_t k, const gp_Pnt &q) {
    near[k]->LoadS2(BRepBuilderAPI_MakeVertex(q).Vertex());
    near[k]->Perform();
    return near[k]->IsDone() ? near[k]->Value() : 1e300;
  };
  struct Side {
    TopoDS_Face face;
    Handle(Geom_Surface) surface;
    double u1, u2;
    std::vector<double> v;  // where each profile lies across the side
  };
  std::vector<Side> sides;
  std::vector<TopoDS_Shape> kept;
  for (TopExp_Explorer it(lofted, TopAbs_FACE); it.More(); it.Next()) {
    const TopoDS_Face face = TopoDS::Face(it.Current());
    if (caps.Contains(face)) {
      kept.push_back(face);
      continue;
    }
    TopLoc_Location location;
    Handle(Geom_Surface) surface = Handle(Geom_Surface)::DownCast(BRep_Tool::Surface(face, location)->Copy());
    if (!location.IsIdentity()) surface->Transform(location.Transformation());
    double u1, u2, v1, v2;
    BRepTools::UVBounds(face, u1, u2, v1, v2);
    // A side OCCT makes flat from an edge to a point: the surface ruled from the edge (v = 0) to the point (v = 1).
    const bool to_point = wires.shapes[0].ShapeType() == TopAbs_VERTEX || wires.shapes[n - 1].ShapeType() == TopAbs_VERTEX;
    if (to_point && n == 2 && Handle(Geom_BSplineSurface)::DownCast(surface).IsNull()) {
      const size_t at_point = wires.shapes[0].ShapeType() == TopAbs_VERTEX ? 0 : 1;
      const gp_Pnt apex = BRep_Tool::Pnt(TopoDS::Vertex(wires.shapes[at_point]));
      Handle(Geom_BSplineCurve) along;
      for (TopExp_Explorer e(face, TopAbs_EDGE); e.More() && along.IsNull(); e.Next()) {
        BRepAdaptor_Curve curve(TopoDS::Edge(e.Current()));
        if (curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2).Distance(apex) < 1e-9) continue;
        if (distance_to(1 - at_point, curve.Value((curve.FirstParameter() + curve.LastParameter()) / 2)) > 1e-6) continue;
        double f, l;
        Handle(Geom_Curve) c = BRep_Tool::Curve(TopoDS::Edge(e.Current()), f, l);
        along = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(c, f, l));
      }
      if (along.IsNull()) throw std::runtime_error("loft: a side to a point has no edge on the profile");
      TColgp_Array2OfPnt rows(1, along->NbPoles(), 1, 2);
      TColStd_Array2OfReal weights(1, along->NbPoles(), 1, 2);
      for (int i = 1; i <= along->NbPoles(); ++i) {
        rows(i, 1 + static_cast<int>(at_point == 0)) = along->Pole(i);
        rows(i, 2 - static_cast<int>(at_point == 0)) = apex;
        weights(i, 1) = weights(i, 2) = along->Weight(i);
      }
      TColStd_Array1OfReal uk(1, along->NbKnots()), vk(1, 2);
      TColStd_Array1OfInteger um(1, along->NbKnots()), vm(1, 2);
      along->Knots(uk);
      along->Multiplicities(um);
      vk(1) = 0, vk(2) = 1, vm(1) = vm(2) = 2;
      surface = new Geom_BSplineSurface(rows, weights, uk, vk, um, vm, along->Degree(), 1);
      surface->Bounds(u1, u2, v1, v2);
    }
    // Each profile's v: the ends are the first and last; between, where the side's middle line meets it.
    const double um = (u1 + u2) / 2;
    const bool forward = distance_to(0, surface->Value(um, v1)) < 1e-6;
    if (!forward && distance_to(0, surface->Value(um, v2)) >= 1e-6)
      throw std::runtime_error("loft: a side doesn't run across the profiles");
    std::vector<double> v(n);
    v[0] = forward ? v1 : v2;
    v[n - 1] = forward ? v2 : v1;
    for (size_t k = 1; k + 1 < n; ++k) {
      const int samples = 400;
      double best = 1e300, at = v1;
      for (int i = 0; i <= samples; ++i) {
        const double w = v1 + (v2 - v1) * i / samples, d = distance_to(k, surface->Value(um, w));
        if (d < best) best = d, at = w;
      }
      double a = std::max(v1, at - (v2 - v1) / samples), b = std::min(v2, at + (v2 - v1) / samples);
      for (int i = 0; i < 60; ++i) {
        const double m1 = a + (b - a) / 3, m2 = b - (b - a) / 3;
        if (distance_to(k, surface->Value(um, m1)) < distance_to(k, surface->Value(um, m2))) b = m2; else a = m1;
      }
      v[k] = (a + b) / 2;
    }
    sides.push_back({face, surface, u1, u2, v});
  }
  if (sides.empty()) throw std::runtime_error("loft: no sides");
  // Parameters across: spaced as the chords between the profiles, averaged over the sides.
  std::vector<double> t(n, 0.0);
  {
    std::vector<double> chord(n - 1, 0.0);
    int count = 0;
    for (const Side &side : sides) {
      for (int i = 0; i <= 16; ++i) {
        const double u = side.u1 + (side.u2 - side.u1) * i / 16;
        for (size_t k = 0; k + 1 < n; ++k) chord[k] += side.surface->Value(u, side.v[k]).Distance(side.surface->Value(u, side.v[k + 1]));
        ++count;
      }
    }
    double total = 0;
    for (size_t k = 0; k + 1 < n; ++k) total += chord[k];
    for (size_t k = 0; k + 1 < n; ++k) t[k + 1] = t[k] + (total > 0 ? chord[k] / total : 1.0 / (n - 1));
    t[n - 1] = 1;
  }
  // The conditions, measured from the profile at that end towards the next.
  struct End {
    gp_Vec v;
    double mag;
    bool tangent, active;
    size_t profile, next;
    std::vector<TopoDS_Face> faces;  // matching these faces' tangency
  } ends[2] = {{gp_Vec(dir(start_dir)), has_start ? start_mag : 0, start_tangent, has_start, 0, 1, {}},
               {gp_Vec(dir(end_dir)), has_end ? end_mag : 0, end_tangent, has_end, n - 1, n - 2, {}}};
  for (const TopoDS_Shape &f : start_faces.shapes) ends[0].faces.push_back(TopoDS::Face(f));
  for (const TopoDS_Shape &f : end_faces.shapes) ends[1].faces.push_back(TopoDS::Face(f));
  const gp_Vec travel(centre(wires.shapes[0]), centre(wires.shapes[n - 1]));
  for (End &end : ends) {
    end.v.Normalize();
    // Along the travel from the first profile to the last.
    if (travel.Dot(end.v) < 0) end.v.Reverse();
  }
  // Polygons with their tangency set at an end, tangent to its plane (batch 056 c003; 060 c006–c008; 061; 062
  // c008): each side is Onshape's bicubic exactly. Along the edge it has knots at thirds, the edge's points at the
  // Greville abscissae; across, the cubic spline through the profiles, knots spaced as the chords between the
  // edges' middles, natural at a free end. At a tangent end its derivative across is, along the edge, the natural
  // cubic spline through its values at the knots: there the start (or end) tangent of the natural spline through
  // the profiles projected into the end's plane, times the magnitude, as long, in the corners' directions blended
  // by h = 3u² − 2u³. Between two profiles that tangent is the chord.
  bool exact = !start_curvature && !end_curvature && start_faces.shapes.empty() && end_faces.shapes.empty() &&
               (has_start || has_end) && (!has_start || start_tangent) && (!has_end || end_tangent);
  std::vector<TopoDS_Face> exact_faces;
  if (exact) {
    const double greville[6] = {0, 1.0 / 9, 1.0 / 3, 2.0 / 3, 8.0 / 9, 1};
    // Each side's corners on each profile, its u running round the first profile anticlockwise about the travel.
    struct Corners {
      std::vector<gp_Pnt> a, b;
    };
    std::vector<Corners> corners;
    const gp_Pnt c0 = centre(wires.shapes[0]);
    for (const Side &side : sides) {
      Corners c;
      const double um = (side.u1 + side.u2) / 2;
      for (size_t k = 0; k < n && exact; ++k) {
        const gp_Pnt a = side.surface->Value(side.u1, side.v[k]), b = side.surface->Value(side.u2, side.v[k]);
        exact = a.Distance(b) > 1e-9 && side.surface->Value(um, side.v[k]).Distance(gp_Pnt((a.XYZ() + b.XYZ()) / 2)) < 1e-9;
        c.a.push_back(a);
        c.b.push_back(b);
      }
      if (!exact) break;
      if (gp_Vec(c.a[0], c.b[0]).Crossed(gp_Vec(c.a[0], c.a[1])).Dot(gp_Vec(c0, gp_Pnt((c.a[0].XYZ() + c.b[0].XYZ()) / 2))) < 0)
        std::swap(c.a, c.b);
      corners.push_back(c);
    }
    // Across: knots spaced as the chords between the edges' middles, over all the sides.
    std::vector<double> tk(n, 0.0);
    if (exact) {
      std::vector<double> chord(n - 1, 0.0);
      for (const Corners &c : corners)
        for (size_t k = 0; k + 1 < n; ++k)
          chord[k] += gp_Pnt((c.a[k].XYZ() + c.b[k].XYZ()) / 2).Distance(gp_Pnt((c.a[k + 1].XYZ() + c.b[k + 1].XYZ()) / 2));
      double total = 0;
      for (double x : chord) total += x;
      for (size_t k = 0; k + 1 < n; ++k) tk[k + 1] = tk[k] + chord[k] / total;
      tk[n - 1] = 1;
    }
    const int mv = static_cast<int>(n) + 2;
    std::vector<double> flat_v;
    for (size_t k = 0; k < n; ++k) for (int r = 0; r < (k == 0 || k == n - 1 ? 4 : 1); ++r) flat_v.push_back(tk[k]);
    auto basis_in = [](const std::vector<double> &flat, int count, double x, int order) {
      std::vector<double> row(count, 0.0);
      math_Matrix ders(1, order + 1, 1, 4);
      int first = 0;
      BSplCLib::EvalBsplineBasis(order, 4, TColStd_Array1OfReal(flat[0], 1, static_cast<int>(flat.size())), x, first, ders);
      for (int j = 0; j < 4; ++j)
        if (first + j - 1 >= 0 && first + j - 1 < count) row[first + j - 1] = ders(order + 1, j + 1);
      return row;
    };
    // Systems across: through the profiles, natural at both ends (for the tangents the conditions are measured from),
    // and with the loft's own conditions.
    auto system_with = [&](bool clamp_start, bool clamp_end) {
      math_Matrix m(1, mv, 1, mv, 0.0);
      int row = 1;
      auto put = [&](const std::vector<double> &r) {
        for (int j = 0; j < mv; ++j) m(row, j + 1) = r[j];
        ++row;
      };
      for (size_t k = 0; k < n; ++k) put(basis_in(flat_v, mv, tk[k], 0));
      put(basis_in(flat_v, mv, tk[0], clamp_start ? 1 : 2));
      put(basis_in(flat_v, mv, tk[n - 1], clamp_end ? 1 : 2));
      return m;
    };
    math_Gauss natural_across(system_with(false, false)), clamped_across(system_with(has_start, has_end));
    const std::vector<double> flat_u = {0, 0, 0, 0, 1.0 / 3, 2.0 / 3, 1, 1, 1, 1};
    math_Matrix natural(1, 6, 1, 6, 0.0);
    {
      const double at[4] = {0, 1.0 / 3, 2.0 / 3, 1};
      for (int k = 0; k < 4; ++k) {
        const auto r = basis_in(flat_u, 6, at[k], 0);
        for (int j = 0; j < 6; ++j) natural(k + 1, j + 1) = r[j];
      }
      const auto r0 = basis_in(flat_u, 6, 0, 2), r1 = basis_in(flat_u, 6, 1, 2);
      for (int j = 0; j < 6; ++j) natural(5, j + 1) = r0[j], natural(6, j + 1) = r1[j];
    }
    math_Gauss natural_along(natural);
    exact = exact && natural_along.IsDone() && natural_across.IsDone() && clamped_across.IsDone();
    for (size_t si = 0; exact && si < corners.size(); ++si) {
      const Corners &c = corners[si];
      auto at = [&](size_t k, double u) { return gp_Pnt(c.a[k].XYZ() + (c.b[k].XYZ() - c.a[k].XYZ()) * u); };
      // The tangent at an end of the natural spline through the profiles' points at u.
      auto natural_tangent = [&](double u, bool at_start) {
        gp_Vec d;
        for (int coord = 1; coord <= 3; ++coord) {
          math_Vector rhs(1, mv, 0.0), x(1, mv);
          for (size_t k = 0; k < n; ++k) rhs(static_cast<int>(k) + 1) = at(k, u).Coord(coord);
          natural_across.Solve(rhs, x);
          const auto r = basis_in(flat_v, mv, at_start ? 0.0 : 1.0, 1);
          double value = 0;
          for (int j = 0; j < mv; ++j) value += r[j] * x(j + 1);
          d.SetCoord(coord, value);
        }
        return d;
      };
      // An end's derivative poles along u.
      auto derivative_poles = [&](const End &end, bool at_start) {
        auto projected = [&](double u) {
          const gp_Vec d = natural_tangent(u, at_start);
          return (d - end.v * d.Dot(end.v)) * end.mag;
        };
        // The corners' directions (batch 061 c010: not their derivatives, which differ in length off the axis).
        const gp_Vec a = projected(0).Normalized(), b = projected(1).Normalized();
        std::vector<gp_Vec> values;
        for (double u : {0.0, 1.0 / 3, 2.0 / 3, 1.0}) {
          const double h = 3 * u * u - 2 * u * u * u;
          gp_Vec d = a * (1 - h) + b * h;
          values.push_back(d.Magnitude() > 1e-12 ? d.Normalized() * projected(u).Magnitude() : d);
        }
        std::vector<gp_Vec> poles(6);
        for (int coord = 1; coord <= 3; ++coord) {
          math_Vector rhs(1, 6, 0.0), x(1, 6);
          for (int k = 0; k < 4; ++k) rhs(k + 1) = values[k].Coord(coord);
          natural_along.Solve(rhs, x);
          for (int j = 0; j < 6; ++j) poles[j].SetCoord(coord, x(j + 1));
        }
        return poles;
      };
      std::vector<gp_Vec> ds(6), de(6);
      if (has_start) ds = derivative_poles(ends[0], true);
      if (has_end) de = derivative_poles(ends[1], false);
      TColgp_Array2OfPnt poles(1, 6, 1, mv);
      for (int i = 0; i < 6; ++i) {
        for (int coord = 1; coord <= 3; ++coord) {
          math_Vector rhs(1, mv, 0.0), x(1, mv);
          for (size_t k = 0; k < n; ++k) rhs(static_cast<int>(k) + 1) = at(k, greville[i]).Coord(coord);
          rhs(static_cast<int>(n) + 1) = ds[i].Coord(coord);
          rhs(static_cast<int>(n) + 2) = de[i].Coord(coord);
          clamped_across.Solve(rhs, x);
          for (int j = 0; j < mv; ++j) {
            gp_XYZ p = poles(i + 1, j + 1).XYZ();
            p.SetCoord(coord, x(j + 1));
            poles(i + 1, j + 1) = gp_Pnt(p);
          }
        }
      }
      TColStd_Array1OfReal uk(1, 4), vk(1, static_cast<int>(n));
      TColStd_Array1OfInteger umult(1, 4), vmult(1, static_cast<int>(n));
      uk(1) = 0, uk(2) = 1.0 / 3, uk(3) = 2.0 / 3, uk(4) = 1;
      umult(1) = umult(4) = 4, umult(2) = umult(3) = 1;
      for (size_t k = 0; k < n; ++k) {
        vk(static_cast<int>(k) + 1) = tk[k];
        vmult(static_cast<int>(k) + 1) = (k == 0 || k == n - 1) ? 4 : 1;
      }
      Handle(Geom_BSplineSurface) surface = new Geom_BSplineSurface(poles, uk, vk, umult, vmult, 3, 3);
      exact_faces.push_back(BRepBuilderAPI_MakeFace(surface, 0, 1, 0, 1, 1e-7).Face());
    }
  }
  // The direction an end's derivative takes at a point of its profile: its vector; matching faces, where the faces
  // next to the profile there meet (a corner of two, batch 058 c006), or across the one face square to the profile.
  auto direction_at = [&](const End &end, const gp_Pnt &q, const gp_Vec &along) {
    if (end.faces.empty()) return end.v;
    std::vector<gp_Vec> normals;
    for (const TopoDS_Face &face : end.faces) {
      BRepExtrema_DistShapeShape near_face(BRepBuilderAPI_MakeVertex(q).Vertex(), face);
      if (!near_face.IsDone() || near_face.Value() > 1e-6) continue;
      Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
      GeomAPI_ProjectPointOnSurf projection(q, surface);
      if (projection.NbPoints() == 0) continue;
      double u, v;
      projection.LowerDistanceParameters(u, v);
      GeomLProp_SLProps props(surface, u, v, 1, 1e-9);
      if (!props.IsNormalDefined()) continue;
      gp_Vec normal(props.Normal());
      if (face.Orientation() == TopAbs_REVERSED) normal.Reverse();
      bool seen = false;
      for (const gp_Vec &m : normals) seen = seen || m.IsParallel(normal, 1e-6);
      if (!seen) normals.push_back(normal);
    }
    gp_Vec t;
    if (normals.size() >= 2) t = normals[0].Crossed(normals[1]);
    else if (normals.size() == 1) t = normals[0].Crossed(along);
    else return end.v;
    if (t.Magnitude() < 1e-12) return end.v;
    t.Normalize();
    if (t.Dot(travel) < 0) t.Reverse();
    return t;
  };
  auto from_chord_along = [&](const End &end, const gp_Vec &c, const gp_Vec &d) {
    return end.tangent ? (c - d * c.Dot(d)) * end.mag : d * (end.mag * (0.9 * c.Dot(d) + 0.1 * c.Magnitude()));
  };
  auto from_chord = [&](const End &end, const gp_Vec &c) { return from_chord_along(end, c, end.v); };
  // The chord, along the travel, from a point of an end's profile to the next profile.
  auto chord_at = [&](const End &end, const gp_Pnt &q) {
    double best = 1e300;
    gp_Vec chord;
    for (const Side &side : sides) {
      const double va = side.v[end.profile], vb = side.v[end.next];
      GeomAPI_ProjectPointOnSurf projection(q, side.surface, side.u1, side.u2, std::min(side.v.front(), side.v.back()),
                                            std::max(side.v.front(), side.v.back()), 1e-9);
      if (!projection.IsDone() || projection.NbPoints() == 0) continue;
      double u, v;
      projection.LowerDistanceParameters(u, v);
      if (projection.LowerDistance() >= best) continue;
      best = projection.LowerDistance();
      chord = gp_Vec(side.surface->Value(u, va), side.surface->Value(u, vb));
      if (end.next < end.profile) chord.Reverse();
    }
    return chord;
  };
  struct Field {
    std::unique_ptr<WireRun> run;
    bool closed_curve = false;
    std::unique_ptr<QuarterSpline> quarters;
    std::vector<gp_Vec> at_vertices;
  } fields[2];
  for (int which = 0; which < 2; ++which) {
    const End &end = ends[which];
    if (!end.active) continue;
    Field &f = fields[which];
    f.run = std::make_unique<WireRun>(TopoDS::Wire(wires.shapes[end.profile]));
    f.closed_curve = f.run->pieces.size() == 1;
    gp_Vec values[4];
    for (int k = 0; k < 4; ++k) values[k] = from_chord(end, chord_at(end, f.run->at(k / 4.0)));
    f.quarters = std::make_unique<QuarterSpline>(values);
    for (const WireRun::Piece &piece : f.run->pieces) {
      const gp_Pnt q = f.run->at(piece.start / f.run->total);
      gp_Pnt at;
      gp_Vec d1;
      piece.curve.D1(piece.reversed ? piece.curve.LastParameter() : piece.curve.FirstParameter(), at, d1);
      f.at_vertices.push_back(from_chord_along(end, chord_at(end, q), direction_at(end, q, piece.reversed ? -d1 : d1)));
    }
  }
  // An end's derivative at u on a side, and how it changes with u.
  auto derivative = [&](int which, const Side &side, double u) -> std::pair<gp_Vec, gp_Vec> {
    const End &end = ends[which];
    const Field &f = fields[which];
    if (!end.active) return {gp_Vec(), gp_Vec()};
    const double va = side.v[end.profile], vb = side.v[end.next];
    gp_Pnt q;
    gp_Vec qu, qv;
    side.surface->D1(u, va, q, qu, qv);
    const auto [s, tangent, piece, within] = f.run->place(q);
    const double ds = qu.Dot(tangent) / f.run->total;
    if (f.closed_curve && end.tangent) {
      const auto [d, dd] = f.quarters->at(s);
      return {d, dd * ds};
    }
    if (f.closed_curve) {
      auto chord = [&](double w) {
        gp_Vec c(side.surface->Value(w, va), side.surface->Value(w, vb));
        return end.next < end.profile ? -c : c;
      };
      const double h = (side.u2 - side.u1) * 1e-6 * (u + (side.u2 - side.u1) * 1e-6 > side.u2 ? -1 : 1);
      auto field_at = [&](double w) {
        gp_Pnt p;
        gp_Vec pu, pv;
        side.surface->D1(w, va, p, pu, pv);
        return from_chord_along(end, chord(w), direction_at(end, p, pu));
      };
      const gp_Vec here = field_at(u), ahead = field_at(u + h);
      return {here, (ahead - here) / h};
    }
    const gp_Vec &a = f.at_vertices[piece], &b = f.at_vertices[(piece + 1) % f.at_vertices.size()];
    const double len = f.run->pieces[piece].length / f.run->total;
    if (end.faces.empty()) {
      // Normal to the profile (or along a direction): from one corner's value to the next by h = 3w² − 2w³ (batch
      // 063 c005–c007: within 0.0022 of Onshape's fitted sides, the same along every edge).
      const double h = within * within * (3 - 2 * within), dh = 6 * within * (1 - within);
      return {a + (b - a) * h, (b - a) * (dh / len * ds)};
    }
    // From one vertex's value to the next in proportion (batch 058 c006: a matched corner's sideways part falls
    // linearly along the edge).
    return {a + (b - a) * within, (b - a) * (1.0 / len * ds)};
  };
  // Across: the cubic spline on knots t₀⁴ t₁ … tₙ₋₂ tₙ₋₁⁴ through the profiles, its ends free or clamped. Its poles
  // are a fixed combination of the values at the profiles and the ends' conditions.
  const int m = static_cast<int>(n) + 2;
  TColStd_Array1OfReal vknots(1, static_cast<int>(n));
  TColStd_Array1OfInteger vmults(1, static_cast<int>(n));
  for (size_t k = 0; k < n; ++k) {
    vknots(static_cast<int>(k) + 1) = t[k];
    vmults(static_cast<int>(k) + 1) = (k == 0 || k == n - 1) ? 4 : 1;
  }
  std::vector<double> flat;
  for (size_t k = 0; k < n; ++k) for (int r = 0; r < (k == 0 || k == n - 1 ? 4 : 1); ++r) flat.push_back(t[k]);
  // Rows of the system: the basis functions (or their derivatives) at a parameter.
  auto basis_row = [&](double x, int order) {
    std::vector<double> row(m, 0.0);
    math_Matrix ders(1, order + 1, 1, 4);
    int first = 0;
    BSplCLib::EvalBsplineBasis(order, 4, TColStd_Array1OfReal(flat.data()[0], 1, static_cast<int>(flat.size())), x, first, ders);
    for (int j = 0; j < 4; ++j)
      if (first + j - 1 >= 0 && first + j - 1 < m) row[first + j - 1] = ders(order + 1, j + 1);
    return row;
  };
  math_Matrix system(1, m, 1, m, 0.0);
  {
    int row = 1;
    auto put = [&](const std::vector<double> &r) {
      for (int j = 0; j < m; ++j) system(row, j + 1) = r[j];
      ++row;
    };
    for (size_t k = 0; k < n; ++k) put(basis_row(t[k], 0));
    put(basis_row(t[0], has_start ? 1 : 2));
    put(basis_row(t[n - 1], has_end ? 1 : 2));
  }
  math_Gauss solver(system);
  if (!solver.IsDone()) throw std::runtime_error("loft: the spline across the profiles is singular");
  BRepBuilderAPI_Sewing sewing(1e-6);
  // Exact sides go in before the caps, so the edges they share run along the sides' u as Onshape's do, and the
  // faces come sides first (batch 066 c006).
  if (!exact)
    for (const TopoDS_Shape &cap : kept) sewing.Add(cap);
  const int spans = 64;
  for (const TopoDS_Face &face : exact ? exact_faces : std::vector<TopoDS_Face>()) sewing.Add(face);
  if (exact)
    for (const TopoDS_Shape &cap : kept) sewing.Add(cap);
  for (const Side &side : exact ? std::vector<Side>() : sides) {
    // At each span end along u: the poles across, and how they change along u.
    std::vector<std::vector<gp_Pnt>> value(spans + 1, std::vector<gp_Pnt>(m));
    std::vector<std::vector<gp_Vec>> slope(spans + 1, std::vector<gp_Vec>(m));
    for (int j = 0; j <= spans; ++j) {
      const double u = side.u1 + (side.u2 - side.u1) * j / spans;
      std::vector<gp_XYZ> rhs(m), drhs(m);
      for (size_t k = 0; k < n; ++k) {
        gp_Pnt p;
        gp_Vec pu, pv;
        side.surface->D1(u, side.v[k], p, pu, pv);
        rhs[k] = p.XYZ();
        drhs[k] = pu.XYZ();
      }
      const auto [a, da] = derivative(0, side, u);
      const auto [b, db] = derivative(1, side, u);
      rhs[n] = a.XYZ(), drhs[n] = da.XYZ();
      rhs[n + 1] = b.XYZ(), drhs[n + 1] = db.XYZ();
      for (int c = 0; c < 3; ++c) {
        math_Vector x(1, m), dx(1, m), bx(1, m), dbx(1, m);
        for (int i = 0; i < m; ++i) bx(i + 1) = rhs[i].Coord(c + 1), dbx(i + 1) = drhs[i].Coord(c + 1);
        solver.Solve(bx, x);
        solver.Solve(dbx, dx);
        for (int i = 0; i < m; ++i) {
          gp_XYZ pv = value[j][i].XYZ(), sv = slope[j][i].XYZ();
          pv.SetCoord(c + 1, x(i + 1));
          sv.SetCoord(c + 1, dx(i + 1));
          value[j][i] = gp_Pnt(pv);
          slope[j][i] = gp_Vec(sv);
        }
      }
    }
    TColgp_Array2OfPnt poles(1, 3 * spans + 1, 1, m);
    const double step = (side.u2 - side.u1) / spans;
    for (int row = 0; row < m; ++row) {
      for (int j = 0; j < spans; ++j) {
        poles(3 * j + 1, row + 1) = value[j][row];
        poles(3 * j + 2, row + 1) = value[j][row].Translated(slope[j][row] * (step / 3));
        poles(3 * j + 3, row + 1) = value[j + 1][row].Translated(-slope[j + 1][row] * (step / 3));
      }
      poles(3 * spans + 1, row + 1) = value[spans][row];
    }
    TColStd_Array1OfReal uknots(1, spans + 1);
    TColStd_Array1OfInteger umults(1, spans + 1);
    for (int j = 0; j <= spans; ++j) {
      uknots(j + 1) = side.u1 + step * j;
      umults(j + 1) = (j == 0 || j == spans) ? 4 : 3;
    }
    Handle(Geom_BSplineSurface) s = new Geom_BSplineSurface(poles, uknots, vknots, umults, vmults, 3, 3);
    // Matching the curvature of the faces next to a profile (batch 057 c007): the sides are the tangency-matched
    // cubics raised to quintics across, with the second row from that end moved so the curve's curvature there is
    // the face's along it: P2 = 2 P1 − P0 + (κ |r'|² / 20) n, r' = 5 (P1 − P0), κ the face's normal curvature
    // along r' (none for a plane).
    if (start_curvature || end_curvature) {
      s->IncreaseDegree(3, 5);
      for (int which = 0; which < 2; ++which) {
        if (!(which == 0 ? start_curvature : end_curvature)) continue;
        const End &end = ends[which];
        const int nv = s->NbVPoles();
        const int r0 = which == 0 ? 1 : nv, r1 = which == 0 ? 2 : nv - 1, r2 = which == 0 ? 3 : nv - 2;
        // The curvature term at each span end along u.
        std::vector<gp_Vec> term(spans + 1);
        for (int j = 0; j <= spans; ++j) {
          const gp_Pnt p0 = s->Pole(3 * j + 1, r0), p1 = s->Pole(3 * j + 1, r1);
          const gp_Vec d = gp_Vec(p0, p1) * 5;
          term[j] = gp_Vec();
          for (const TopoDS_Face &face : end.faces) {
            BRepExtrema_DistShapeShape near_face(BRepBuilderAPI_MakeVertex(p0).Vertex(), face);
            if (!near_face.IsDone() || near_face.Value() > 1e-6) continue;
            Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
            GeomAPI_ProjectPointOnSurf projection(p0, surface);
            if (projection.NbPoints() == 0) continue;
            double fu, fv;
            projection.LowerDistanceParameters(fu, fv);
            GeomLProp_SLProps props(surface, fu, fv, 2, 1e-9);
            if (!props.IsCurvatureDefined() || d.Magnitude() < 1e-12) break;
            const gp_Vec normal(props.Normal());
            const gp_Vec t = (d - normal * d.Dot(normal)).Normalized();
            double kappa = 0;
            if (props.IsUmbilic()) {
              kappa = props.MaxCurvature();
            } else {
              gp_Dir dmax, dmin;
              props.CurvatureDirections(dmax, dmin);
              const double c = t.Dot(gp_Vec(dmax)), sn = t.Dot(gp_Vec(dmin));
              kappa = props.MaxCurvature() * c * c + props.MinCurvature() * sn * sn;
            }
            term[j] = normal * (kappa * d.SquareMagnitude() / 20);
            break;
          }
        }
        for (int j = 0; j <= spans; ++j) {
          const gp_Vec slope = j == 0        ? (term[1] - term[0]) / step
                               : j == spans ? (term[spans] - term[spans - 1]) / step
                                            : (term[j + 1] - term[j - 1]) / (2 * step);
          for (int k : {0, 1, 2}) {
            const int i = 3 * j + 1 + (k == 0 ? 0 : k == 1 ? 1 : -1);
            if (i < 1 || i > s->NbUPoles()) continue;
            const gp_Vec offset = k == 0 ? term[j] : k == 1 ? term[j] + slope * (step / 3) : term[j] - slope * (step / 3);
            const gp_XYZ p = s->Pole(i, r1).XYZ() * 2 - s->Pole(i, r0).XYZ() + offset.XYZ();
            s->SetPole(i, r2, gp_Pnt(p));
          }
        }
      }
    }
    // The spans meet with one slope either side (C1), so their knots need only the degree less one: as Hermite
    // pieces OCCT takes the surface for C0, which it won't offset (a bottle lofted through three circles and
    // shelled, batch 145 c005).
    for (int j = s->NbUKnots() - 1; j >= 2; --j)
      if (s->UMultiplicity(j) > s->UDegree() - 1) s->RemoveUKnot(j, s->UDegree() - 1, 1e-9);
    // Along a straight edge every row is a line, and Onshape's side is a single cubic along it (its poles at thirds,
    // batch 175 c004 and c006): the spans' knots all come out; along a curved edge none do.
    for (int j = s->NbUKnots() - 1; j >= 2; --j) s->RemoveUKnot(j, 0, 1e-9);
    double fu1 = side.u1, fu2 = side.u2;
    // Round a closed edge (a circle) Onshape's side is periodic along it, its seam's knot double as the others are
    // (batch 177 c007).
    bool closed_u = s->NbUKnots() > 2;
    for (int j = 1; j <= s->NbVPoles() && closed_u; ++j)
      closed_u = s->Pole(1, j).Distance(s->Pole(s->NbUPoles(), j)) < 1e-12;
    if (closed_u) {
      try {
        s->SetUPeriodic();
        if (s->UMultiplicity(1) > s->UDegree() - 1) s->RemoveUKnot(1, s->UDegree() - 1, 1e-9);
      } catch (const Standard_Failure &) {
      }
    }
    if (s->NbUKnots() == 2) {
      // That single cubic runs from 0 to 1 along the edge, as Onshape's does.
      TColStd_Array1OfReal unit(1, 2);
      unit(1) = 0, unit(2) = 1;
      s->SetUKnots(unit);
      fu1 = 0, fu2 = 1;
    }
    // A side ending at a point has a degenerate row there: the face's edge there is degenerate.
    ShapeFix_Face fix(BRepBuilderAPI_MakeFace(s, fu1, fu2, 0, 1, 1e-7).Face());
    fix.Perform();
    sewing.Add(fix.Face());
  }
  sewing.Perform();
  TopoDS_Shape result = sewing.SewedShape();
  if (solid) {
    TopoDS_Shell shell;
    for (TopExp_Explorer it(result, TopAbs_SHELL); it.More(); it.Next()) shell = TopoDS::Shell(it.Current());
    if (shell.IsNull() && result.ShapeType() == TopAbs_FACE) {
      // One side closing on itself (between two points).
      BRep_Builder builder;
      builder.MakeShell(shell);
      builder.Add(shell, result);
    }
    if (shell.IsNull()) throw std::runtime_error("loft: the sides and ends don't close");
    BRepBuilderAPI_MakeSolid made(shell);
    ShapeFix_Solid fix(made.Solid());
    fix.Perform();
    result = fix.Solid();
  }
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History;
  op->result = result;
  for (int i = 1; i <= caps.Extent(); ++i) {
    TopoDS_Shape image = sewing.Modified(caps(i));
    (i == 1 && !maker.FirstShape().IsNull() ? op->first : op->last).push_back(image);
  }
  op->result = planes_recognized(op->result, op->history);
  return op;
}

// Lofts end to end, one through each group of sections (`counts` of them in turn, each group starting where the
// last ended), sewn into one body: their sides, with the first group's first end and the last group's last end.
std::unique_ptr<Operation> loft_pieces_op(const ShapeList &wires, rust::Slice<const uint32_t> counts, bool solid) {
  BRepBuilderAPI_Sewing sewing(1e-6);
  TopoDS_Shape first, last;
  size_t at = 0;
  for (size_t g = 0; g < counts.size(); ++g) {
    BRepOffsetAPI_ThruSections maker(solid ? Standard_True : Standard_False, Standard_False, 1e-7);
    maker.CheckCompatibility(Standard_True);
    for (uint32_t i = 0; i < counts[g]; ++i) maker.AddWire(TopoDS::Wire(wires.shapes.at(at + i)));
    at += counts[g] - 1;
    maker.Build();
    if (!maker.IsDone()) throw std::runtime_error("loft failed");
    TopTools_IndexedMapOfShape caps;
    if (!maker.FirstShape().IsNull()) caps.Add(maker.FirstShape());
    if (!maker.LastShape().IsNull()) caps.Add(maker.LastShape());
    for (TopExp_Explorer it(maker.Shape(), TopAbs_FACE); it.More(); it.Next())
      if (!caps.Contains(it.Current())) sewing.Add(it.Current());
    if (g == 0 && !maker.FirstShape().IsNull()) first = maker.FirstShape();
    if (g + 1 == counts.size() && !maker.LastShape().IsNull()) last = maker.LastShape();
  }
  if (!first.IsNull()) sewing.Add(first);
  if (!last.IsNull()) sewing.Add(last);
  sewing.Perform();
  TopoDS_Shape result = sewing.SewedShape();
  if (solid) {
    TopoDS_Shell shell;
    for (TopExp_Explorer it(result, TopAbs_SHELL); it.More(); it.Next()) shell = TopoDS::Shell(it.Current());
    if (shell.IsNull()) throw std::runtime_error("loft: the pieces don't close");
    BRepBuilderAPI_MakeSolid made(shell);
    ShapeFix_Solid fix(made.Solid());
    fix.Perform();
    result = fix.Solid();
  }
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History;
  op->result = result;
  if (!first.IsNull()) op->first.push_back(sewing.Modified(first));
  if (!last.IsNull()) op->last.push_back(sewing.Modified(last));
  // Onshape keeps such pieces' sides B-splines, flat or not (batch 057 c010, scaled along a line then an arc).
  return op;
}

// The outer wire of a face.
std::unique_ptr<Shape> outer_wire(const Shape &face) {
  return wrap(BRepTools::OuterWire(TopoDS::Face(face.shape)));
}

}  // namespace occt
