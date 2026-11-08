// Modelling operations beyond extrude and booleans: revolve, primitives, splitting and general transforms.

#include "occt/cpp/occt.h"

#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopoDS_Compound.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Defeaturing.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepAdaptor_Surface.hxx>
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
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <gp_Ax1.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Trsf.hxx>

#include <cmath>
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
  } else {
    gp_GTrsf gtrsf;
    for (int i = 0; i < 3; ++i) {
      for (int j = 0; j < 3; ++j) gtrsf.SetValue(i + 1, j + 1, t.m[i * 4 + j]);
      gtrsf.SetValue(i + 1, 4, t.m[i * 4 + 3]);
    }
    BRepBuilderAPI_GTransform maker(s.shape, gtrsf, Standard_True);
    check_done(maker, "transform");
    op->history = new BRepTools_History(arguments, maker);
    op->result = maker.Shape();
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
                                    Frame neutral) {
  BRepOffsetAPI_DraftAngle maker(solid.shape);
  gp_Pln plane(pnt(neutral.origin), dir(neutral.normal));
  for (const TopoDS_Shape &f : faces.shapes) {
    maker.Add(TopoDS::Face(f), dir(pull), angle, plane);
    if (!maker.AddDone()) throw std::runtime_error("draft: a face can't be drafted");
  }
  maker.Build();
  return finish(one(solid.shape), maker, "draft");
}

// Thickens a face or sheet `t1` along its normal and `t2` against it. Planar faces are swept exactly; curved ones
// are offset.
std::unique_ptr<Operation> thicken_op(const Shape &input, double t1, double t2) {
  // Work on a copy: a face of a solid must not share topology with the new body.
  Shape sheet(BRepBuilderAPI_Copy(input.shape).Shape());
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(sheet.shape, TopAbs_FACE, faces);
  bool planar = faces.Extent() > 0;
  for (int i = 1; i <= faces.Extent(); ++i)
    if (BRepAdaptor_Surface(TopoDS::Face(faces(i))).GetType() != GeomAbs_Plane) planar = false;
  if (planar && faces.Extent() == 1) {
    const TopoDS_Face &face = TopoDS::Face(faces(1));
    gp_Vec n(face_normal(face));
    gp_Trsf back;
    back.SetTranslation(n * -t2);
    TopoDS_Shape start = BRepBuilderAPI_Transform(face, back, Standard_True).Shape();
    BRepPrimAPI_MakePrism maker(start, n * (t1 + t2), Standard_False, Standard_True);
    maker.Build();
    return finish(one(sheet.shape), maker, "thicken");
  }
  // BRepOffset_MakeOffset in thickening mode keeps analytic surfaces exact (a cylinder offsets to a cylinder);
  // the simpler offset turns them into B-splines.
  TopoDS_Shape base = sheet.shape;
  if (t2 != 0) {
    BRepOffset_MakeOffset back;
    back.Initialize(sheet.shape, -t2, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc);
    back.MakeOffsetShape();
    if (!back.IsDone()) throw std::runtime_error("thicken: offset failed");
    base = back.Shape();
  }
  BRepOffset_MakeOffset maker;
  maker.Initialize(base, t1 + t2, 1e-7, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc,
                   Standard_True);
  maker.MakeOffsetShape();
  if (!maker.IsDone()) throw std::runtime_error("thicken failed");
  auto op = std::make_unique<Operation>();
  op->history = new BRepTools_History();
  op->result = maker.Shape();
  // Thickening a face of a solid along its outward normal can come out inside out.
  GProp_GProps props;
  BRepGProp::VolumeProperties(op->result, props);
  if (props.Mass() < 0) op->result.Reverse();
  return op;
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
  // BRepOffset_MakeOffset has no history interface: record each input face's image.
  TopTools_IndexedMapOfShape inputs;
  TopExp::MapShapes(solid.shape, TopAbs_FACE, inputs);
  for (int i = 1; i <= inputs.Extent(); ++i) {
    TopTools_ListOfShape images;
    maker.OffsetFacesFromShapes().LastImage(inputs(i), images);
    for (const TopoDS_Shape &image : images)
      if (!image.IsSame(inputs(i))) op->history->AddModified(inputs(i), image);
  }
  op->result = maker.Shape();
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
          throw std::runtime_error("replace face: a neighbouring face isn't perpendicular");
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

}  // namespace occt
