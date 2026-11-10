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
#include <Geom_BSplineCurve.hxx>
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
  // The faces round it: perpendicular to it (planes or cylinders along its normal), or planes leaning.
  std::vector<TopoDS_Face> neighbours;
  bool square = true;
  for (TopExp_Explorer it(f, TopAbs_EDGE); it.More(); it.Next()) {
    const TopTools_ListOfShape *faces = edge_faces.Seek(it.Current());
    if (faces == nullptr) continue;
    for (const TopoDS_Shape &g : *faces) {
      if (g.IsSame(f)) continue;
      BRepAdaptor_Surface surface(TopoDS::Face(g));
      if (surface.GetType() == GeomAbs_Plane) {
        if (std::abs(surface.Plane().Axis().Direction().Dot(n)) > 1e-9) square = false;
        neighbours.push_back(TopoDS::Face(g));
      } else if (surface.GetType() == GeomAbs_Cylinder) {
        if (!surface.Cylinder().Axis().Direction().IsParallel(n, 1e-9))
          throw std::runtime_error("replace face: a neighbouring face isn't perpendicular");
        square = false;
        neighbours.clear();
        break;
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
  // Extend the solid past the target plane, however it's tilted: along the face's normal when the faces round it
  // are square to it; otherwise, where leaning planes are round it, by the region inside all their planes and
  // beyond the face's (batch 058 c003, a trapezoid prism's top).
  TopoDS_Shape extension;
  if (square || neighbours.empty()) {
    TopoDS_Shape copy = BRepBuilderAPI_Copy(f).Shape();
    extension = BRepPrimAPI_MakePrism(copy, gp_Vec(n) * reach, Standard_True).Shape();
  } else {
    const gp_Pnt c((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
    extension = BRepPrimAPI_MakeBox(gp_Pnt(c.X() - reach, c.Y() - reach, c.Z() - reach), 2 * reach, 2 * reach, 2 * reach).Shape();
    auto keep_side = [&](const TopoDS_Face &plane_face, const gp_Pnt &inside) {
      BRepAdaptor_Surface surface(plane_face);
      TopoDS_Face big = BRepBuilderAPI_MakeFace(surface.Plane(), -4 * reach, 4 * reach, -4 * reach, 4 * reach).Face();
      BRepAlgoAPI_Common common(extension, BRepPrimAPI_MakeHalfSpace(big, inside).Solid());
      common.SetRunParallel(Standard_False);
      common.Build();
      if (!common.IsDone()) throw std::runtime_error("replace face: extending failed");
      extension = common.Shape();
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
    keep_side(f, props.CentreOfMass().Translated(gp_Vec(n) * (1e-3 * size)));
  }
  BRepAlgoAPI_Fuse fuse(solid.shape, extension);
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
  arguments.Append(extension);
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
std::unique_ptr<Operation> merge_faces_across_op(const Shape &body, const ShapeList &removable) {
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
  op->result = unifier.Shape();
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
  // Along curves other than lines and arcs, OpenCascade fits the swept surfaces to 1e-4 in model units (here
  // metres: 0.065 mm off along a spline, batch 072 c002); swept a thousand times larger, then shrunk back, they fit
  // to a tenth of a micrometre.
  bool spline_path = false;
  for (TopExp_Explorer e(spine, TopAbs_EDGE); e.More(); e.Next()) {
    GeomAbs_CurveType type = BRepAdaptor_Curve(TopoDS::Edge(e.Current())).GetType();
    if (type != GeomAbs_Line && type != GeomAbs_Circle) spline_path = true;
  }
  std::unique_ptr<Operation> op;
  if (spline_path) {
    gp_Trsf up;
    up.SetScale(gp_Pnt(0, 0, 0), 1000.0);
    BRepBuilderAPI_Transform big_profile(profile.shape, up, Standard_True);
    BRepBuilderAPI_Transform big_spine(spine, up, Standard_True);
    BRepOffsetAPI_MakePipe maker(TopoDS::Wire(big_spine.Shape()), big_profile.Shape());
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
    // A side ending at a point has a degenerate row there: the face's edge there is degenerate.
    ShapeFix_Face fix(BRepBuilderAPI_MakeFace(s, side.u1, side.u2, 0, 1, 1e-7).Face());
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
