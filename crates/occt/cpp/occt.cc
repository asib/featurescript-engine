#include "occt/cpp/occt.h"
#include <ShapeFix_Edge.hxx>
#include <Geom2d_Curve.hxx>
#include <BRepBuilderAPI_Copy.hxx>
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

// Splits a face larger than all the edges by the edges, then drops the faces that reach the big face's boundary.
// What's left are the regions the edges enclose, oriented with the frame's normal.
std::unique_ptr<ShapeList> sketch_regions(const ShapeList &edges, Frame f) {
  auto out = std::make_unique<ShapeList>();
  if (edges.shapes.empty()) return out;
  gp_Pln plane(ax3(f));
  Bnd_Box box;
  for (const TopoDS_Shape &e : edges.shapes) BRepBndLib::Add(e, box);
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
  TopTools_ListOfShape arguments;
  arguments.Append(outer.Face());
  splitter.SetArguments(arguments);
  splitter.SetTools(to_list(edges));
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
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(splitter.Shape(), TopAbs_FACE, faces);
  for (int i = 1; i <= faces.Extent(); ++i) {
    TopTools_IndexedMapOfShape face_edges;
    TopExp::MapShapes(faces(i), TopAbs_EDGE, face_edges);
    bool outside = false;
    for (int j = 1; j <= face_edges.Extent() && !outside; ++j) outside = boundary.Contains(face_edges(j));
    if (!outside) out->shapes.push_back(faces(i));
  }
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
  }
  return op;
}

std::unique_ptr<Shape> operation_shape(const Operation &op) { return wrap(op.result); }

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

}  // namespace

Box3 bounding_box(const Shape &s, bool precise) {
  Bnd_Box box;
  BRepBndLib::AddOptimal(s.shape, box, precise ? Standard_False : Standard_True, Standard_False);
  if (box.IsVoid()) return Box3{Vec3{0, 0, 0}, Vec3{0, 0, 0}, true};
  double lo[3], hi[3];
  box.Get(lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
  if (has_free_form_faces(s.shape)) {
    // OCCT's boxes of B-spline faces can miss part of the face (a scaled sphere moved away from the origin loses
    // a sliver), even loose ones, so measure each side exactly: the distance from a plane just outside the box to
    // the shape.
    double size = std::max({hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], 1e-6});
    double margin = size;
    for (int axis = 0; axis < 3; ++axis) {
      for (int side = 0; side < 2; ++side) {
        gp_Dir normal(axis == 0, axis == 1, axis == 2);
        double at = side == 0 ? lo[axis] - margin : hi[axis] + margin;
        gp_Pnt origin((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2);
        origin.SetCoord(axis + 1, at);
        double reach = 4 * size;
        TopoDS_Face plane = BRepBuilderAPI_MakeFace(gp_Pln(origin, normal), -reach, reach, -reach, reach).Face();
        BRepExtrema_DistShapeShape extrema(plane, s.shape);
        if (!extrema.IsDone() || extrema.NbSolution() == 0) continue;
        if (side == 0)
          lo[axis] = at + extrema.Value();
        else
          hi[axis] = at - extrema.Value();
      }
    }
  }
  return Box3{Vec3{lo[0], lo[1], lo[2]}, Vec3{hi[0], hi[1], hi[2]}, false};
}

// The box around a shape in the coordinates of `frame` (origin, z = normal, x).
Box3 bounding_box_in(const Shape &s, Frame frame, bool precise) {
  gp_Trsf to_local;
  to_local.SetTransformation(ax3(frame));
  return bounding_box(Shape(s.shape.Moved(TopLoc_Location(to_local))), precise);
}

// OCCT integrates some free-form faces badly: extrusions and revolutions of B-splines by 6e-4, closed B-spline
// faces (a scaled sphere) by 1e-5. Converting free-form faces to NURBS and splitting closed ones into open patches
// first makes areas and volumes accurate to about 1e-8.
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

TopoDS_Shape prepared_for_integration(const TopoDS_Shape &input) {
  const TopoDS_Shape shape = with_accurate_pcurves(input);
  if (!has_free_form_faces(shape)) return shape;
  TopoDS_Shape result = shape;
  // Only swept and offset faces need converting; converting some imported B-spline shapes crashes OCCT.
  bool swept = false;
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More() && !swept; it.Next()) {
    switch (BRepAdaptor_Surface(TopoDS::Face(it.Current()), Standard_False).GetType()) {
      case GeomAbs_SurfaceOfExtrusion:
      case GeomAbs_SurfaceOfRevolution:
      case GeomAbs_OffsetSurface:
        swept = true;
        break;
      default:
        break;
    }
  }
  if (swept) {
    BRepBuilderAPI_NurbsConvert converter(result, Standard_True);
    if (converter.IsDone()) result = converter.Shape();
  }
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
  return result;
}

// Volumes and areas use adaptive integration: the default Gauss rule is off by 1e-4 on rational B-spline surfaces,
// such as a scaled sphere.
constexpr double GPROP_EPS = 1e-12;

double shape_volume(const Shape &s) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(prepared_for_integration(s.shape), props, GPROP_EPS, Standard_True);
  return props.Mass();
}

double shape_area(const Shape &s) {
  GProp_GProps props;
  BRepGProp::SurfaceProperties(prepared_for_integration(s.shape), props, GPROP_EPS, Standard_True);
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

// The principal moments of inertia about the centre of mass, smallest first.
Vec3 shape_principal_moments(const Shape &s) {
  GProp_GProps props;
  switch (s.shape.ShapeType()) {
    case TopAbs_VERTEX: return Vec3{0, 0, 0};
    case TopAbs_EDGE:
    case TopAbs_WIRE: BRepGProp::LinearProperties(s.shape, props); break;
    case TopAbs_FACE:
    case TopAbs_SHELL: BRepGProp::SurfaceProperties(s.shape, props); break;
    default: BRepGProp::VolumeProperties(prepared_for_integration(s.shape), props, GPROP_EPS, Standard_True); break;
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
std::unique_ptr<ShapeList> visible_sub_shapes(const Shape &s, int32_t kind) {
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

bool shape_valid(const Shape &s) { return BRepCheck_Analyzer(s.shape).IsValid(); }

// Writes AP242 in metres, naming each shape's product and, as Onshape does, its MANIFOLD_SOLID_BREP. The solids
// are named in the order they were transferred, so each shape must be a single solid for its name to land on it.
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
