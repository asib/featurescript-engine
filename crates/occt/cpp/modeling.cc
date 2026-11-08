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
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <Geom_BSplineSurface.hxx>
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
#include <BRepLib.hxx>
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
  bool crossed = false;
  if (section.IsDone())
    for (TopExp_Explorer it(section.Shape(), TopAbs_EDGE); it.More(); it.Next()) {
      // Edges where the tools meet the faces' edges have no single face to go to.
      crossed = true;
      TopoDS_Shape face;
      if (!section.HasAncestorFaceOn1(it.Current(), face)) continue;
      if (on_boundary(TopoDS::Edge(it.Current()), face)) continue;
      splitter.Add(TopoDS::Edge(it.Current()), TopoDS::Face(face));
      any = true;
    }
  if (!crossed) throw std::runtime_error("split faces: the tools don't cross the faces");
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
std::unique_ptr<Operation> sweep_controlled_op(const Shape &profile, const Shape &path, int mode, Vec3 direction) {
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

// A loft between two profiles whose ends leave them along conditions (a magnitude that isn't a number leaves that
// end free). Each side becomes one cubic span across, from the first profile through rows set by the derivatives
// (a free end's second derivative zero) to the last (batches 055, 056). A chord from the first profile to the last
// gives a derivative: along a vector v (the profile's normal, or a direction), the magnitude times 0.9 of the chord
// along v plus 0.1 of its length; square to v (tangent to the profile's plane), the magnitude times the chord's
// part there. On a profile of several edges the derivative goes from its value at one vertex to the next as
// 3t² − 2t³; on a profile of one closed edge it is the chord's at each point along v, and square to v the periodic
// cubic spline through its values a quarter of the way round each time. The sides' rows are cubic Hermite fits of
// 64 spans (as Onshape's profiles are).
std::unique_ptr<Operation> loft_clamped_op(const ShapeList &wires, bool solid, Vec3 start_dir, double start_mag,
                                           bool start_tangent, Vec3 end_dir, double end_mag, bool end_tangent) {
  if (wires.shapes.size() != 2) throw std::runtime_error("loft: end conditions need two profiles");
  BRepOffsetAPI_ThruSections maker(solid ? Standard_True : Standard_False, Standard_False, 1e-7);
  maker.CheckCompatibility(Standard_True);
  for (const TopoDS_Shape &w : wires.shapes) maker.AddWire(TopoDS::Wire(w));
  maker.Build();
  if (!maker.IsDone()) throw std::runtime_error("loft failed");
  const TopoDS_Shape lofted = maker.Shape();
  TopTools_IndexedMapOfShape caps;
  if (!maker.FirstShape().IsNull()) caps.Add(maker.FirstShape());
  if (!maker.LastShape().IsNull()) caps.Add(maker.LastShape());
  // The wire as lofted: the first section's edges as the sides' first rows meet them.
  const WireRun run(TopoDS::Wire(wires.shapes[0]));
  auto centre = [](const TopoDS_Shape &w) {
    GProp_GProps props;
    BRepGProp::LinearProperties(w, props);
    return props.CentreOfMass();
  };
  auto c_first = [&] { return centre(wires.shapes[0]); };
  auto c_last = [&] { return centre(wires.shapes[1]); };
  BRepExtrema_DistShapeShape near_first;
  near_first.LoadS1(wires.shapes[0]);
  struct Side {
    TopoDS_Face face;
    Handle(Geom_Surface) surface;
    double u1, u2, v_first, v_last;
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
    // Across the profiles is v unless a u iso line of the face lies on the first profile.
    near_first.LoadS2(BRepBuilderAPI_MakeVertex(surface->Value((u1 + u2) / 2, v1)).Vertex());
    near_first.Perform();
    const bool v_first_low = near_first.IsDone() && near_first.Value() < 1e-6;
    near_first.LoadS2(BRepBuilderAPI_MakeVertex(surface->Value((u1 + u2) / 2, v2)).Vertex());
    near_first.Perform();
    const bool v_first_high = near_first.IsDone() && near_first.Value() < 1e-6;
    if (!v_first_low && !v_first_high) throw std::runtime_error("loft: a side doesn't run across the profiles");
    sides.push_back({face, surface, u1, u2, v_first_low ? v1 : v2, v_first_low ? v2 : v1});
  }
  if (sides.empty()) throw std::runtime_error("loft: no sides");
  // The chord from the first profile to the last at a point of the first.
  auto chord_at = [&](const gp_Pnt &q) {
    double best = 1e300;
    gp_Vec chord;
    for (const Side &side : sides) {
      GeomAPI_ProjectPointOnSurf projection(q, side.surface, side.u1, side.u2, std::min(side.v_first, side.v_last),
                                            std::max(side.v_first, side.v_last), 1e-9);
      if (!projection.IsDone() || projection.NbPoints() == 0) continue;
      double u, v;
      projection.LowerDistanceParameters(u, v);
      if (projection.LowerDistance() >= best) continue;
      best = projection.LowerDistance();
      chord = gp_Vec(side.surface->Value(u, side.v_first), side.surface->Value(u, side.v_last));
    }
    return chord;
  };
  const bool has_start = !std::isnan(start_mag), has_end = !std::isnan(end_mag);
  struct End {
    gp_Vec v;
    double mag;
    bool tangent;
  } ends[2] = {{gp_Vec(dir(start_dir)), has_start ? start_mag : 0, start_tangent},
               {gp_Vec(dir(end_dir)), has_end ? end_mag : 0, end_tangent}};
  // The derivative a chord gives: along v, 0.9 of the chord along it plus 0.1 of its length; square to v, the
  // chord's part there. v points from the first profile towards the last.
  for (End &end : ends) {
    end.v.Normalize();
    gp_Vec across(c_first(), c_last());
    if (across.Dot(end.v) < 0) end.v.Reverse();
  }
  auto from_chord = [&](const End &end, const gp_Vec &c) {
    return end.tangent ? (c - end.v * c.Dot(end.v)) * end.mag
                       : end.v * (end.mag * (0.9 * c.Dot(end.v) + 0.1 * c.Magnitude()));
  };
  // A profile of one closed edge: along a vector, the chord at each point; tangent to the plane, the periodic
  // spline through the chords a quarter of the way round each time. A profile of several edges: the chords at its
  // vertices, along each edge from one to the next as 3t² − 2t³.
  const bool closed_curve = run.pieces.size() == 1;
  std::vector<QuarterSpline> quarters;
  std::vector<std::array<gp_Vec, 2>> at_vertices;
  for (const End &end : ends) {
    gp_Vec values[4];
    for (int k = 0; k < 4; ++k) values[k] = from_chord(end, chord_at(run.at(k / 4.0)));
    quarters.emplace_back(values);
  }
  std::vector<gp_Vec> vertex_chords;
  for (const WireRun::Piece &piece : run.pieces) vertex_chords.push_back(chord_at(run.at(piece.start / run.total)));
  // The derivative and how it changes along the profile (per unit of the profile's length fraction s), at u on a
  // side, where the first profile is at s and the side's u runs ds per unit of u.
  auto derivative = [&](int which, const Side &side, double u, double s, size_t piece, double within, double ds) {
    const End &end = ends[which];
    if (closed_curve && end.tangent) return quarters[which].at(s);
    if (closed_curve) {
      // The chord here: straight across the side.
      auto chord = [&](double w) { return gp_Vec(side.surface->Value(w, side.v_first), side.surface->Value(w, side.v_last)); };
      const double h = (side.u2 - side.u1) * 1e-6 * (u + (side.u2 - side.u1) * 1e-6 > side.u2 ? -1 : 1);
      const gp_Vec here = from_chord(end, chord(u)), ahead = from_chord(end, chord(u + h));
      return std::make_pair(here, (ahead - here) / (h * ds));
    }
    const gp_Vec a = from_chord(end, vertex_chords[piece]),
                 b = from_chord(end, vertex_chords[(piece + 1) % vertex_chords.size()]);
    const double t = within, len = run.pieces[piece].length / run.total;
    return std::make_pair(a + (b - a) * (3 * t * t - 2 * t * t * t), (b - a) * ((6 * t - 6 * t * t) / len));
  };
  BRepBuilderAPI_Sewing sewing(1e-6);
  for (const TopoDS_Shape &cap : kept) sewing.Add(cap);
  const int spans = 64;
  for (const Side &side : sides) {
    // Each row's value and derivative along u at the span ends.
    std::vector<std::array<gp_Pnt, 4>> value(spans + 1);
    std::vector<std::array<gp_Vec, 4>> slope(spans + 1);
    for (int j = 0; j <= spans; ++j) {
      const double u = side.u1 + (side.u2 - side.u1) * j / spans;
      gp_Pnt r0, r3;
      gp_Vec r0u, r0v, r3u, r3v;
      side.surface->D1(u, side.v_first, r0, r0u, r0v);
      side.surface->D1(u, side.v_last, r3, r3u, r3v);
      const auto [s, tangent, piece, along_piece] = run.place(r0);
      const double ds = r0u.Dot(tangent) / run.total;
      const auto [a, da] = derivative(0, side, u, s, piece, along_piece, ds);
      const auto [b, db] = derivative(1, side, u, s, piece, along_piece, ds);
      gp_Pnt r1 = r0.Translated(a / 3), r2 = r3.Translated(-b / 3);
      gp_Vec r1u = r0u + da * (ds / 3), r2u = r3u - db * (ds / 3);
      if (!has_start) {
        r1 = gp_Pnt((r0.XYZ() + r2.XYZ()) / 2);
        r1u = (r0u + r2u) / 2;
      }
      if (!has_end) {
        r2 = gp_Pnt((r1.XYZ() + r3.XYZ()) / 2);
        r2u = (r1u + r3u) / 2;
      }
      value[j] = {r0, r1, r2, r3};
      slope[j] = {r0u, r1u, r2u, r3u};
    }
    TColgp_Array2OfPnt poles(1, 3 * spans + 1, 1, 4);
    const double step = (side.u2 - side.u1) / spans;
    for (int row = 0; row < 4; ++row) {
      for (int j = 0; j < spans; ++j) {
        poles(3 * j + 1, row + 1) = value[j][row];
        poles(3 * j + 2, row + 1) = value[j][row].Translated(slope[j][row] * (step / 3));
        poles(3 * j + 3, row + 1) = value[j + 1][row].Translated(-slope[j + 1][row] * (step / 3));
      }
      poles(3 * spans + 1, row + 1) = value[spans][row];
    }
    TColStd_Array1OfReal uknots(1, spans + 1), vknots(1, 2);
    TColStd_Array1OfInteger umults(1, spans + 1), vmults(1, 2);
    for (int j = 0; j <= spans; ++j) {
      uknots(j + 1) = side.u1 + step * j;
      umults(j + 1) = (j == 0 || j == spans) ? 4 : 3;
    }
    vknots(1) = 0;
    vknots(2) = 1;
    vmults(1) = vmults(2) = 4;
    Handle(Geom_BSplineSurface) s = new Geom_BSplineSurface(poles, uknots, vknots, umults, vmults, 3, 3);
    sewing.Add(BRepBuilderAPI_MakeFace(s, side.u1, side.u2, 0, 1, 1e-7).Face());
  }
  sewing.Perform();
  TopoDS_Shape result = sewing.SewedShape();
  if (solid) {
    TopoDS_Shell shell;
    for (TopExp_Explorer it(result, TopAbs_SHELL); it.More(); it.Next()) shell = TopoDS::Shell(it.Current());
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

// The outer wire of a face.
std::unique_ptr<Shape> outer_wire(const Shape &face) {
  return wrap(BRepTools::OuterWire(TopoDS::Face(face.shape)));
}

}  // namespace occt
