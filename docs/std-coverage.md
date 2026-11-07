# std coverage

The goal: every feature in Onshape's std runs on our engine and matches what Onshape builds, checked against
fixture batches Onshape generated (see `fixtures/` and `crates/fs-fixtures`). This file tracks where each feature
stands. "Matches" means every case of it in a batch builds the same parts (STEP) and the same probe lines as
Onshape.

## Features

| feature (file) | status | batch |
|---|---|---|
| `extrude` (extrude.fs) | matches: blind, symmetric, two directions; new, add, remove, intersect | 001 |
| `revolve` (revolve.fs) | matches: full, blind, opposite, symmetric, two directions; new, add, remove, intersect. Not yet: up-to bounds, thin, surface | 002 |
| `cube`, `fCuboid`, `fCylinder`, `fCone`, `fSphere`, `fEllipsoid` (primitives.fs) | matches | 002 |
| `booleanBodies` (boolean.fs) | matches: union, subtraction (keep tools, several targets), intersection. Not yet: offset, sheet metal | 002 |
| `transform`, `copyPart` (transformCopy.fs) | matches: translate (xyz, distance, line), rotate, copy, uniform scale. Not yet: mate connectors, non-uniform scale | 002 |
| `mirror` (mirror.fs) | matches: part mirror. Not yet: face and feature mirror | 002 |
| `linearPattern` (linearPattern.fs) | matches: part pattern, one and two directions, centred, skipped instances, add. Not yet: face and feature patterns | 002 |
| `circularPattern` (circularPattern.fs) | matches: part pattern, equal spacing, centred, opposite, add. Not yet: face and feature patterns | 002 |
| `splitPart` (splitpart.fs) | matches: split solids by a plane or face, keep both, front or back. Not yet: sheet tools, splitting faces | 002 |
| `deleteBodies` (deleteBodies.fs) | matches | 002 |
| `cPlane` (cplane.fs) | matches: offset, plane and point, line and angle, line and point, three points, mid plane, curve point. Not yet: tangent plane | 002 |
| `cPoint` (cpoint.fs) | matches: point on an edge | 002 |
| `assignVariable` (variable.fs) | matches: assigned lengths, numbers and angles; measured distance and length. Not yet: table and diameter modes | 002 |
| `fillet`, `chamfer`, `shell`, `draft`, `deleteFace`, `moveFace`, `replaceFace`, `thicken`, `hole`, `rib`, `modifyFillet`, `faceBlend`, `bodyDraft` | not started | 003 (planned) |
| `sweep`, `loft`, `helix`, `fitSpline`, sketch curves (ellipses, splines, conics, text), `curvePattern`, `compositeCurve`, `projectCurves`, `intersectionCurve`, `isoparametricCurve`, `bridgingCurve`, `offsetCurveOnFace`, `trimCurve`, `editCurve`, `routingCurve` | not started | 004 (planned) |
| `fill`, `boundarySurface`, `ruledSurface`, `offsetSurface`, `extendSurface`, `enclose`, `mutualTrim`, `constrainedSurface`, `isocline`, `wrap`, `decal` | not started | 005 (planned) |
| `mateConnector`, `frame`, `frameTrim`, `gusset`, `endcap`, `cutlist`, `compositePart`, the `sectionPart` family, `importDerived`, `derivedMirror`, `importForeign`, `nameEntity`, `tag`, `queryVariable` | not started | 006 (planned) |
| sheet metal: `sheetMetalStart`, `sheetMetalFlange`, `sheetMetalHem`, `sheetMetalTab`, `sheetMetalJoint`, `sheetMetalMakeJoint`, `sheetMetalCorner`, `sheetMetalCornerBreak`, `sheetMetalBendRelief`, `sheetMetalRip`, `sheetMetalJog`, `sheetMetalBend`, `sheetMetalUnfold`/`Refold`, `sheetMetalFormed`, `sheetMetalLoft`, `sheetMetalGeometryPattern`, `sheetMetalTransform`, `sheetMetalEnd`, `pcbStart`/`pcbEnd` | not started | 007+ (planned) |
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
  (bodies), `opSplitPart`, `opDeleteBodies`.
- evaluation: `evaluateQuery` (and counts), `evPlane`, `evOwnerSketchPlane`, `evVertexPoint`, `evArea`, `evLength`,
  `evVolume`, `evBox` (with `cSys`), `evAxis`, `evLine`, `evCurveDefinition` (lines, circles, ellipses),
  `evEdgeTangentLines`, `evFaceTangentPlanes`, `evDistance` (minimum; extended sides).
- patterns: `computeLinearPatternTransforms`, `computeCircularPatternTransforms`, `setPatternData`.
- properties: `setProperty`, `getProperty`.
- not modelled yet, answered as if absent: attributes, sheet metal, composite parts, mate connectors.

## Query types

Handled: `NOTHING`, `EVERYTHING`, `TRANSIENT`, `CREATED_BY`, `SKETCH_REGION`, `SKETCH_ENTITY`, `IMPRINT`, `UNION`,
`INTERSECTION`, `SUBTRACTION`, `ENTITY_FILTER`, `NTH_ELEMENT`, `BODY_TYPE`, `CONSTRUCTION_FILTER`,
`SKETCH_OBJECT_FILTER`, `OWNED_BY_PART`, `OWNER_PART`, `CAP_ENTITY`, `GEOMETRY`, `CONTAINS_POINT`, and the sheet
metal, mesh, composite and modifiable filters (as if there were none).
