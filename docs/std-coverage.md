# std coverage

The goal: every feature in Onshape's std runs on our engine and matches what Onshape builds, checked against
fixture batches Onshape generated (see `fixtures/` and `crates/fs-fixtures`). This file tracks where each feature
stands. "Matches" means every case of it in a batch builds the same parts (STEP) and the same probe lines as
Onshape.

## Features

| feature (file) | status | batch |
|---|---|---|
| `extrude` (extrude.fs) | matches: blind, symmetric, two directions; new, add, remove, intersect; surfaces from edges; thin walls; up to a face (with offset), body, vertex, the next part; through all (as far as Onshape: the parts' box diagonal plus 0.01 mm, 007); drafts. In a sheet metal flat pattern (`SMFlatOp`), cutting and adding, for plates without bends (059) and parts of planar walls and bends, across the bends too, sketched on any wall's face in the flat (061, 062: a flanged plate, a channel; the flat faces where Onshape has them). Known gap: a cut whose edge crosses a bend (062 c006). Not yet: flat patterns of parts with curved walls | 001, 004, 007, 059, 061, 062 |
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
| `fillet` (fillet.fs) | matches: circular fillets of edges, edge sets, every edge (vertex blends), concave edges, cylinders, holes, tangent chains; the coordinate systems of straight edges' fillet cylinders (047). Conic cross sections and asymmetric fillets of straight edges between planes: rho, sizes, 60° and 120° corners, concave edges, flipped and uneven radii, equal radii, asymmetric conics, several edges (003 c013–c014, 052; Onshape's cubic B-spline sections, knot for knot; its own volumes of them stray, so their probes are compared loosely). Curvature cross sections, the same range (053: Onshape's quintic sections, pole for pole). Fillets too big for the faces they round, which Onshape lets consume them (c015), when the edges are straight between planes and don't meet. Two such fillets meeting at a corner, mitred (053 c013–c015). Known gaps: conic, curvature and asymmetric fillets of curved edges, between curved faces or of three edges meeting at a vertex (053 c012, which Onshape blends with a bicubic patch); curvature fillets of magnitude 0.98 and up (055 c019: 0.99). Curvature fillets from 0.72 to 0.97 (053 c004, 055, 056, 059: two regimes). Variable (smooth) and partial fillets, full rounds: match. A sheet metal corner edge (a corner break, as in Onshape). Not yet: variable conic, curvature and asymmetric | 003, 010, 017, 052, 053, 055, 056, 059 |
| `chamfer` (chamfer.fs) | matches: equal offsets, two offsets (both ways), offset and angle, every edge, cylinders, concave edges, tangent chains, a sheet metal corner edge | 003, 017 |
| `shell` (shell.fs) | matches: removing faces inwards and outwards, hollow, cylinders, an L | 003 |
| `draft` (draft.fs) | matches: neutral plane drafts, both pull directions, cylinders; reference entity drafts (inside rib); parting line drafts, one sided either way, symmetric and two sided (049) | 003, 006, 049 |
| `deleteFace` (deleteFace.fs) | matches: heal, cap, leave open | 003 |
| `moveFace` (moveFace.fs) | matches: offsets; translating a face along its normal, a hole sideways, a top face sideways; rotating a planar face whose neighbours are perpendicular to it; a top face translated up to a plane and to a vertex; creating new surfaces (translated, offset) | 003, 010, 018, 021, 026 |
| `replaceFace` (replaceFace.fs) | matches: a planar face replaced by a plane, with an offset, or by a curved sheet's surface, when its neighbours are perpendicular to it; a cylinder by a coaxial cylinder. A planar face with leaning planar neighbours replaced by a plane; a plain cylinder's side by an off-axis parallel cylinder (058). Not yet: other curved replaced faces | 003, 020, 028, 058 |
| `thicken` (thicken.fs) | matches: faces of solids, planar and curved, one and two sides, midplane, added to a part; sheets (inside rib, thin revolve and thin loft, the offset faces meeting sharply) | 003, 006, 028 |
| `externalThread` (externalThread.fs) | matches: ISO threads on a shaft, blind with the face split, with a chamfer, with an undercut, up to the next face, with their hole attributes; an ANSI thread on two shafts at once (034) | 031, 034 |
| `hole` (hole.fs) | matches: simple, counterbored and countersunk holes; blind (118° and flat tips), through, up to the next part, up to a face, blind in the last part; several at once; simple, counterbored and countersunk holes through a sheet metal plate; from a plane; tapped and clearance holes from the standard tables, with their hole attributes; the newer (V3) hole; depth to the tip; clearance and tapped (the V3 hole's, through one part into the next) | 006, 007, 017, 018, 021, 024, 026 |
| `rib` (rib.fs) | matches: parallel and normal to the sketch plane, with and without draft, a short profile extended to the part, crossing ribs merged | 006, 007, 019, 021 |
| `modifyFillet` (modifyFillet.fs) | matches: changing the radius, removing; refilleting neighbouring fillets (a top fillet round upright ones) | 006, 028 |
| `bodyDraft` (bodyDraft.fs) | matches: by faces, top edges and parts, one side of a planar parting object, crossing the part or at its side; both sides (symmetric), matching at the parting plane or not, plane corners (050); by edges on both sides, an L with its concave edge on both sides, whole parts refused on themselves (051). A cylindrical parting sheet, split by it (058). Not yet: curved parting sheets on both sides | 006, 007, 050, 051, 058 |
| `faceBlend` (faceBlend.fs) | matches: a circular rolling ball blend between two sheets meeting at an edge. Between planar sheets (054): conic and curvature sections, two radii (either way round), width control (even and uneven), untrimmed and detached blends, sheets at 60° and 120°, sheets apart refused for std's wrong flip. Untrimmed blends' run-on, short and long trims, sheets apart blended, uneven radii and conics at 60° (055). Width ratios at 60° and 120° (refused where a side gets no setback, as Onshape does), two radii at 120°, conic, curvature and uneven sections trimmed to walls of unequal extent (056). Slanted ends where walls of unequal extent are trimmed (055 c004; the engine's own sections built the same way match it too). Not yet: curved sides, several faces a side, caps, limits, cliff edges, help points, swept profiles | 010, 054, 055, 056 |
| sketch curves (sketch.fs) | matches: lines, arcs, circles, rectangles, polygons, polylines, ellipses, elliptical arcs, fit splines (open, closed, with end derivatives), Béziers, conics; the sketch's bodies and entities. Not yet: text, images, interpolated splines, constraints (accepted, not solved) | 001, 004 |
| `sweep` (sweep.fs) | matches: circles and rectangles along lines, arcs and tangent chains; surface sweeps; thin. Twisted and scaled along a line (019); along an arc keeping orientation (to 1.6e-5: Onshape's sides are approximations) and locked to a direction (021). Along a sharp L, mitred; locked to a planar face along a tangent path, and rejected along a sharp one (034). Known gap: where `evPlane` puts the origin of a swept planar face (c034). Twisted and scaled along an arc (048, to Onshape's approximations). Twisted and scaled along a line and an arc (057; to Onshape's approximations, 7e-5). Locked to a cylinder's side along a coaxial circle (058: a revolve). Twisted and scaled along a spline, loosely (065: within 2.5e-4 of the volume). Not yet: locked to other curved faces, twist and scale along chains with splines in them | 004, 006, 019, 021, 034, 048, 057, 058 |
| `loft` (loft.fs) | matches: lofts between squares and between two circles; surface lofts between arcs. Lofts through three profiles (004 c038), to points at either end (057), and from a square to a circle, the circle split at the corners' angles (004 c039). Thin lofts with their ends square to the walls. End conditions between two profiles, at one end or both: normal to the profiles, along a direction, tangent to a circle's plane; squares and circles, coaxial and off axis (055, 056; a square's to Onshape's fits, 0.05 mm³). End conditions through more profiles, and to a point. Matching the tangency of the faces next to a profile, squares (to Onshape's bulging fits) and circles (057, 058); matching their curvature, between two profiles (057 c007, to Onshape's fits, 3.6e-4). Tangent to a polygon's plane, exactly, at either end or both, through two profiles or more (056 c003; 060 c006–c008; 061 c008–c010; 062 c008: rectangles, hexagons, squares off axis, magnitude 2; through three, its inner knot 5e-5 off). Known gap: triangles turned to each other, which Onshape fails (061 c011). Normal to a polygon's plane, loosely (056 c001, 063 c005–c007: Onshape blends the derivative along each edge by a rule not found, within 2e-5 of the volume). Known gap: normal to a direction, which Onshape refuses (056 c006). Not yet: guides (055 c027's arc guide is ignored by Onshape too), connections, sections, thin lofts' ends trimmed to the profiles' planes | 004, 006, 028, 055, 056, 057, 058, 060, 061, 062, 063 |
| `helix` (helix.fs) | matches: around a cylinder by turns and pitch, both hands; about an axis; tapering | 004 |
| `fitSpline` (fitSpline.fs) | matches: through vertices, open and closed (with simple knots, as Onshape's); through tangent edges; with start and end directions, one end alone, magnitudes; matching curvature at an end (Onshape's knots and poles) and at both; closed ones are periodic; derivatives at inner points, as routing curves give them (Onshape's knots and poles, 035) | 004, 010, 011, 018, 020, 027, 030, 035 |
| `compositeCurve` (compositeCurve.fs) | matches | 005 |
| `projectCurves` (projectCurves.fs) | matches: two sketches; a curve onto a face along a direction, and along its normals | 005, 019 |
| `intersectionCurve` (faceIntersection.fs) | matches: a cylinder and a plane; crossing cylinders, each closed curve from the seam of the cylinder it winds round and split where it touches an edge of either face, either way round (059 c001, c002; 060 c002–c005; 061 c006, c007) | 005, 059, 060, 061 |
| `isoparametricCurve` (isoparametricCurve.fs) | matches: equally spaced both ways; at given positions round a cylinder (Onshape's u runs from the cylinder's x axis towards its y axis), through a point | 005, 018, 020 |
| `offsetCurveOnFace` (offsetCurveOnFace.fs) | matches: a straight edge across a planar face; a cylinder's top edge down its side. Which way an offset goes, and no curve where it leaves the face (020 c008, 027, 059: into the face by default, but out of a planar face whose edge's other face is curved); across a fillet's cylinder. Known gap: a chamfer's edge on a block's top extended or splitting, which Onshape finds no result for (left out of batch 027). Not yet: other curved faces; extending and splitting checked against Onshape | 005, 020, 027, 059 |
| `trimCurve` (moveCurveBoundary.fs) | matches: extending a straight curve by a distance and up to a plane, trimming it by a plane and by another straight curve; extending up to a cylinder; arcs, cubic B-splines and chains ending in them extended at their end, straight on and curving on, by a distance and up to a plane (Onshape's B-spline knots and poles), arcs trimmed by a plane (036); extended at their start, B-splines trimmed by a plane, arcs trimmed by a line, elliptical arcs extended (038). Arcs and cubic B-splines trimmed by curves (065 c006). Not yet: extended up to curves other than lines, rational or other degree B-splines | 005, 018, 021, 027, 036, 038 |
| `curvePattern` (curvePattern.fs) | matches: equal and distance spacing along an arc, keeping orientation, locked to a face | 005, 018, 019, 020 |
| `offsetSurface` (offsetSurface.fs) | matches: faces of solids and sheets, both ways | 005 |
| `extendSurface` (extend.fs) | matches: planar sheets extended by a distance, all or some edges; a cylindrical sheet, carried on round its cylinder; up to a plane and a vertex, up to a part; sheets extruded from curves, past a curve's end straight on and keeping its curvature, and along the extrusion (039); cones round and along, spheres past a circle (039, each point carried the distance along its circle or line). Known gap: spheres round (039 c003, 055 c012–c015, 056 c019–c021: Onshape's new edges sampled; they depend on the whole band). A torus sheet round the torus and round the tube (057). Ruled B-spline sheets (lofts between curves) past a curved edge, loosely (065 c004). Not yet: other curved sheets | 005, 018, 020, 021, 027, 039, 055, 056, 057 |
| `fill` (fillSurface.fs) | matches: planar loops (also a triangle; see known gaps for its plane's origin); four lines round a non-planar loop, Onshape's bilinear patch exactly (062 c004). Not yet: other non-planar fills, tangency and curvature conditions, guides | 005, 024, 062 |
| `boundarySurface` (bsurf.fs) | matches, loosely: two u profiles, with or without v profiles along the ruled surface's sides (005 c019, 062 c001, c002; Onshape rules between its own fits of arcs, 4e-6 off). Known gaps: v profiles across, end conditions (063 c009–c011) | 005, 062, 063 |
| `ruledSurface` (ruledSurface.fs) | matches: normal to a face, aligned with a direction (also as angle 0 from it), tangent to a face, 30° from a vector; vertex overrides on a straight edge (a width, up to a point); normal to a curved face along its edge, the rulings turning with it, with width overrides when they stay in a plane (040, to Onshape's approximations). Not yet: rulings turning out of a plane along a curved edge (std's up-to override takes only vertices and mate connectors) | 005, 018, 019, 024, 040 |
| `enclose` (enclose.fs) | matches: a tube closed by a slanted plane (once areas and volumes were integrated over curves projected again from the edges' 3D curves, batch 026) | 005 |
| `mutualTrim` (mutualTrim.fs) | matches: two crossing planar sheets, merged | 005 |
| `mateConnector` (mateConnector.fs) | matches: on faces (centroid), vertices, circular edges (centre), edge mid points; moved, turned, flipped, secondary axis; between two faces; realigned to two edges; a given normal (ignored, as Onshape does, off meshes) | 005, 018, 020 |
| `transform` by mate connectors | matches | 005 |
| `bridgingCurve` (bridgingCurve.fs) | matches: tangent and curvature matches between lines and an arc | 010 |
| `wrap` (wrap.fs) | matches: a planar face onto a cylinder as a surface, a solid and an imprint; onto a cone as a surface; unwrapping a cylinder's face onto a plane with `opWrap`, a whole side refused, and a cone's side refused as Onshape refuses it (`INVALID_ROLL_SURFACE`, 046) | 010, 020, 024, 046 |
| `editCurve` (editCurve.fs) | matches: raising the degree (of an arc, via its exact rational B-spline, and of a line), flattening onto a plane, approximating a line and arc with one cubic (to within its tolerance), moving control points; raising a periodic curve (known gap: Onshape keeps std's clamped result flagged periodic, which OCCT can't, 027 c005) | 011, 027 |
| `routingCurve` (routingCurve.fs) | matches: polylines with bends, interpolated splines, their tables (XYZ and LRA); points at vertices (034); points on curves, derivatives at end and inner points (035). Not yet: points from CSV | 011, 034, 035 |
| `constrainedSurface` (constrainedSurface.fs) | Known gap: a smooth surface through nine points is within tolerance of them, but isn't Onshape's surface (c006). Not yet: normals at the points, meshes, deviation | 011 |
| `decal` (decal.fs) | matches: planar and cylindrical, turned and shifted (an image given by size only); realigned to a mate connector (035; batch 011's frames checked in full there) | 011, 035 |
| `compositePart` (compositePart.fs) | matches: closed composite of two blocks; copied with its bodies by `opPattern`, and held in an open composite (035) | 006, 035 |
| `isocline` (isocline.fs) | matches: splitting a sphere, curves on a torus, a sphere and a cylinder across the direction; a cone across the direction (034). A whole torus tilted to the direction, as curves (057) and split along them (058). Curves on B-spline faces, traced (065 c003). Not yet: splitting B-spline faces, partial tilted tori | 007, 028, 030, 034, 057, 058 |
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
| `derivedMirror` (derivedMirror.fs) | runs: mirrors a Part Studio's parts across the xy plane. Can't be compared: Onshape fails it with "Execution error" from a feature (it builds its source twice, which Onshape refuses for a build function in the studio, 060 c001); it is meant for assembly mirrors | 060 |
| `importForeign` | not started (needs uploaded data) | later |
| `sheetMetalRecognize` (sheetMetalStart.fs) | matches: an extruded L bracket recognized as sheet metal with bends; an L with its bend already rounded, a rolled wall (half a tube), an L of two thicknesses (042). Not yet: rips, walls on other curved surfaces | 031, 042 |
| `sheetMetalStart` (sheetMetalStart.fs) | matches: thickening sketch regions (both ways), extruding sketch lines into an L and a U (with bends), converting a block into an open box (bends along the bottom, rips up the corners) | 012 |
| `sheetMetalFlange` (sheetMetalFlange.fs) | matches: 90° and 45° flanges (inner, outer, middle and hold-line alignment, default and own radius, the other way), up to a plane, flanges on two edges meeting at a corner (simple corner, edge-to-edge rip), on an edge of a converted box (whose rips leave the wall short of the edge's ends). Mitred flanges at 30° fail in Onshape and the engine alike. Partial flanges, with their bend reliefs (obround, rectangular, tear; scaled). Corner reliefs where two flanges meet: round and rectangular (scaled), sized round and square, closed; their areas and volumes within 0.01 (2e-6 of them), as Onshape's relieved bends are B-spline faces | 012, 013, 014, 015, 016 |
| `sheetMetalBend` (sheetMetalBend.fs) | matches: a plate bent along a line, 90° up and 60° down holding the other side | 013 |
| `sheetMetalJoint` (sheetMetalJoint.fs) | matches: a bend made a rip (the model falls into two parts), a bend given another radius | 013 |
| `sheetMetalTab` (sheetMetalTab.fs) | matches: a tab merged into a plate's edge | 013 |
| `sheetMetalEnd` (sheetMetalEnd.fs) | matches: the parts become ordinary parts | 013 |
| `sheetMetalJog` (sheetMetalJog.fs) | matches: a 5 mm jog anchored outside, whose bends std turns to 75.5° to meet with no wall between | 013 |
| `sheetMetalCorner` (sheetMetalCorner.fs) | matches: a round relief where two flanges meet, cut in the flat pattern and folded back through the plate, the bends and the flanges (area and volume within 0.01) | 013 |
| `sheetMetalHem` (sheetMetalHem.fs) | matches: straight hems (with a radius, closed to the minimal gap, aligned in place or outside), a rolled hem. tear drops (closed to the minimal gap or with a tip gap, in place or outside; their profile from a constrained sketch the engine solves). Known gap: hems meeting at a closed corner (021 c012: the volume to 2e-6, but the engine trims 0.57 mm² off the plate's underside at the corner, which Onshape keeps whole); the engine's STEP of a tear drop hem reads back 2e-6 off (029) | 014, 021, 029 |
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
  coordinate systems for extruded and revolved cylinders and cones; B-spline surfaces as `BSplineSurface`, unless
  `returnBSplinesAsOther`).
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

- **Spheres extended round** (batch 039 c003, 055 c012–c015): a band of a sphere extended round, at its edge
  along a meridian, turns d/r at the equator but more at latitude 60° (0.200754, 0.406117, 1.106271 rad for 1, 2,
  5 mm at radius 5), on a wobbly approximated edge; 10 mm fails with `EXTEND_SELF_INTERSECTION`. The rule isn't
  known yet.
- **Curvature fillets of magnitude 0.98 and up** (batch 055 c019, 063 c001–c003): past the third regime's end
  (0.979) α₂ and β₃ follow a law not found (data in docs/FINDINGS.md).
- **Fills and boundary surfaces of curves** (batch 063 c008, c009, c011): Onshape's own fits; a boundary surface's
  end conditions aren't modelled (the engine ignores them; Onshape fails one tangent to its profile, c010).
- **A circle across a bend cut in the flat** (batch 062 c006): Onshape wraps it onto the bend; the engine's
  definition cut across the sharp edge fails to rebuild.
- **Flat patterns of parts with curved walls** (`qCorrespondingInFlat`): the engine models a sheet metal model's
  flat pattern (a hidden body) only for planar walls and bends; otherwise a held wall's faces stand for their
  counterparts in the flat. Its edges, vertices and side faces in the flat are nothing.
- **Loft normal to a direction** (batch 056 c006): Onshape refuses it (`LOFT_INVALID`); the engine builds it.
- **Spheres' bands extended round** away from the equator (batch 056 c019–c021) add data to the sphere gap above.
- **Where `evPlane` puts the origin of a rotated copy's piece bounded by a curve** (batch 037 c004): the engine
  carries the copy's axes and stretches the box as for straight-edged faces, and lands 0.025 mm off Onshape's,
  all across the copy's axes: the side where the face touches the box along the curve evidently stretches by more.
- **Lofts matching the faces next to a profile** (batches 057, 058): the engine's sides are exact at the corners,
  but Onshape's bulge between them by a rule not found (up to 3.6e-4 of the volume, matching curvature); these are
  compared loosely.
- **Relieved bends** (batch 013 c006; batch 016 c007–c011, c013). Corner reliefs and closed corners leave Onshape's
  bends B-spline faces; the engine wraps its flat bend onto the cylinder, which gives areas and volumes within
  0.01 mm² and mm³ (2e-6 of them). Those cases are compared to 0.01.
- **Constrained surfaces** (batch 011, c006). Through nine points on a 40 × 30 mm grid with the middle one 4 mm
  up, Onshape's surface reaches 0.05 mm (in x) and 0.09 mm (in y) past the outer points, dips to −0.015 mm and
  has an area of 1242.61 mm²; the engine's least-squares bicubic over the points' rectangle has 1255.91 mm². Both
  are within the 0.01 mm tolerance of every point.
