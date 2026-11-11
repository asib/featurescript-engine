// Helpers shared by the C++ files of the bridge. Include after "occt/src/lib.rs.h".

#pragma once

#include <BRepBuilderAPI_MakeShape.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <TopTools_ListOfShape.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include <memory>
#include <stdexcept>
#include <string>

namespace occt {
inline namespace detail {

// OCCT prints transfer statistics to stdout; a library shouldn't.
inline void quiet() {
  static const bool done = [] {
    Message::DefaultMessenger()->RemovePrinters(STANDARD_TYPE(Message_PrinterOStream));
    return true;
  }();
  (void)done;
}

inline gp_Pnt pnt(const Vec3 &v) { return gp_Pnt(v.x, v.y, v.z); }
inline gp_Dir dir(const Vec3 &v) { return gp_Dir(v.x, v.y, v.z); }
inline Vec3 vec3(const gp_XYZ &p) { return Vec3{p.X(), p.Y(), p.Z()}; }
inline gp_Ax3 ax3(const Frame &f) { return gp_Ax3(pnt(f.origin), dir(f.normal), dir(f.x)); }
inline gp_Ax2 ax2(const Frame &f) { return gp_Ax2(pnt(f.origin), dir(f.normal), dir(f.x)); }

inline std::unique_ptr<Shape> wrap(const TopoDS_Shape &s) { return std::make_unique<Shape>(s); }

inline std::unique_ptr<ShapeList> wrap_list(const TopTools_ListOfShape &l) {
  auto out = std::make_unique<ShapeList>();
  for (const TopoDS_Shape &s : l) out->shapes.push_back(s);
  return out;
}

inline TopTools_ListOfShape to_list(const ShapeList &l) {
  TopTools_ListOfShape out;
  for (const TopoDS_Shape &s : l.shapes) out.Append(s);
  return out;
}

inline TopAbs_ShapeEnum shape_enum(int32_t kind) {
  if (kind < TopAbs_COMPOUND || kind > TopAbs_SHAPE) throw std::invalid_argument("bad shape kind");
  return static_cast<TopAbs_ShapeEnum>(kind);
}

inline void check_done(const BRepBuilderAPI_MakeShape &maker, const char *what) {
  if (!maker.IsDone()) throw std::runtime_error(std::string(what) + " failed");
}

}  // namespace detail
TopoDS_Shape edges_fused(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history);
}  // namespace occt
