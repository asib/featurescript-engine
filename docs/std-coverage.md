# std coverage

The goal: every feature in Onshape's std runs on our engine and matches what Onshape builds, checked against
fixture batches Onshape generated (see `fixtures/` and `crates/fs-fixtures`). This file tracks where each feature
stands. "Matches" means every case of it in a batch builds the same parts (STEP) and the same probe lines as
Onshape.

## Features

| feature (file) | status | batch |
|---|---|---|
| `extrude` (extrude.fs) | matches: blind, symmetric, two directions; new, add, remove, intersect; surfaces from edges; thin walls; up to a face (with offset), body, vertex, the next part; through all; drafts | 001, 004 |
| `revolve` (revolve.fs) | matches: full, blind, opposite, symmetric, two directions; new, add, remove, intersect; surfaces; thin; up to a face. Not yet: up-to bounds for the start, up to a body or vertex (written, not checked) | 002, 004, 006 |
| `cube`, `fCuboid`, `fCylinder`, `fCone`, `fSphere`, `fEllipsoid` (primitives.fs) | matches | 002 |
| `booleanBodies` (boolean.fs) | matches: union, subtraction (keep tools, several targets), intersection. Not yet: offset, sheet metal | 002 |
| `transform`, `copyPart` (transformCopy.fs) | matches: translate (xyz, distance, line), rotate, copy, uniform scale. Not yet: mate connectors, non-uniform scale | 002 |
| `mirror` (mirror.fs) | matches: part mirror, face mirror (a boss). Not yet: feature mirror | 002, 006 |
| `linearPattern` (linearPattern.fs) | matches: part pattern, one and two directions, centred, skipped instances, add; face pattern (a boss). Not yet: feature patterns | 002, 006 |
| `circularPattern` (circularPattern.fs) | matches: part pattern, equal spacing, centred, opposite, add; face pattern (a boss, touching the block's edges). Not yet: feature patterns | 002, 006 |
| `splitPart` (splitpart.fs) | matches: split solids by a plane or face, keep both, front or back; splitting a face by sketch edges. Not yet: sheet tools, splitting faces by faces and planes (written, not checked) | 002, 006 |
| `deleteBodies` (deleteBodies.fs) | matches | 002 |
| `cPlane` (cplane.fs) | matches: offset, plane and point, line and angle, line and point, three points, mid plane, curve point. Known gap: tangent planes are placed from the cylinder's surface origin, which Onshape puts at the far end of an extruded cylinder (c016 of batch 006) | 002, 006 |
| `cPoint` (cpoint.fs) | matches: point on an edge | 002 |
| `assignVariable` (variable.fs) | matches: assigned lengths, numbers and angles; measured distance and length. Not yet: table and diameter modes | 002 |
| `fillet` (fillet.fs) | matches: circular fillets of edges, edge sets, every edge (vertex blends), concave edges, cylinders, holes, tangent chains. Known gaps: conic cross sections (c013), asymmetric fillets (c014), fillets too big for the faces they round, which Onshape lets consume a face (c015). Not yet: variable, partial, curvature | 003 |
| `chamfer` (chamfer.fs) | matches: equal offsets, two offsets (both ways), offset and angle, every edge, cylinders, concave edges, tangent chains | 003 |
| `shell` (shell.fs) | matches: removing faces inwards and outwards, hollow, cylinders, an L | 003 |
| `draft` (draft.fs) | matches: neutral plane drafts, both pull directions, cylinders; reference entity drafts (inside rib). Not yet: parting line drafts | 003, 006 |
| `deleteFace` (deleteFace.fs) | matches: heal, cap, leave open | 003 |
| `moveFace` (moveFace.fs) | matches: offsets; translating a face along its normal, a hole sideways; rotating a planar face whose neighbours are perpendicular to it. Not yet: other translations and rotations, up to an entity, creating new faces | 003 |
| `replaceFace` (replaceFace.fs) | matches: a planar face replaced by a plane, with an offset, when its neighbours are perpendicular to it. Not yet: other faces | 003 |
| `thicken` (thicken.fs) | matches: faces of solids, planar and curved, one and two sides, midplane, added to a part; sheets (inside rib and thin revolve) | 003, 006 |
| `hole` (hole.fs) | matches: simple, counterbored and countersunk holes; blind (118° and flat tips) and through; several at once. Not yet: up to next or an entity, blind in last, tapped holes and clearance, starting from a sketch or plane, the newer hole (isV2) | 006 |
| `rib` (rib.fs) | matches: parallel to the sketch plane, with and without draft. Not yet: normal to the sketch plane, extending to the part, merging ribs | 006 |
| `modifyFillet` (modifyFillet.fs) | matches: changing the radius, removing. Not yet: refilleting neighbours | 006 |
| `bodyDraft` (bodyDraft.fs) | matches: faces, one side of the parting object. Not yet: both sides, matching faces at the parting object, corner treatments | 006 |
| `faceBlend` | not started | later |
| sketch curves (sketch.fs) | matches: lines, arcs, circles, rectangles, polygons, polylines, ellipses, elliptical arcs, fit splines (open, closed, with end derivatives), Béziers, conics; the sketch's bodies and entities. Not yet: text, images, interpolated splines, constraints (accepted, not solved) | 001, 004 |
| `sweep` (sweep.fs) | matches: circles and rectangles along lines, arcs and tangent chains; surface sweeps; thin. Known gap: where `evPlane` puts the origin of a swept planar face (c034). Not yet: profile control, twist, scale | 004, 006 |
| `loft` (loft.fs) | matches: lofts between squares and between two circles; surface lofts between arcs. Known gaps: lofts between unlike profiles or through three profiles, whose surfaces come from each kernel's own lofting (c038, c039). Not yet: guides, connections, end conditions, sections, thin | 004, 006 |
| `helix` (helix.fs) | matches: around a cylinder by turns and pitch, both hands; about an axis; tapering | 004 |
| `fitSpline` (fitSpline.fs) | matches: through vertices, open and closed. Not yet: through edges, end directions | 004 |
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
| `bridgingCurve`, `editCurve`, `routingCurve`, `constrainedSurface`, `isocline`, `wrap`, `decal` | not started | later |
| `compositePart` (compositePart.fs) | matches: closed composite of two blocks | 006 |
| `frame`, `frameTrim`, `gusset`, `endcap`, `cutlist`, the `sectionPart` family, `importDerived`, `derivedMirror`, `importForeign`, `nameEntity`, `tag`, `queryVariable` | not started | 007 (planned) |
| sheet metal: `sheetMetalStart`, `sheetMetalFlange`, `sheetMetalHem`, `sheetMetalTab`, `sheetMetalJoint`, `sheetMetalMakeJoint`, `sheetMetalCorner`, `sheetMetalCornerBreak`, `sheetMetalBendRelief`, `sheetMetalRip`, `sheetMetalJog`, `sheetMetalBend`, `sheetMetalUnfold`/`Refold`, `sheetMetalFormed`, `sheetMetalLoft`, `sheetMetalGeometryPattern`, `sheetMetalTransform`, `sheetMetalEnd`, `pcbStart`/`pcbEnd` | not started | 008+ (planned) |
| tables (`cutlistTable`, `holeTable`, `routingCurveTable`), computed properties (`computeMass`) | not started | later |

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
  `opMoveFace`, `opReplaceFace`, `opModifyFillet`, `opHole`, `opCreateCompositePart` (the cases above), and the
  curve and surface operations of batches 004 and 005.
- attributes: `setAttribute`, `getAttributes`, `getAttribute`, `getAllAttributes`, `removeAttributes`; they follow
  their entities through operations.
- evaluation: `evaluateQuery` (and counts), `evPlane`, `evOwnerSketchPlane`, `evVertexPoint`, `evArea`, `evLength`,
  `evVolume`, `evBox` (with `cSys`), `evAxis`, `evLine`, `evCurveDefinition` (lines, circles, ellipses),
  `evEdgeTangentLines` (with a face's orientation), `evFaceTangentPlanes`, `evFaceTangentPlanesAtEdge`, `evDistance`
  (minimum; extended sides), `evCollisionDetection`.
- patterns: `computeLinearPatternTransforms`, `computeCircularPatternTransforms`, `setPatternData`.
- properties: `setProperty`, `getProperty`.
- not modelled yet, answered as if absent: sheet metal, feature patterns.

## Query types

Handled: `NOTHING`, `EVERYTHING`, `TRANSIENT`, `CREATED_BY`, `SKETCH_REGION`, `SKETCH_ENTITY`, `IMPRINT`, `UNION`,
`INTERSECTION`, `SUBTRACTION`, `ENTITY_FILTER`, `NTH_ELEMENT`, `BODY_TYPE`, `CONSTRUCTION_FILTER`,
`SKETCH_OBJECT_FILTER`, `OWNED_BY_PART`, `OWNER_PART`, `CAP_ENTITY`, `GEOMETRY`, `CONTAINS_POINT`, `VERTEX_ADJACENT`,
`EDGE_ADJACENT`, `EDGE_VERTEX`, `TANGENT_CONNECTED_EDGES`, `TANGENT_CONNECTED_FACES`, `CLOSEST_TO`, `FARTHEST_ALONG`,
`LARGEST`, `SMALLEST`, `TRACKING` (entities record what they were made from), `ATTRIBUTE_FILTER`, `OP_HOLE_FACE`,
`OP_HOLE_PROFILE`, `EDGE_TOPOLOGY_FILTER`, `SPLIT` (split faces' sides), `CONTAINED_IN_COMPOSITE`,
`COMPOSITE_CONTAINING`, and the sheet metal, mesh and modifiable filters (as if there were none).

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
- **A plane tangent to a cylinder** (batch 006, c016): `cPlane` places it from the cylinder's surface origin.
  For a cylinder extruded from z = 0 to 20, Onshape's origin is at z = 20, the far end; OCCT's is at the profile.
  More cases are needed to learn the rule (which end, and for other ways of making a cylinder).
- **Fillets too big for their faces** (c015). A 15 mm fillet on the four vertical edges of a 30 × 20 mm block:
  Onshape lets the two fillets on each 20 mm side consume it, leaving the lens where the cylinders meet (4 faces,
  6 edges, 4 vertices); OCCT fails.
