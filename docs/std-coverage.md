# std coverage

The goal: every feature in Onshape's std runs on our engine and matches what Onshape builds, checked against
fixture batches Onshape generated (see `fixtures/` and `crates/fs-fixtures`). This file tracks where each feature
stands. "Matches" means every case of it in a batch builds the same parts (STEP) and the same probe lines as
Onshape.

## Features

| feature (file) | status | batch |
|---|---|---|
| `extrude` (extrude.fs) | matches: blind, symmetric, two directions; new, add, remove, intersect; surfaces from edges; thin walls; up to a face (with offset), body, vertex, the next part; through all; drafts | 001, 004 |
| `revolve` (revolve.fs) | matches: full, blind, opposite, symmetric, two directions; new, add, remove, intersect; surfaces; thin; up to a face (with offset), a part, a vertex; a second direction up to a face or a vertex (with offset) | 002, 004, 006, 007, 024 |
| `cube`, `fCuboid`, `fCylinder`, `fCone`, `fSphere`, `fEllipsoid` (primitives.fs) | matches | 002 |
| `booleanBodies` (boolean.fs) | matches: union, subtraction (keep tools, several targets), intersection; extrudes cut through sheet metal plates, flanges and bends; a subtraction with the tool's faces offset | 002, 017, 021 |
| `transform`, `copyPart` (transformCopy.fs) | matches: translate (xyz, distance, line), rotate, copy, uniform and non-uniform scale, by mate connectors | 002, 005, 007 |
| `mirror` (mirror.fs) | matches: part, face and feature mirrors (a boss), feature mirrors reapplying the feature | 002, 006, 009, 027 |
| `linearPattern` (linearPattern.fs) | matches: part pattern, one and two directions, centred, skipped instances, add; face and feature patterns (a boss); sheet metal parts patterned as parts, a cut hole's face patterned in a sheet metal plate; feature patterns reapplying the feature in each instance (a boss up to a slanted plane); face patterns of hole-feature holes and of forms in sheet metal (linear and circular) | 002, 006, 009, 017, 026, 032, 033 |
| `circularPattern` (circularPattern.fs) | matches: part pattern, equal spacing, centred, opposite, add; face and feature patterns (a boss, touching the block's edges); feature patterns reapplying the feature in each instance; face patterns of sheet metal holes and forms | 002, 006, 009, 026 |
| `splitPart` (splitpart.fs) | matches: split solids by a plane or face, keep both, front or back; splitting faces by sketch edges, a plane, and a face of another part; a block split by a curved sheet | 002, 006, 007, 018 |
| `deleteBodies` (deleteBodies.fs) | matches | 002 |
| `cPlane` (cplane.fs) | matches: offset, plane and point, line and angle, line and point, three points, mid plane, curve point, tangent to a cylinder | 002, 006 |
| `cPoint` (cpoint.fs) | matches: point on an edge | 002 |
| `assignVariable` (variable.fs) | matches: assigned lengths, numbers and angles; measured distance, length and diameter; from a table by row label and column index | 002, 020, 021 |
| `fillet` (fillet.fs) | matches: circular fillets of edges, edge sets, every edge (vertex blends), concave edges, cylinders, holes, tangent chains. Known gaps: conic cross sections (c013), asymmetric fillets (c014), fillets too big for the faces they round, which Onshape lets consume a face (c015). Variable (smooth) and partial fillets, full rounds: match. A sheet metal corner edge (a corner break, as in Onshape). Not yet: curvature, asymmetric | 003, 010, 017 |
| `chamfer` (chamfer.fs) | matches: equal offsets, two offsets (both ways), offset and angle, every edge, cylinders, concave edges, tangent chains, a sheet metal corner edge | 003, 017 |
| `shell` (shell.fs) | matches: removing faces inwards and outwards, hollow, cylinders, an L | 003 |
| `draft` (draft.fs) | matches: neutral plane drafts, both pull directions, cylinders; reference entity drafts (inside rib). Not yet: parting line drafts | 003, 006 |
| `deleteFace` (deleteFace.fs) | matches: heal, cap, leave open | 003 |
| `moveFace` (moveFace.fs) | matches: offsets; translating a face along its normal, a hole sideways, a top face sideways; rotating a planar face whose neighbours are perpendicular to it; a top face translated up to a plane and to a vertex; creating new surfaces (translated, offset) | 003, 010, 018, 021, 026 |
| `replaceFace` (replaceFace.fs) | matches: a planar face replaced by a plane, with an offset, or by a curved sheet's surface, when its neighbours are perpendicular to it; a cylinder by a coaxial cylinder. Not yet: other curved replaced faces | 003, 020, 028 |
| `thicken` (thicken.fs) | matches: faces of solids, planar and curved, one and two sides, midplane, added to a part; sheets (inside rib, thin revolve and thin loft, the offset faces meeting sharply) | 003, 006, 028 |
| `externalThread` (externalThread.fs) | matches: ISO threads on a shaft, blind with the face split, with a chamfer, with an undercut, up to the next face, with their hole attributes; an ANSI thread on two shafts at once (034) | 031, 034 |
| `hole` (hole.fs) | matches: simple, counterbored and countersunk holes; blind (118° and flat tips), through, up to the next part, up to a face, blind in the last part; several at once; simple, counterbored and countersunk holes through a sheet metal plate; from a plane; tapped and clearance holes from the standard tables, with their hole attributes; the newer (V3) hole; depth to the tip; clearance and tapped (the V3 hole's, through one part into the next) | 006, 007, 017, 018, 021, 024, 026 |
| `rib` (rib.fs) | matches: parallel and normal to the sketch plane, with and without draft, a short profile extended to the part, crossing ribs merged | 006, 007, 019, 021 |
| `modifyFillet` (modifyFillet.fs) | matches: changing the radius, removing; refilleting neighbouring fillets (a top fillet round upright ones) | 006, 028 |
| `bodyDraft` (bodyDraft.fs) | matches: by faces, top edges and parts, one side of a planar parting object, crossing the part or at its side. Not yet: both sides, matching faces at the parting object, corner treatments, curved parting objects | 006, 007 |
| `faceBlend` (faceBlend.fs) | matches: a circular rolling ball blend between two sheets meeting at an edge. Not yet: sides apart, conic and curvature sections, width control, caps, limits | 010 |
| sketch curves (sketch.fs) | matches: lines, arcs, circles, rectangles, polygons, polylines, ellipses, elliptical arcs, fit splines (open, closed, with end derivatives), Béziers, conics; the sketch's bodies and entities. Not yet: text, images, interpolated splines, constraints (accepted, not solved) | 001, 004 |
| `sweep` (sweep.fs) | matches: circles and rectangles along lines, arcs and tangent chains; surface sweeps; thin. Twisted and scaled along a line (019); along an arc keeping orientation (to 1.6e-5: Onshape's sides are approximations) and locked to a direction (021). Along a sharp L, mitred; locked to a planar face along a tangent path, and rejected along a sharp one (034). Known gap: where `evPlane` puts the origin of a swept planar face (c034). Not yet: locked to curved faces, twist and scale along curved paths | 004, 006, 019, 021, 034 |
| `loft` (loft.fs) | matches: lofts between squares and between two circles; surface lofts between arcs. Known gaps: lofts between unlike profiles or through three profiles, whose surfaces come from each kernel's own lofting (c038, c039). Thin lofts with their ends square to the walls. Not yet: guides, connections, end conditions, sections, thin lofts' ends trimmed to the profiles' planes | 004, 006, 028 |
| `helix` (helix.fs) | matches: around a cylinder by turns and pitch, both hands; about an axis; tapering | 004 |
| `fitSpline` (fitSpline.fs) | matches: through vertices, open and closed (with simple knots, as Onshape's); through tangent edges; with start and end directions, one end alone, magnitudes; matching curvature at an end (Onshape's knots and poles) and at both; closed ones are periodic; derivatives at inner points, as routing curves give them (Onshape's knots and poles, 035) | 004, 010, 011, 018, 020, 027, 030, 035 |
| `compositeCurve` (compositeCurve.fs) | matches | 005 |
| `projectCurves` (projectCurves.fs) | matches: two sketches; a curve onto a face along a direction, and along its normals | 005, 019 |
| `intersectionCurve` (faceIntersection.fs) | matches: a cylinder and a plane. Known gap: Onshape splits the closed curves where two cylinders cross into two edges each (c006) | 005 |
| `isoparametricCurve` (isoparametricCurve.fs) | matches: equally spaced both ways; at given positions round a cylinder (Onshape's u runs from the cylinder's x axis towards its y axis), through a point | 005, 018, 020 |
| `offsetCurveOnFace` (offsetCurveOnFace.fs) | matches: a straight edge across a planar face; a cylinder's top edge down its side. Known gap: some offsets Onshape finds no result for (warning `OFFSET_CURVE_ON_FACE_CHAIN_YIELDED_NO_RESULT`), where the engine makes a curve: a cylinder's top edge across its top (020 c008), and a chamfer's edge on a block's top extended or splitting (left out of batch 027); the rule isn't known. Not yet: other curved faces; extending and splitting checked against Onshape | 005, 020, 027 |
| `trimCurve` (moveCurveBoundary.fs) | matches: extending a straight curve by a distance and up to a plane, trimming it by a plane and by another straight curve; extending up to a cylinder; arcs, cubic B-splines and chains ending in them extended at their end, straight on and curving on, by a distance and up to a plane (Onshape's B-spline knots and poles), arcs trimmed by a plane (036); extended at their start, B-splines trimmed by a plane, arcs trimmed by a line, elliptical arcs extended (038). Not yet: curves other than lines trimmed by or extended up to curves other than lines, rational or other degree B-splines | 005, 018, 021, 027, 036, 038 |
| `curvePattern` (curvePattern.fs) | matches: equal and distance spacing along an arc, keeping orientation, locked to a face | 005, 018, 019, 020 |
| `offsetSurface` (offsetSurface.fs) | matches: faces of solids and sheets, both ways | 005 |
| `extendSurface` (extend.fs) | matches: planar sheets extended by a distance, all or some edges; a cylindrical sheet, carried on round its cylinder; up to a plane and a vertex, up to a part; sheets extruded from curves, past a curve's end straight on and keeping its curvature, and along the extrusion (039). Known gap: cones and spheres (039 c001–c004). Not yet: other curved sheets | 005, 018, 020, 021, 027, 039 |
| `fill` (fillSurface.fs) | matches: planar loops (also a triangle; see known gaps for its plane's origin). Not yet: non-planar fills, tangency and curvature conditions, guides | 005, 024 |
| `boundarySurface` (bsurf.fs) | Known gap: two u profiles give a surface 4e-6 off Onshape's (c019). Not yet: v profiles, end conditions | 005 |
| `ruledSurface` (ruledSurface.fs) | matches: normal to a face, aligned with a direction (also as angle 0 from it), tangent to a face, 30° from a vector; vertex overrides on a straight edge (a width, up to a point); normal to a curved face along its edge, the rulings turning with it, with width overrides when they stay in a plane (040, to Onshape's approximations). Not yet: rulings turning out of a plane along a curved edge, overrides up to faces and edges | 005, 018, 019, 024, 040 |
| `enclose` (enclose.fs) | matches: a tube closed by a slanted plane (once areas and volumes were integrated over curves projected again from the edges' 3D curves, batch 026) | 005 |
| `mutualTrim` (mutualTrim.fs) | matches: two crossing planar sheets, merged | 005 |
| `mateConnector` (mateConnector.fs) | matches: on faces (centroid), vertices, circular edges (centre), edge mid points; moved, turned, flipped, secondary axis; between two faces; realigned to two edges; a given normal (ignored, as Onshape does, off meshes) | 005, 018, 020 |
| `transform` by mate connectors | matches | 005 |
| `bridgingCurve` (bridgingCurve.fs) | matches: tangent and curvature matches between lines and an arc | 010 |
| `wrap` (wrap.fs) | matches: a planar face onto a cylinder as a surface, a solid and an imprint; onto a cone as a surface; unwrapping a cylinder's face onto a plane with `opWrap`, and a whole side refused (046). Not yet: unwrapping cones | 010, 020, 024, 046 |
| `editCurve` (editCurve.fs) | matches: raising the degree (of an arc, via its exact rational B-spline, and of a line), flattening onto a plane, approximating a line and arc with one cubic (to within its tolerance), moving control points; raising a periodic curve (known gap: Onshape keeps std's clamped result flagged periodic, which OCCT can't, 027 c005) | 011, 027 |
| `routingCurve` (routingCurve.fs) | matches: polylines with bends, interpolated splines, their tables (XYZ and LRA); points at vertices (034); points on curves, derivatives at end and inner points (035). Not yet: points from CSV | 011, 034, 035 |
| `constrainedSurface` (constrainedSurface.fs) | Known gap: a smooth surface through nine points is within tolerance of them, but isn't Onshape's surface (c006). Not yet: normals at the points, meshes, deviation | 011 |
| `decal` (decal.fs) | matches: planar and cylindrical, turned and shifted (an image given by size only); realigned to a mate connector (035; batch 011's frames checked in full there) | 011, 035 |
| `compositePart` (compositePart.fs) | matches: closed composite of two blocks; copied with its bodies by `opPattern`, and held in an open composite (035) | 006, 035 |
| `isocline` (isocline.fs) | matches: splitting a sphere, curves on a torus, a sphere and a cylinder across the direction; a cone across the direction (034). Not yet: B-spline faces, tilted tori | 007, 028, 030, 034 |
| `nameEntity` (nameEntity.fs) | matches: a face found again by name after a fillet trims it | 007 |
| `queryVariable` (queryVariable.fs) | matches: fillets of a radius and more; the selection queries all match | 007 |
| `frame` (frame.fs) | matches: square tube, flat bar and round tube profiles (from build functions, as element libraries give them) along an L, a closed rectangle and a line with a tangent arc; mitred, butted both ways, coped, corners left alone; turned and mirrored; trimmed to a plane. corner overrides (butt, coped butt flipped, none); ends limited by a part; profile locked to a planar face; aligned at a corner of the profile; a tagged profile at its extra alignment point (034). Not yet: profiles locked to curved faces | 008, 028, 030, 034 |
| `frameTrim` (frameTrim.fs) | matches: ordered groups, to a plane | 008 |
| `gusset` (gusset.fs) | matches: triangle and chamfered rectangle, centred, in a mitred corner, offset from the centre, aligned to an edge | 008, 021, 024 |
| `endcap` (endcap.fs) | matches: matching the profile, filleted and chamfered rectangles, circle, internal | 008, 019 |
| `importDerived` (importDerived.fs) | matches: at the origin, at mate connectors, chosen parts with their mate connectors (from build functions); sheet metal models kept (at the origin, at a mate connector, at the source's mate connector, then edited; one of two models), or derived as plain parts; composite parts with sheet metal, open and closed (045) | 009, 023, 045 |
| the `sectionPart` family (sectionpart.fs) | matches: `sectionPart`, `planeSectionPart`; `jogSectionPart` along a jogged line, as a partial section, and broken out inside a closed spline to a depth; `multiplePlaneSectionPart` with two planes; crop views, offset and aligned sections (037); the assembly forms, `sectionTransformedParts` and `multiplePlanesSectionTransformedParts`, with parts at several instances (041) | 009, 025, 037, 041 |
| `cutlist` (cutlist.fs) | matches: the two tubes of a mitred L, grouped and tabled; a column overridden for all frames, and for one frame (034); a frame's tangent segments merged, listed apart or, given the closed composite, as one item (035) | 011, 030, 034, 035 |
| `tag` (tag.fs) | matches: a form's positive part; a frame profile with an alignment point (034); plated PCB holes, standard content (038) | 011, 034, 038 |
| `derivedMirror`, `importForeign` | not started (`derivedMirror` fails in a Part Studio feature; `importForeign` needs uploaded data) | later |
| `sheetMetalRecognize` (sheetMetalStart.fs) | matches: an extruded L bracket recognized as sheet metal with bends; an L with its bend already rounded, a rolled wall (half a tube), an L of two thicknesses (042). Not yet: rips, walls on other curved surfaces | 031, 042 |
| `sheetMetalStart` (sheetMetalStart.fs) | matches: thickening sketch regions (both ways), extruding sketch lines into an L and a U (with bends), converting a block into an open box (bends along the bottom, rips up the corners) | 012 |
| `sheetMetalFlange` (sheetMetalFlange.fs) | matches: 90° and 45° flanges (inner, outer, middle and hold-line alignment, default and own radius, the other way), up to a plane, flanges on two edges meeting at a corner (simple corner, edge-to-edge rip), on an edge of a converted box (whose rips leave the wall short of the edge's ends). Mitred flanges at 30° fail in Onshape and the engine alike. Partial flanges, with their bend reliefs (obround, rectangular, tear; scaled). Corner reliefs where two flanges meet: round and rectangular (scaled), sized round and square, closed; their areas and volumes within 0.01 (2e-6 of them), as Onshape's relieved bends are B-spline faces | 012, 013, 014, 015, 016 |
| `sheetMetalBend` (sheetMetalBend.fs) | matches: a plate bent along a line, 90° up and 60° down holding the other side | 013 |
| `sheetMetalJoint` (sheetMetalJoint.fs) | matches: a bend made a rip (the model falls into two parts), a bend given another radius | 013 |
| `sheetMetalTab` (sheetMetalTab.fs) | matches: a tab merged into a plate's edge | 013 |
| `sheetMetalEnd` (sheetMetalEnd.fs) | matches: the parts become ordinary parts | 013 |
| `sheetMetalJog` (sheetMetalJog.fs) | matches: a 5 mm jog anchored outside, whose bends std turns to 75.5° to meet with no wall between | 013 |
| `sheetMetalCorner` (sheetMetalCorner.fs) | matches: a round relief where two flanges meet, cut in the flat pattern and folded back through the plate, the bends and the flanges (area and volume within 0.01) | 013 |
| `sheetMetalHem` (sheetMetalHem.fs) | matches: straight hems (with a radius, closed to the minimal gap, aligned in place or outside), a rolled hem. tear drops (closed to the minimal gap or with a tip gap, in place or outside; their profile from a constrained sketch the engine solves). Known gap: hems meeting at a closed corner, where Onshape leaves the plate whole and the engine cuts its corner (021 c012); the engine's STEP of a tear drop hem reads back 2e-6 off (029) | 014, 021, 029 |
| `sheetMetalRip` (sheetMetalRip.fs) | matches: a plate ripped corner to corner (Onshape leaves it whole) | 014 |
| `sheetMetalCornerBreak` (sheetMetalCornerBreak.fs) | matches: plate corners rounded and chamfered (also with two offsets), a flange's corner rounded, a conic (within 2.4e-6: Onshape's is an approximation) | 015, 020 |
| `sheetMetalUnfold`, `sheetMetalRefold` (sheetMetalUnfold.fs) | matches: an L's bend unfolded holding a wall, and folded again; rolled walls unrolled (holding an edge), a U's bends all at once (043). Not yet: rolled walls with walls beyond them, refolding rolled walls | 015, 043 |
| `sheetMetalMakeJoint` (sheetMetalMakeJoint.fs) | matches: two walls' free edges joined by a bend, and by a butt rip | 017 |
| `sheetMetalBendRelief` (sheetMetalBendRelief.fs) | matches: a bend end given a sized rectangular or obround relief, a tear, an extended obround (no change from the plain one there) | 017, 019 |
| `sheetMetalLoft` (sheetMetalLoft.fs) | matches: a wall between two parallel lines. Known gaps (044): where a bend meets the profiles' planes at a slant (L profiles, a skew line split along its shorter diagonal), Onshape ends it with B-spline faces (within 1.3e-3 of the volume). Not yet: curved or unlike profiles, connections, adding to a model (044 holds Onshape's results) | 017, 044 |
| `sheetMetalGeometryPattern` (sheetMetalPattern.fs) | matches, through linear patterns: a part pattern, a face pattern of a hole | 017 |
| `pcbStart`, `pcbEnd` (pcbStart.fs, pcbEnd.fs) | matches: a flex PCB plate with a flange, finished without unfolding. Not yet: unfolding, the bend table | 017 |
| `sheetMetalFormed` (sheetMetalFormed.fs) | matches: a boss and a cup (a part to add and one to remove) formed on a plate from form Part Studios given as build functions, at one and two places, flipped | 022 |
| `sheetMetalTransform` | matches: moving derived sheet metal models (a quarter turn, a tilt, to a base mate connector) through `importDerived`. Not yet: through `derivedMirror` | 023 |
| tables (`cutlistTable`, `routingCurveTable`) | match, called from a feature | 011 |
| `holeTable`, computed properties (`computeMass`) | Onshape won't run them inside a feature (they read properties, or start their own feature), and neither does the engine: both refuse with "Execution error" as Onshape does (c009, c014). Their output outside a feature isn't checked | 011 |

## Builtins

`tools/check-std.mjs` and the `missing builtins` line of a failed generator list builtins std calls that the
engine doesn't have. After batch 002, std calls 269 builtins; these are implemented or answered:

- language and maths: all.
- context and features: contexts, feature start/end/abort, statuses, computed parameters, `getLastActiveId`.
- variables: `setVariable`, `getVariable`, `setQueryVariable`, `getQueryVariable`, `getAllVariables`,
  `getAllVariablesAndDescriptions`.
- sketches: `newSketch`, `skPoint`, `skLineSegment`, `skCircle`, `skArc`, `skSolve`; `skConstraint` constraints are
  solved where the entities don't already meet them (fix, coincident, tangent, perpendicular, parallel,
  horizontal, vertical, distance; batch 029); others are ignored.
- evaluations added: `evOffsetDetection` (planar walls; batch 031).
- operations: `opPoint`, `opPlane`, `opExtrude`, `opRevolve`, `opSphere`, `opBoolean`, `opTransform`, `opPattern`
  (bodies and faces), `opSplitPart`, `opSplitFace`, `opDeleteBodies`, `opFillet` (circular), `opChamfer`,
  `opShell`, `opDraft` (reference surface and entity), `opBodyDraft`, `opThicken`, `opOffsetFace`, `opDeleteFace`,
  `opMoveFace`, `opReplaceFace`, `opModifyFillet`, `opHole`, `opCreateCompositePart`, `opNameEntity`,
  `opCreateIsocline`, `opSplitByIsocline`, `opMergeContexts`, `opEditCurve`, `opPolyline`,
  `opConstrainedSurface`, `updateSheetMetalGeometry` (planar walls, straight bends, edge-to-edge rips, simple
  corners), `opEdgeChange` (moving straight free edges of planar faces) (the cases above), and the
  curve and surface operations of batches 004 and 005.
- attributes: `setAttribute`, `getAttributes`, `getAttribute`, `getAllAttributes`, `removeAttributes`; they follow
  their entities through operations.
- evaluation: `evaluateQuery` (and counts), `evPlane`, `evOwnerSketchPlane`, `evVertexPoint`, `evArea`, `evLength`,
  `evVolume`, `evBox` (with `cSys`), `evAxis`, `evLine`, `evCurveDefinition` (lines, circles, ellipses,
  B-splines), `evApproximateBSplineCurve`, `evMaxPathDeviation`, `approximateSpline` (least squares with knots
  added where the fit is worst; not periodic yet), `clusterBodies`, `getHoleAttributes`,
  `evEdgeTangentLines` (with a face's orientation), `evFaceTangentPlanes`, `evFaceTangentPlanesAtEdge`, `evDistance`
  (minimum; extended sides), `evCollisionDetection`, `evEdgeConvexity`, `evSurfaceDefinition` (with Onshape's
  coordinate systems for extruded and revolved cylinders and cones).
- patterns: `computeLinearPatternTransforms`, `computeCircularPatternTransforms`, `setPatternData`.
- properties: `setProperty`, `getProperty`.
- not modelled yet, answered as if absent: feature patterns, flat sheet metal; Onshape's history bookkeeping
  (`convert`, `recordQuery`, `clampContextVersion`, the disambiguation builtins) does nothing.

## Query types

Handled: `NOTHING`, `EVERYTHING`, `TRANSIENT`, `CREATED_BY`, `SKETCH_REGION`, `SKETCH_ENTITY`, `IMPRINT`, `UNION`,
`INTERSECTION`, `SUBTRACTION`, `ENTITY_FILTER`, `NTH_ELEMENT`, `BODY_TYPE`, `CONSTRUCTION_FILTER`,
`SKETCH_OBJECT_FILTER`, `OWNED_BY_PART`, `OWNER_PART`, `CAP_ENTITY`, `GEOMETRY`, `CONTAINS_POINT`, `VERTEX_ADJACENT`,
`EDGE_ADJACENT`, `EDGE_VERTEX`, `TANGENT_CONNECTED_EDGES`, `TANGENT_CONNECTED_FACES`, `CLOSEST_TO`, `FARTHEST_ALONG`,
`LARGEST`, `SMALLEST`, `TRACKING` (entities record what they were made from), `ATTRIBUTE_FILTER`, `OP_HOLE_FACE`,
`OP_HOLE_PROFILE`, `EDGE_TOPOLOGY_FILTER`, `SPLIT` (split faces' sides), `CONTAINED_IN_COMPOSITE`,
`COMPOSITE_CONTAINING`, `CONSUMED`, `COMPOSITE_PART_TYPE_FITLER`, `NAMED`, `MATE_CONNECTOR`, `PLANE_PARALLEL_DIRECTION`, `FACE_PARALLEL_DIRECTION`,
`INTERSECTS_PLANE`, the selection queries (`CONVEX_CONNECTED_FACES`, `CONCAVE_CONNECTED_FACES`,
`HOLE_FACES`, `FILLET_FACES`, `LOOP_EDGES`, `PARALLEL_EDGES`, `LOOP_BOUNDED_FACES`, `FACE_OR_EDGE_BOUNDED_FACES`,
`PATTERN`, `PLANE_NORMAL`, `IN_FRONT_OF_PLANE`, `EDGE_CONVEXITY_FILTER`), and the sheet metal, mesh and modifiable
filters (as if there were none).

## Known gaps

What fixtures show Onshape doing that the engine can't yet:

- **Conic fillets** (batch 003, c013). Onshape's result matches a rational quadratic cross section with the
  circular fillet's tangent points and weight `rho / (1 - rho)` to 5e-4 of the cross-section area, which is
  within the probes' tolerance; OCCT has no conic fillets, so this needs building by hand.
- **Asymmetric fillets** (c014). The cross section removes 2.14379 mm² for setbacks of 2 and 5 mm, close to but
  not a quarter ellipse (2.14602 mm²); the exact curve isn't known yet.
- **Cones and spheres extended** (batch 039 c001–c004). Onshape carries the surface on, but the distance isn't
  arc length at an obvious radius: a quarter cone (radii 10 to 5) extended 5 mm round turns 0.666633 rad more
  (5 mm of arc at radius 7.50037, not quite the edge's middle radius 7.5); a band of a sphere (radius 10, 0° to
  60°) turns 0.621337 rad (5 mm at radius 8.047); extended past its top it reaches latitude 88.654° (5.0011 mm of
  arc). The rule isn't known yet.
- **Where `evPlane` puts a planar face's origin** (batch 004 c034; batch 024 c008–c010; batch 037 c004, in the
  axes of a rotated copy's plane, not the canonical ones). Batch 040 shows the rule: the middle of the face's box
  in its plane surface's own axes (a ruled sheet's x runs along its middle ruling; the engine records that frame
  and matches). Where the engine doesn't know the surface's frame it uses the canonical axes, as below. Near the middle of the
  face's box in the plane's axes, which the engine uses, but 0.3% of the box's size off it along each axis, one
  way or the other: a right triangle 30 × 12 mm gives (15.09, 6.036), not the box centre (15, 6) or the centroid
  (10, 4); the annular sector a line swept along an arc makes, (16.239, 12.045) against the box centre
  (16.142, 12.142). For faces symmetric in their box the two agree.
- **Lofts between unlike profiles** (c038, square to circle: 3% of the volume) **and through three profiles**
  (c039: 3e-4; Onshape splits the side into four patches): the surfaces come from each kernel's own lofting.
- **Coordinate systems of fillets and holes cut through all** (batch 007, c007): a 4 mm fillet on the vertical
  edge of a block has its origin at the edge's top end with x pointing away from the corner; one case isn't enough
  for the rule. A cylinder cut through all has its origin at the far end of Onshape's own through-all depth
  (2108.986 mm from the profile in that case), which the engine doesn't reproduce.
- **Relieved bends** (batch 013 c006; batch 016 c007–c011, c013). Corner reliefs and closed corners leave Onshape's
  bends B-spline faces; the engine wraps its flat bend onto the cylinder, which gives areas and volumes within
  0.01 mm² and mm³ (2e-6 of them). Those cases are compared to 0.01.
- **Constrained surfaces** (batch 011, c006). Through nine points on a 40 × 30 mm grid with the middle one 4 mm
  up, Onshape's surface reaches 0.05 mm (in x) and 0.09 mm (in y) past the outer points, dips to −0.015 mm and
  has an area of 1242.61 mm²; the engine's least-squares bicubic over the points' rectangle has 1255.91 mm². Both
  are within the 0.01 mm tolerance of every point.
- **Fillets too big for their faces** (c015). A 15 mm fillet on the four vertical edges of a 30 × 20 mm block:
  Onshape lets the two fillets on each 20 mm side consume it, leaving the lens where the cylinders meet (4 faces,
  6 edges, 4 vertices); OCCT fails.
