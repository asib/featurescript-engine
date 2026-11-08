# std coverage

The goal: every feature in Onshape's std runs on our engine and matches what Onshape builds, checked against
fixture batches Onshape generated (see `fixtures/` and `crates/fs-fixtures`). This file tracks where each feature
stands. "Matches" means every case of it in a batch builds the same parts (STEP) and the same probe lines as
Onshape.

## Features

| feature (file) | status | batch |
|---|---|---|
| `extrude` (extrude.fs) | matches: blind, symmetric, two directions; new, add, remove, intersect; surfaces from edges; thin walls; up to a face (with offset), body, vertex, the next part; through all; drafts | 001, 004 |
| `revolve` (revolve.fs) | matches: full, blind, opposite, symmetric, two directions; new, add, remove, intersect; surfaces; thin; up to a face (with offset), a part, a vertex. Not yet: up-to bounds for the start | 002, 004, 006, 007 |
| `cube`, `fCuboid`, `fCylinder`, `fCone`, `fSphere`, `fEllipsoid` (primitives.fs) | matches | 002 |
| `booleanBodies` (boolean.fs) | matches: union, subtraction (keep tools, several targets), intersection. Not yet: offset, sheet metal | 002 |
| `transform`, `copyPart` (transformCopy.fs) | matches: translate (xyz, distance, line), rotate, copy, uniform and non-uniform scale, by mate connectors | 002, 005, 007 |
| `mirror` (mirror.fs) | matches: part, face and feature mirrors (a boss) | 002, 006, 009 |
| `linearPattern` (linearPattern.fs) | matches: part pattern, one and two directions, centred, skipped instances, add; face and feature patterns (a boss). Not yet: reapplying features in each instance | 002, 006, 009 |
| `circularPattern` (circularPattern.fs) | matches: part pattern, equal spacing, centred, opposite, add; face and feature patterns (a boss, touching the block's edges). Not yet: reapplying features in each instance | 002, 006, 009 |
| `splitPart` (splitpart.fs) | matches: split solids by a plane or face, keep both, front or back; splitting faces by sketch edges, a plane, and a face of another part. Not yet: sheet tools | 002, 006, 007 |
| `deleteBodies` (deleteBodies.fs) | matches | 002 |
| `cPlane` (cplane.fs) | matches: offset, plane and point, line and angle, line and point, three points, mid plane, curve point, tangent to a cylinder | 002, 006 |
| `cPoint` (cpoint.fs) | matches: point on an edge | 002 |
| `assignVariable` (variable.fs) | matches: assigned lengths, numbers and angles; measured distance and length. Not yet: table and diameter modes | 002 |
| `fillet` (fillet.fs) | matches: circular fillets of edges, edge sets, every edge (vertex blends), concave edges, cylinders, holes, tangent chains. Known gaps: conic cross sections (c013), asymmetric fillets (c014), fillets too big for the faces they round, which Onshape lets consume a face (c015). Variable (smooth) and partial fillets, full rounds: match. Not yet: curvature, asymmetric | 003, 010 |
| `chamfer` (chamfer.fs) | matches: equal offsets, two offsets (both ways), offset and angle, every edge, cylinders, concave edges, tangent chains | 003 |
| `shell` (shell.fs) | matches: removing faces inwards and outwards, hollow, cylinders, an L | 003 |
| `draft` (draft.fs) | matches: neutral plane drafts, both pull directions, cylinders; reference entity drafts (inside rib). Not yet: parting line drafts | 003, 006 |
| `deleteFace` (deleteFace.fs) | matches: heal, cap, leave open | 003 |
| `moveFace` (moveFace.fs) | matches: offsets; translating a face along its normal, a hole sideways, a top face sideways; rotating a planar face whose neighbours are perpendicular to it. Not yet: up to an entity, creating new faces | 003, 010 |
| `replaceFace` (replaceFace.fs) | matches: a planar face replaced by a plane, with an offset, when its neighbours are perpendicular to it. Not yet: other faces | 003 |
| `thicken` (thicken.fs) | matches: faces of solids, planar and curved, one and two sides, midplane, added to a part; sheets (inside rib and thin revolve) | 003, 006 |
| `hole` (hole.fs) | matches: simple, counterbored and countersunk holes; blind (118° and flat tips), through, up to the next part, up to a face, blind in the last part; several at once. Not yet: tapped holes and clearance, offsets from the tip, starting from a sketch or plane, the newer hole (isV2) | 006, 007 |
| `rib` (rib.fs) | matches: parallel and normal to the sketch plane, with and without draft. Not yet: extending to the part, merging ribs | 006, 007 |
| `modifyFillet` (modifyFillet.fs) | matches: changing the radius, removing. Not yet: refilleting neighbours | 006 |
| `bodyDraft` (bodyDraft.fs) | matches: by faces, top edges and parts, one side of a planar parting object, crossing the part or at its side. Not yet: both sides, matching faces at the parting object, corner treatments, curved parting objects | 006, 007 |
| `faceBlend` (faceBlend.fs) | matches: a circular rolling ball blend between two sheets meeting at an edge. Not yet: sides apart, conic and curvature sections, width control, caps, limits | 010 |
| sketch curves (sketch.fs) | matches: lines, arcs, circles, rectangles, polygons, polylines, ellipses, elliptical arcs, fit splines (open, closed, with end derivatives), Béziers, conics; the sketch's bodies and entities. Not yet: text, images, interpolated splines, constraints (accepted, not solved) | 001, 004 |
| `sweep` (sweep.fs) | matches: circles and rectangles along lines, arcs and tangent chains; surface sweeps; thin. Known gap: where `evPlane` puts the origin of a swept planar face (c034). Not yet: profile control, twist, scale | 004, 006 |
| `loft` (loft.fs) | matches: lofts between squares and between two circles; surface lofts between arcs. Known gaps: lofts between unlike profiles or through three profiles, whose surfaces come from each kernel's own lofting (c038, c039). Not yet: guides, connections, end conditions, sections, thin | 004, 006 |
| `helix` (helix.fs) | matches: around a cylinder by turns and pitch, both hands; about an axis; tapering | 004 |
| `fitSpline` (fitSpline.fs) | matches: through vertices, open and closed (with simple knots, as Onshape's); through tangent edges. Not yet: end directions, approximation | 004, 010, 011 |
| `compositeCurve` (compositeCurve.fs) | matches | 005 |
| `projectCurves` (projectCurves.fs) | matches: two sketches; a curve onto a face along a direction. Not yet: along the face's normals | 005 |
| `intersectionCurve` (faceIntersection.fs) | matches: a cylinder and a plane. Known gap: Onshape splits the closed curves where two cylinders cross into two edges each (c006) | 005 |
| `isoparametricCurve` (isoparametricCurve.fs) | matches: equally spaced both ways. Not yet: curves at given parameters or points | 005 |
| `offsetCurveOnFace` (offsetCurveOnFace.fs) | matches: a straight edge across a planar face. Not yet: curved edges and faces, extending, splitting | 005 |
| `trimCurve` (moveCurveBoundary.fs) | matches: extending a straight curve by a distance. Not yet: trimming, extending up to an entity, curved extensions | 005 |
| `curvePattern` (curvePattern.fs) | matches: equal spacing along an arc. Not yet: distance spacing, locked faces, keeping orientation | 005 |
| `offsetSurface` (offsetSurface.fs) | matches: faces of solids and sheets, both ways | 005 |
| `extendSurface` (extend.fs) | matches: planar sheets extended by a distance, all or some edges. Not yet: curved sheets, up-to bounds | 005 |
| `fill` (fillSurface.fs) | matches: planar loops. Not yet: non-planar fills, tangency and curvature conditions, guides | 005 |
| `boundarySurface` (bsurf.fs) | Known gap: two u profiles give a surface 4e-6 off Onshape's (c019). Not yet: v profiles, end conditions | 005 |
| `ruledSurface` (ruledSurface.fs) | matches: normal to a face, aligned with a direction. Not yet: tangent, angles, vertex overrides | 005 |
| `enclose` (enclose.fs) | Known gap: a tube closed by a slanted plane is 3e-6 off in area, because OCCT's curve where the plane cuts the cylinder is an approximation (c022) | 005 |
| `mutualTrim` (mutualTrim.fs) | matches: two crossing planar sheets, merged | 005 |
| `mateConnector` (mateConnector.fs) | matches: on faces (centroid), vertices, circular edges (centre), edge mid points; moved, turned, flipped, secondary axis. Not yet: between entities, realigning, specifying the normal | 005 |
| `transform` by mate connectors | matches | 005 |
| `bridgingCurve` (bridgingCurve.fs) | matches: tangent and curvature matches between lines and an arc | 010 |
| `wrap` (wrap.fs) | matches: a planar face onto a cylinder as a surface. Not yet: cones, solids, imprinting, unwrapping | 010 |
| `editCurve` (editCurve.fs) | matches: raising the degree (of an arc, via its exact rational B-spline, and of a line), flattening onto a plane, approximating a line and arc with one cubic (to within its tolerance), moving control points. Not yet: periodic curves | 011 |
| `routingCurve` (routingCurve.fs) | matches: polylines with bends, interpolated splines, their tables (XYZ and LRA). Not yet: points from vertices, curves or CSV, derivatives | 011 |
| `constrainedSurface` (constrainedSurface.fs) | Known gap: a smooth surface through nine points is within tolerance of them, but isn't Onshape's surface (c006). Not yet: normals at the points, meshes, deviation | 011 |
| `decal` (decal.fs) | matches: planar and cylindrical, turned and shifted (an image given by size only). Not yet: realigning to a mate connector | 011 |
| `compositePart` (compositePart.fs) | matches: closed composite of two blocks | 006 |
| `isocline` (isocline.fs) | matches: splitting a sphere, curves on a torus. Not yet: other faces, tilted tori | 007 |
| `nameEntity` (nameEntity.fs) | matches: a face found again by name after a fillet trims it | 007 |
| `queryVariable` (queryVariable.fs) | matches: fillets of a radius and more; the selection queries all match | 007 |
| `frame` (frame.fs) | matches: square tube, flat bar and round tube profiles (from build functions, as element libraries give them) along an L, a closed rectangle and a line with a tangent arc; mitred, butted both ways, coped, corners left alone; turned and mirrored; trimmed to a plane. Not yet: corner overrides, locked profile faces, alignment points, trimming to parts | 008 |
| `frameTrim` (frameTrim.fs) | matches: ordered groups, to a plane | 008 |
| `gusset` (gusset.fs) | matches: triangle and chamfered rectangle, centred, in a mitred corner. Not yet: aligned, offset | 008 |
| `endcap` (endcap.fs) | matches: matching the profile, filleted rectangle. Not yet: circle, internal, chamfered corners | 008 |
| `importDerived` (importDerived.fs) | matches: at the origin, at mate connectors, chosen parts with their mate connectors (from build functions). Not yet: sheet metal | 009 |
| the `sectionPart` family (sectionpart.fs) | matches: `sectionPart`, `planeSectionPart`. Not yet: several planes, jogged sections | 009 |
| `cutlist` (cutlist.fs) | matches: the two tubes of a mitred L, grouped and tabled. Not yet: overrides, composite segments | 011 |
| `tag` (tag.fs) | matches: a form's positive part. Not yet: frame profiles, PCB holes, standard content | 011 |
| `derivedMirror`, `importForeign` | not started (`derivedMirror` fails in a Part Studio feature; `importForeign` needs uploaded data) | later |
| `sheetMetalStart` (sheetMetalStart.fs) | matches: thickening sketch regions (both ways), extruding sketch lines into an L and a U (with bends), converting a block into an open box (bends along the bottom, rips up the corners) | 012 |
| `sheetMetalFlange` (sheetMetalFlange.fs) | matches: 90° and 45° flanges (inner alignment, default and own radius, the other way), flanges on two edges meeting at a corner (simple corner, edge-to-edge rip). Not yet: other alignments, up to an entity, miters, partial flanges | 012 |
| sheet metal: `sheetMetalHem`, `sheetMetalTab`, `sheetMetalJoint`, `sheetMetalMakeJoint`, `sheetMetalCorner`, `sheetMetalCornerBreak`, `sheetMetalBendRelief`, `sheetMetalRip`, `sheetMetalJog`, `sheetMetalBend`, `sheetMetalUnfold`/`Refold`, `sheetMetalFormed`, `sheetMetalLoft`, `sheetMetalGeometryPattern`, `sheetMetalTransform`, `sheetMetalEnd`, `pcbStart`/`pcbEnd` | not started. The folded part is built from the model for planar walls, straight bends, edge-to-edge rips and simple corners; not yet: rolled walls, butt rips, corner and bend reliefs, the flat pattern | later |
| tables (`cutlistTable`, `routingCurveTable`) | match, called from a feature | 011 |
| `holeTable`, computed properties (`computeMass`) | Onshape won't run them inside a feature (they read properties, or start their own feature), and neither does the engine: both refuse with "Execution error" as Onshape does (c009, c014). Their output outside a feature isn't checked | 011 |

## Builtins

`tools/check-std.mjs` and the `missing builtins` line of a failed generator list builtins std calls that the
engine doesn't have. After batch 002, std calls 269 builtins; these are implemented or answered:

- language and maths: all.
- context and features: contexts, feature start/end/abort, statuses, computed parameters, `getLastActiveId`.
- variables: `setVariable`, `getVariable`, `setQueryVariable`, `getQueryVariable`, `getAllVariables`,
  `getAllVariablesAndDescriptions`.
- sketches: `newSketch`, `skPoint`, `skLineSegment`, `skCircle`, `skArc`, `skSolve`; `skConstraint` is accepted and
  ignored.
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
- **Where `evPlane` puts a planar face's origin** (c034): the centroid for faces with straight edges, but not for
  the annular sector a line swept along an arc makes: Onshape gives (16.239, 12.045), the centroid is
  (18.127, 10.157) and the box centre (16.142, 12.142).
- **Lofts between unlike profiles** (c038, square to circle: 3% of the volume) **and through three profiles**
  (c039: 3e-4; Onshape splits the side into four patches): the surfaces come from each kernel's own lofting.
- **Coordinate systems of fillets and holes cut through all** (batch 007, c007): a 4 mm fillet on the vertical
  edge of a block has its origin at the edge's top end with x pointing away from the corner; one case isn't enough
  for the rule. A cylinder cut through all has its origin at the far end of Onshape's own through-all depth
  (2108.986 mm from the profile in that case), which the engine doesn't reproduce.
- **Constrained surfaces** (batch 011, c006). Through nine points on a 40 × 30 mm grid with the middle one 4 mm
  up, Onshape's surface reaches 0.05 mm (in x) and 0.09 mm (in y) past the outer points, dips to −0.015 mm and
  has an area of 1242.61 mm²; the engine's least-squares bicubic over the points' rectangle has 1255.91 mm². Both
  are within the 0.01 mm tolerance of every point.
- **Fillets too big for their faces** (c015). A 15 mm fillet on the four vertical edges of a 30 × 20 mm block:
  Onshape lets the two fillets on each 20 mm side consume it, leaving the lens where the cylinders meet (4 faces,
  6 edges, 4 vertices); OCCT fails.
