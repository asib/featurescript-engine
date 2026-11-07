#include "occt/cpp/occt.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
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
#include <Interface_InterfaceModel.hxx>
#include <Interface_Static.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <STEPControl_Reader.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <StepShape_ManifoldSolidBrep.hxx>
#include <TCollection_ExtendedString.hxx>
#include <TCollection_HAsciiString.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_MapOfShape.hxx>
#include <TopoDS.hxx>
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

namespace occt {
namespace {

gp_Pnt pnt(const Vec3 &v) { return gp_Pnt(v.x, v.y, v.z); }
gp_Dir dir(const Vec3 &v) { return gp_Dir(v.x, v.y, v.z); }
Vec3 vec3(const gp_XYZ &p) { return Vec3{p.X(), p.Y(), p.Z()}; }
gp_Ax3 ax3(const Frame &f) { return gp_Ax3(pnt(f.origin), dir(f.normal), dir(f.x)); }
gp_Ax2 ax2(const Frame &f) { return gp_Ax2(pnt(f.origin), dir(f.normal), dir(f.x)); }

std::unique_ptr<Shape> wrap(const TopoDS_Shape &s) { return std::make_unique<Shape>(s); }

std::unique_ptr<ShapeList> wrap_list(const TopTools_ListOfShape &l) {
  auto out = std::make_unique<ShapeList>();
  for (const TopoDS_Shape &s : l) out->shapes.push_back(s);
  return out;
}

TopTools_ListOfShape to_list(const ShapeList &l) {
  TopTools_ListOfShape out;
  for (const TopoDS_Shape &s : l.shapes) out.Append(s);
  return out;
}

TopAbs_ShapeEnum shape_enum(int32_t kind) {
  if (kind < TopAbs_COMPOUND || kind > TopAbs_SHAPE) throw std::invalid_argument("bad shape kind");
  return static_cast<TopAbs_ShapeEnum>(kind);
}

void check_done(const BRepBuilderAPI_MakeShape &maker, const char *what) {
  if (!maker.IsDone()) throw std::runtime_error(std::string(what) + " failed");
}

}  // namespace

std::unique_ptr<Shape> shape_clone(const Shape &s) { return wrap(s.shape); }
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

std::unique_ptr<Operation> prism(const Shape &profile, Vec3 v) {
  BRepPrimAPI_MakePrism maker(profile.shape, gp_Vec(v.x, v.y, v.z), Standard_False, Standard_True);
  maker.Build();
  check_done(maker, "extrude");
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape arguments;
  arguments.Append(profile.shape);
  op->history = new BRepTools_History(arguments, maker);
  op->result = maker.Shape();
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(profile.shape, TopAbs_FACE, faces);
  for (int i = 1; i <= faces.Extent(); ++i) {
    op->first.push_back(maker.FirstShape(faces(i)));
    op->last.push_back(maker.LastShape(faces(i)));
  }
  return op;
}

// kind: 0 union, 1 subtraction, 2 intersection. `unify` merges faces and edges left on the same surface or curve.
std::unique_ptr<Operation> boolean_op(int32_t kind, const ShapeList &objects, const ShapeList &tools, bool unify) {
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
    ShapeUpgrade_UnifySameDomain unifier(op->result, Standard_True, Standard_True, Standard_False);
    unifier.Build();
    op->history->Merge(unifier.History());
    op->result = unifier.Shape();
  }
  return op;
}

std::unique_ptr<Operation> transform_op(const Shape &s, const Transform &t) {
  gp_Trsf trsf;
  trsf.SetValues(t.m[0], t.m[1], t.m[2], t.m[3], t.m[4], t.m[5], t.m[6], t.m[7], t.m[8], t.m[9], t.m[10], t.m[11]);
  BRepBuilderAPI_Transform maker(s.shape, trsf, Standard_True);
  check_done(maker, "transform");
  auto op = std::make_unique<Operation>();
  TopTools_ListOfShape arguments;
  arguments.Append(s.shape);
  op->history = new BRepTools_History(arguments, maker);
  op->result = maker.Shape();
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

Box3 bounding_box(const Shape &s, bool precise) {
  Bnd_Box box;
  if (precise)
    BRepBndLib::AddOptimal(s.shape, box, Standard_False, Standard_False);
  else
    BRepBndLib::Add(s.shape, box);
  if (box.IsVoid()) return Box3{Vec3{0, 0, 0}, Vec3{0, 0, 0}, true};
  double x0, y0, z0, x1, y1, z1;
  box.Get(x0, y0, z0, x1, y1, z1);
  return Box3{Vec3{x0, y0, z0}, Vec3{x1, y1, z1}, false};
}

double shape_volume(const Shape &s) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(s.shape, props);
  return props.Mass();
}

double shape_area(const Shape &s) {
  GProp_GProps props;
  BRepGProp::SurfaceProperties(s.shape, props);
  return props.Mass();
}

double shape_length(const Shape &s) {
  GProp_GProps props;
  BRepGProp::LinearProperties(s.shape, props);
  return props.Mass();
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
  return Frame{vec3(position.Location().XYZ()), vec3(normal.XYZ()), vec3(position.XDirection().XYZ())};
}

int32_t curve_type(const Shape &edge) {
  return static_cast<int32_t>(BRepAdaptor_Curve(TopoDS::Edge(edge.shape)).GetType());
}

bool shape_valid(const Shape &s) { return BRepCheck_Analyzer(s.shape).IsValid(); }

// Writes AP242 in metres, naming each shape's product.
void write_step(const ShapeList &shapes, const rust::Vec<rust::String> &names, rust::Str path) {
  if (shapes.shapes.size() != names.size()) throw std::invalid_argument("one name per shape");
  Handle(TDocStd_Document) document;
  XCAFApp_Application::GetApplication()->NewDocument("MDTV-XCAF", document);
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
  if (writer.Write(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("STEP write failed");
  XCAFApp_Application::GetApplication()->Close(document);
}

// Reads every MANIFOLD_SOLID_BREP of a STEP file, in metres, with the name written on it.
std::unique_ptr<ShapeList> read_step(rust::Str path, rust::Vec<rust::String> &names) {
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
