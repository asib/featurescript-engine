// Modelling operations beyond extrude and booleans: revolve, primitives, splitting and general transforms.

#include "occt/cpp/occt.h"

#include <BRepGProp.hxx>
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
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRepTools_ReShape.hxx>
#include <ShapeAnalysis_CanonicalRecognition.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Defeaturing.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <gp_Pnt2d.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <Law_Linear.hxx>
#include <Law_S.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepFeat_SplitShape.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <ShapeFix_Solid.hxx>
#include <ShapeFix_Shell.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
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
    op->result = planes_recognized(maker.Shape(), op->history);
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

// Rounds edges of a solid with a constant radius.
std::unique_ptr<Operation> fillet_op(const Shape &solid, const ShapeList &edges, double radius) {
  BRepFilletAPI_MakeFillet maker(solid.shape);
  for (const TopoDS_Shape &e : edges.shapes) maker.Add(radius, TopoDS::Edge(e));
  maker.Build();
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

// Chamfers edges: kind 0 equal distances `d1`; kind 1 distances `d1` on `faces[i]` and `d2` on the other face;
// kind 2 distance `d1` on `faces[i]` and angle `d2`. `faces` has a face next to each edge (unused for kind 0).
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

// Shells a solid, removing `faces`: `thickness` positive outward, negative inward. Corners are sharp, as in Onshape.
std::unique_ptr<Operation> shell_op(const Shape &solid, const ShapeList &faces, double thickness) {
  BRepOffsetAPI_MakeThickSolid maker;
  maker.MakeThickSolidByJoin(solid.shape, to_list(faces), thickness, 1e-7, BRepOffset_Skin, Standard_False,
                             Standard_False, GeomAbs_Intersection);
  maker.Build();
  return finish(one(solid.shape), maker, "shell");
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
  return finish(one(solid.shape), maker, "draft");
}

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
    const TopoDS_Face &face = TopoDS::Face(faces(1));
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
    back.Initialize(sheet, -t2, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc);
    back.MakeOffsetShape();
    if (!back.IsDone()) throw std::runtime_error("thicken: offset failed");
    op->history->Merge(one(sheet), back);
    base = back.Shape();
  }
  BRepOffset_MakeOffset maker;
  maker.Initialize(base, t1 + t2, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc,
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
  for (TopExp_Explorer it(f, TopAbs_EDGE); it.More(); it.Next()) {
    const TopTools_ListOfShape *faces = edge_faces.Seek(it.Current());
    if (faces == nullptr) continue;
    for (const TopoDS_Shape &g : *faces) {
      if (g.IsSame(f)) continue;
      BRepAdaptor_Surface surface(TopoDS::Face(g));
      if (surface.GetType() == GeomAbs_Plane) {
        if (std::abs(surface.Plane().Axis().Direction().Dot(n)) > 1e-9)
          throw std::runtime_error("replace face: a neighbouring face isn't perpendicular (" +
                                   std::to_string(surface.Plane().Axis().Direction().Dot(n)) + ")");
      } else if (surface.GetType() == GeomAbs_Cylinder) {
        if (!surface.Cylinder().Axis().Direction().IsParallel(n, 1e-9))
          throw std::runtime_error("replace face: a neighbouring face isn't perpendicular");
      } else {
        throw std::runtime_error("replace face: a neighbouring face isn't planar or cylindrical");
      }
    }
  }
  Bnd_Box bounds;
  BRepBndLib::Add(solid.shape, bounds);
  double x0, y0, z0, x1, y1, z1;
  bounds.Get(x0, y0, z0, x1, y1, z1);
  double size = gp_Pnt(x0, y0, z0).Distance(gp_Pnt(x1, y1, z1));
  gp_Pnt origin = pnt(target.origin);
  double reach = 10 * size + 10 * origin.Distance(gp_Pnt((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2)) + 1;
  // Extend the solid along the face's normal, past the target plane however it's tilted.
  TopoDS_Shape copy = BRepBuilderAPI_Copy(f).Shape();
  BRepPrimAPI_MakePrism prism(copy, gp_Vec(n) * reach, Standard_True);
  BRepAlgoAPI_Fuse fuse(solid.shape, prism.Shape());
  fuse.SetRunParallel(Standard_False);
  fuse.Build();
  if (!fuse.IsDone()) throw std::runtime_error("replace face: extending failed");
  // Cut with the target plane and keep what's behind it.
  gp_Dir tn = dir(target.normal);
  if (tn.Dot(n) < 0) tn.Reverse();
  TopoDS_Face cutter = BRepBuilderAPI_MakeFace(gp_Pln(origin, tn), -reach * 2, reach * 2, -reach * 2, reach * 2).Face();
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
    if (gp_Vec(origin, props.CentreOfMass()).Dot(gp_Vec(tn)) < 0) {
      builder.Add(kept, it.Current());
      ++count;
    }
  }
  if (count == 0) throw std::runtime_error("replace face: nothing left");
  ShapeUpgrade_UnifySameDomain unifier(kept, Standard_True, Standard_True, Standard_False);
  unifier.Build();
  auto op = std::make_unique<Operation>();
  // Keep the identities of faces that survive: track the fuse, the split and the merge.
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
std::unique_ptr<Operation> split_faces_op(const Shape &body, const ShapeList &faces, const ShapeList &tools) {
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
  if (section.IsDone())
    for (TopExp_Explorer it(section.Shape(), TopAbs_EDGE); it.More(); it.Next()) {
      TopoDS_Shape face;
      if (!section.HasAncestorFaceOn1(it.Current(), face)) continue;
      splitter.Add(TopoDS::Edge(it.Current()), TopoDS::Face(face));
      any = true;
    }
  if (!any) throw std::runtime_error("split faces: the tools don't cross the faces");
  splitter.Build();
  return finish(one(body.shape), splitter, "split faces");
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
std::unique_ptr<Operation> sweep_op(const Shape &profile, const Shape &path) {
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
  BRepOffsetAPI_MakePipe maker(spine, profile.shape);
  maker.Build();
  auto op = finish(one(profile.shape), maker, "sweep");
  if (profile.shape.ShapeType() == TopAbs_FACE) {
    op->first.push_back(maker.FirstShape());
    op->last.push_back(maker.LastShape());
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

}  // namespace

// Lofts through wires in order: a solid through closed wires if `solid`, else a sheet.
std::unique_ptr<Operation> loft_op(const ShapeList &wires, bool solid) {
  BRepOffsetAPI_ThruSections maker(solid ? Standard_True : Standard_False, Standard_False, 1e-7);
  maker.CheckCompatibility(Standard_True);
  for (const TopoDS_Shape &w : wires.shapes) maker.AddWire(TopoDS::Wire(w));
  maker.Build();
  auto op = finish(to_list(wires), maker, "loft");
  op->result = planes_recognized(op->result, op->history);
  TopoDS_Shape first = maker.FirstShape(), last = maker.LastShape();
  if (!first.IsNull()) op->first.push_back(first);
  if (!last.IsNull()) op->last.push_back(last);
  return op;
}

// The outer wire of a face.
std::unique_ptr<Shape> outer_wire(const Shape &face) {
  return wrap(BRepTools::OuterWire(TopoDS::Face(face.shape)));
}

}  // namespace occt
