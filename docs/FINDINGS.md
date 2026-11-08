# Findings

Facts about Onshape and FeatureScript that our engine has to match. Each entry says how it was found.

## Language

- **Operator precedence and associativity** (fixture batch 001, language probes c096–c104; details in
  `fixtures/batch-001/README.md`):
  - `-2` is a negative constant: `-2^2` is `(-2)^2 = 4`. FsDoc's tokens page says so too ("Negated constants are
    converted to negative constants"). Whether `-x^2` is `-(x^2)` for a variable `x` hasn't been checked yet.
  - The ternary `?:` is left-associative: `a ? b : c ? d : e` is `(a ? b : c) ? d : e`.
  - `~` (string concatenation) binds looser than `==`. Where it sits relative to `&&` and `||` hasn't been checked
    yet; the parser puts it between `&&` and `==`.
  - `??` binds looser than `&&`.
  - `is` and `<` bind tighter than `==`.
  - `*` and `%` share a level and group left to right; `^` is right-associative.

  `crates/fs-lang/src/parser.rs` has the full table, with the checked rows marked.
- **`%`** takes the sign of its second operand: `-7 % 3` is `2` (probe c104; FsDoc's syntax page says the same).
- **Feature functions need `geometry.fs`.** `onshape/std/common.fs` exports the operations (`opExtrude`), sketch
  functions and queries, but not features such as `extrude`. With only `common.fs` imported, calling
  `extrude(context, id, definition)` fails at run time with "Function extrude with 3 argument(s) not found".
  Importing `onshape/std/geometry.fs` fixes it. (First attempt at batch 001.)
- **std versions per file.** Since std 2985, each file's `FeatureScript` header and import versions change only
  when the file or something it imports changes, so one std release mixes versions from 2985 upwards. (Copying std
  3112.0; `std/NOTICE.md`.)
- **Error messages match.** Running the batch 001 probes c097 and c102 on our interpreter throws exactly the
  messages Onshape gave ("First operand of logical or conditional operator must be boolean, value is string" or
  "... number").
- **Lambdas capture by value.** FsDoc's relational page defines lambda equality by "bound values captured from an
  enclosing block scope", so the interpreter copies the visible variables when it creates a lambda. Not yet
  checked against Onshape directly.
- **Top-level constants must be evaluated lazily.** std's `defineTolerance` calls its lambda with no arguments,
  although the lambda takes one. So `lengthTolerance`, `diameterTolerance` and `angleTolerance` can't be
  evaluated, and std still loads. Our interpreter evaluates each constant on first use.
- **`~` turns anything into text.** std's `toString(value)` is just `"" ~ value`, and `"a" ~ false` is
  `"afalse"` (probe c097). How Onshape writes non-integer numbers this way hasn't been checked; we use the shortest
  form that reads back exactly.
- **Regex replacement strings.** In `@replace`, `$n` inserts a capture group and backslashes are literal. That is
  an assumption: std builds `REGEX_UNITS` with the replacement `")\\b)"` and clearly means a word boundary.
  Every `id + "name"` goes through `@replace(addend, "^\\.", "_")`, so regexes are on the hot path.
- **Feature tokens.** `@startFeature` returns a token that std keeps in `var token is map`. `@endFeature` and
  `@abortFeature` take that token, or `{}` meaning the open feature with that id (std's two-argument
  `endFeature(context, id)`).
- **Overloads are found through argument types.** A call also considers the overloads exported by the module that
  declares each argument's type: std's `toString(value is map)` in string.fs prints a `Vector` in the map with
  vector.fs's `toString` as `(10, 0, 0)`, a `CoordSystem` with coordSystem.fs's, a `Matrix` with matrix.fs's,
  though string.fs imports none of them. (Batch 011, c005, c010.)
- **`mapArray` and `subArray` keep the type tag.** `mapArray` over a `Vector` gives a `Vector` (it starts from
  `@resize(arr, 0)`), and std's `separatePointsAndWeights` divides `subArray` of a weighted `Vector` by a number,
  which only works if the result is still a `Vector`. (Batch 011, c001, c004, c005.)
- **Refusals are "Execution error".** Onshape won't read properties (`@getProperty`) of a context while a feature
  in it is running ("Cannot get properties during feature regeneration in current context"), and won't start a
  feature whose id isn't under the running one ("Starting feature propertyRollbackId in scope of …"), which a
  computed property such as `computeMass` does. So the hole table and `computeMass` can't run in a Part Studio
  feature. Caught, either error's message is just "Execution error". (Batch 011, c009, c014.)
- **New Feature Studios** start with `FeatureScript 3112;` and `import(path : "onshape/std/common.fs", version :
  "3112.0");` (as of 2026-11-07, creating the fixture document).

## Features

- **Two-direction extrude, both ends the same way.** A blind extrude with `oppositeDirection: true` and
  `secondDirectionOppositeDirection: true` points both ends the same way and fails with `EXTRUDE_FAILED`. (Batch
  001, c065 and c069.)
- **Zero blind depth** fails with `EXTRUDE_FAILED`. (Batch 001, c105.)
- **An empty selection** (`entities: qNothing()`) fails with `EXTRUDE_NO_SELECTED_REGION`. (Batch 001, c106.)
- **A feature can catch a sub-feature's error.** A feature that calls another feature can catch its errors with
  `try`/`catch`, call `clearFeatureStatus` on itself, and still succeed. Batch 001's generator does this, and the
  six failing cases didn't stop the other 100.

- **Where an extrude starts.** std passes `@opExtrude` an `endDepth` and a `startDepth`, and the flag
  `isStartBoundOpposite` says which way the start is measured. With `true` (the default) the start is
  `startDepth` behind the profile; with `false` it is `startDepth` along the extrude direction. A symmetric
  extrude passes half the depth both ways, as a negative `startDepth` with the flag `false`. The second-direction
  cases c065 and c069 pass a positive `startDepth` with the flag `false`, which puts the start beyond the end, and
  that is why Onshape fails them with `EXTRUDE_FAILED`. (Batch 001, from the definitions std builds.)
- **Extrude's intersect is a "subtract complement".** For `NewBodyOperationType.INTERSECT`, std calls
  `opBoolean` with `BooleanOperationType.SUBTRACT_COMPLEMENT`: each target keeps only the part inside the new
  body. (Batch 001, c092–c095.)
- **Failed operations record their own status.** std sometimes calls an operation inside `try`, such as
  `try(opBoolean(context, id, definition))` in `boolean.fs`, and then reads the status of `id` with
  `processSubfeatureStatus`. So an `@op…` builtin that fails must report its error as the status of the id it was
  given, as well as throwing. Without that, a failed boolean inside an extrude looked like success.
- **Overlapping regions merge.** Extruding every region of two overlapping circles builds one solid in Onshape,
  not three. (Batch 001, c049–c053.)
- **Builtins return std's types.** std's `evPlane` returns `@evPlane`'s result as a `Plane` without converting it,
  so the builtin must return a map tagged `Plane` whose origin is a vector of lengths. Other builtins return plain
  numbers in metres that std wraps itself (`@evBox`'s corners go through `box3d`; transforms through
  `transformFromBuiltin`). Queries are passed wrapped, as `@evaluateQuery(context, { "query" : query })`.
- **Revolve's bounds are angles measured forward.** std's revolve passes `@opRevolve` a `startBoundAngle` and an
  `endBoundAngle`, both measured forward (right-handed about the axis) from the profile, and the revolve sweeps
  forward from the start to the end. A symmetric revolve of `a` starts at `2π − a/2`. A blind second direction
  of 45° with a first direction of 60° sweeps only from 45° to 60°, a 15° revolve, unless
  `startOppositeDirection` is set. (Batch 002, c003–c006.)
- **Feature parameters without defaults must be passed.** `transform` has no default for `makeCopy`, and
  translating by a line needs `oppositeDirectionEntity`; leaving them out fails the precondition. (Batch 002, the
  first build.)
- **`opPoint` always makes construction points.** (Batch 002, c077 and c078.)
- **Plane frames are canonical.** The x direction of a plane `@opPlane` makes, and of the plane `@evPlane` returns,
  depends only on the normal: for a mostly horizontal plane (|normal · z| > 1/√2) it is the world x axis projected
  onto the plane; otherwise the plane's y is the world z axis projected onto it and x is y × normal. The x
  direction passed in is ignored. `@evPlane`'s origin is the face's centroid. (Batch 002, the construction planes
  in c045–c076.)
- **A line's axis passes through its midpoint.** `@evAxis` and `@evLine` give a line edge's axis with its origin at
  the edge's midpoint, pointing along the edge's own direction (a sketch line's start to end, kept by extrudes).
  (Batch 002, c072.)
- **Topology without seams.** Onshape's queries see no seam or pole edges and no vertices on closed edges: a full
  revolve's circular edges have no vertices, an extruded circle has 3 faces, 2 edges and 0 vertices, a sphere is
  one face with nothing else, a cone has one vertex at its apex, and a B-spline sphere (the scaled sphere of
  `fEllipsoid`) has a vertex at each pole. (Batch 002 probes.)
- **`evVolume` is accurate to about 1e-7** on curved parts: the two halves of a cylinder split by a slanted plane
  differ from the exact half by that much. (Batch 002, c065.)
- **Returned values must be tagged.** A builtin's result checked with `returns DistanceResult` must carry the
  `DistanceResult` type tag; typecheck predicates only run on `as` casts. (Batch 002, `evDistance`.)
- **`box` is a reserved word.** Onshape rejects it as an identifier. (Batch 002's first generator.)
- **Capping a face Onshape can't cap leaves it.** `deleteFace` with `DeleteFaceType.CAP` on the face of a hole
  through a block changes nothing: no planar face spans its two circles. (Batch 003, c039.)
- **Thicken keeps the original.** Thickening a face of a solid makes a new solid and leaves the original body.
  (Batch 003, c054.)
- **Some feature parameters have no defaults:** `replaceFace`'s `offset` and `thicken`'s `thickness2` must be
  passed. (Batch 003's first build.)
- **A sketch is a wire body per curve and a sheet body per set of regions joined by shared edges** (a ring and
  the square inside it are one sheet of two faces; batch 008, c001), with point bodies for points and centres;
  construction curves are wire bodies too, but `qConstructionFilter` doesn't count them as construction.
  `qCreatedBy(sketch, EntityType.EDGE)` finds the regions' edges as well as the curves'. (Batch 004, c012.)
- **Fit splines are cubic with centripetal parameters on [0, 1]**, natural (zero second derivative) at the ends
  unless end derivatives are given, which are with respect to that parameter; a spline whose last point is its
  first is periodic. This reproduces Onshape's splines to 1e-8. (Batch 004, c004–c006.)
- **`skEllipticalArc`'s parameters have a period of 1** and `skEllipse`'s `majorAxis` must be a unit vector.
- **Up to next stops at the first face of any part**, whatever the boolean scope, even when the extrude starts
  inside a part. (Batch 004, c026.)
- **A sheet extruded from edges faces along the edge's tangent × the extrude direction.** (Batch 004, c013.)
- **Thin walls:** the first thickness goes to the right of the first edge (its direction × the plane normal) for
  an open chain, outward for a closed loop; corners are sharp. (Batch 004, c019–c021.)
- **A helix with `clockwise: false` turns left-handed about its direction.** (Batch 004, c043.)
- **A loft between parallel squares has planar sides**, and sweeps merge faces that continue onto one surface
  along the path. (Batch 004, c032, c035, c036.)
- **Mate connector frames:** a face's connector is at its centroid with z along its normal; a circle's at its
  centre with z along its axis; an edge's mid point has z along the edge; a vertex has the world frame. When z
  comes from geometry, x is the canonical x for it. Flipping turns the frame half a turn about y; the secondary axis
  picks ±x or ±y as x; offsets are along those axes and the rotation comes last. Moving a part by mate connectors
  leaves the connectors where they were. (Batch 005, c024–c029.)
- **Equally spaced iso curves:** across an open parameter range at i / (n − 1), ends included and dropped where
  they lie on the face's boundary (so three curves round a cylinder's height leave one circle); round a periodic
  range at (i + ½) / n. U-direction curves on a cylinder run along it. (Batch 005, c007 and c008.)
- **A projection or intersection that closes on itself is one edge** with no vertices (but Onshape splits the
  curves where two cylinders cross into two edges each). (Batch 005, c004 and c006.)
- **Assumptions not yet checked against Onshape:** transient ids are strings (ours look like `"T12"`); the
  results of `@evaluateQuery` come in body order, then faces, edges and vertices.

- **`qBodyType` folds into `qEverything`.** `qBodyType(qEverything(...), types)` doesn't wrap the query: it sets
  `bodyType` on the `EVERYTHING` query itself, so an engine that ignores that field returns every body. (Batch
  006, c017: the count of composite parts came out as every body in the context.)
- **Holes** are revolved from their profiles, each placed along the axis from where the hole's cylinder enters
  (`TARGET_START`) or leaves (`LAST_TARGET_END`) the targets; a through hole ends 2000 × the zero-length tolerance
  past the last target. `tipAngle` only matters with `tipAngleStyle` `CUSTOM`. `hole` needs the sketch points'
  vertices as `locations`: their point bodies make `opHole` fail. (Batch 006, c001–c006.)
- **A subtraction that keeps its tools** gives the target new faces where the tools cut it, created by the boolean:
  `rib` relies on `qCreatedBy(booleanId, EntityType.FACE)` finding them. (Batch 006, c007.)
- **Thicken tracks back to the sheet.** `rib` tracks the edges of the sheet it extrudes (`startTracking`) and
  finds the rib's top and side faces through `opThicken`: the walls are generated from the sheet's boundary edges.
  `opThicken` of all of a sheet's faces makes one solid. (Batch 006, c008.)
- **Body draft** turns each steep face about its top edge and adds material down to the parting object: the side
  faces of a 40 × 30 × 12 block drafted 4° from its bottom make a frustum 0.839 mm wider each side at the bottom.
  (Batch 006, c011.)
- **Face patterns** add the boss the faces make (or cut the pocket) at each instance. A boss tangent to the
  block's edge splits the block's edge there, one vertex on the boss's circle. (Batch 006, c012–c014.)
- **A revolved sheet faces along tangent × sweep direction**, as an extruded one faces along tangent × direction;
  thin revolve thickens it along that normal. (Batch 006, c018.)
- **Cylinders' and cones' coordinate systems** (`evSurfaceDefinition`) depend on how they were made. An extruded
  cylinder has its origin on the axis at the far end of the extrude, z pointing back along it, and x = the
  circle's y × z (so x is the sketch's −x for an extrude along the sketch normal). A revolved cylinder has its
  origin at the axis's origin, z against the axis, x from the profile towards the axis. A revolved cone has its
  origin at the apex, z against the axis, y from the axis towards the profile. A sphere's is the global frame at
  its centre. (Batch 006, c016; batch 007, c001–c006.)
- **Selection queries** (batch 007, c008–c012): convex (concave) connected faces flood across convex (concave)
  edges, counting a fillet's smooth edges by how the faces bend; `qEdgeConvexityTypeFilter` counts them as smooth.
  Hole faces of a simple hole are just its wall. `qFilletFaces` takes only `EQUAL`, `LESS_EQUAL` and
  `GREATER_EQUAL`. `qLoopEdges` of a two-sided edge is empty. `qFaceOrEdgeBoundedFaces` floods around a single
  bounding edge to every face; `qLoopBoundedFaces` floods everything on the edge's side but the bounding face.
- **Holes up to the next part stop where the hole leaves the first part**; holes up to a face stop the shaft at the
  face, the tip going on past it. Both need `holeDepthComputed` set (positive) when called from code, as the UI
  sets it. (Batch 007, c014–c015.)
- **Body draft across the parting plane**: the part is split there; only the faces in front are drafted about their
  top edges, leaving a ledge at the parting plane. (Batch 007, c019.)
- **Revolve up-to offsets** turn the bound further along the revolve unless flipped. (Batch 007, c022.)
- **Isoclines** are where the angle between the face's tangent plane and the direction (like a draft angle) is the
  given angle: latitude circles on a sphere, two circles at the same height on a torus. (Batch 007, c026–c027.)
- **A closed composite part** owns its bodies' entities (`qOwnedByBody` of it finds them); the bodies stay in the
  context as solids, but Onshape's STEP export writes them without their names. (Batch 006, c017.)
- **A named function can't be used as a value**: "Cannot reference function f as a variable." Pass lambdas, as
  std's operations are (`export const opExtrude = function(...)`). A build function for `PartStudioData` must be
  a lambda too. (Batch 008's first build.)
- **Frames** instantiate their profile's context (std's `derive`, merging contexts), sweep it from whichever end of
  each path edge the path's order starts at, and mitre and butt by replacing cap faces with planes: the new face
  keeps the cap's attributes, which gussets and end caps look for. Coped corners subtract one member from the
  other and delete the leftover pieces, found by tracking the member's body. (Batch 008.)
- **Sketch arcs run counterclockwise** about the sketch normal, whichever of `skArc`'s ends is its start; a
  bridging curve's direction at an arc depends on it. (Batch 010, c006.)
- **Variable fillets with a smooth transition** vary the radius as 3t² − 2t³ between the vertices (level at both
  ends): the volume matches that law exactly. (Batch 010, c002.)
- **Partial fillet bounds** each keep the fillet on the side their edge comes from (`isFlipped`: the side it goes
  to), and the fillet is where any bound keeps it: bounds at 25% and 75% of an edge fillet the outer quarters. The
  ends are square, capped with planar faces. (Batch 010, c003.)
- **Wrapping** keeps distances: a 10 × 10 mm square wrapped onto a 10 mm cylinder along its tangent line has area
  100 mm². (Batch 010, c007.)
- **Mutual trim** splits each sheet's faces with the other (`opSplitFace` with `mutualImprint`), keeps the pieces
  behind the other sheet's normal (`qSplitBy`), and merges the sheets into one. (Batch 005, c023.)

- **Edit curve keeps rational curves rational.** It makes every curve rational (weights of 1) before editing, and
  Onshape's B-spline keeps those weights; OCCT drops equal weights, so the engine nudges the last by an ulp.
  Onshape's fit splines have simple interior knots (C2 cubics), and an arc's B-spline is the rational quadratic
  with middle weight cos 45°, raised to degree 4 with weights 1, 0.854, 0.805, 0.854, 1. (Batch 011, c001,
  c002, c004.)
- **Path deviation** is symmetric: for a line and an arc with the same ends it is the sagitta, at the middle of
  each. (Batch 011, c005.)
- **Routing curves need a bend radius** even as splines: the feature reads `bendRadius` whatever the curve type.
  Polylines are lines and tangent arcs in one wire. (Batch 011, c007, c008.)
- **Approximating a line and a tangent arc** with one cubic to 0.01 mm, Onshape's curve dips 0.0012 mm below the
  line; the engine's least-squares fit dips 0.0057 mm. Both are within tolerance, so the probe is compared to it.
  (Batch 011, c003.)
- **The form tag** adds a mate connector at the part's origin with the attribute. (Batch 011, c012.)

- **Sheet metal parts** (batch 012) come from std's definition sheet, whose faces are walls and whose edges are
  joints: a thickened sketch region's definition is the face on the far side of the material, with the material
  in front of it (along its normal); an extruded sketch line's is the sketch line itself, the material on the
  right of the line's direction; a converted part's faces are the inside of the material. A bend's attribute
  radius is the part's inner radius; the part's bend is the definition's corner rounded on the inside, its inner
  and outer surfaces concentric. Where two bends meet at a corner (the simple corner, the default), each stops
  where the other cuts back the wall they share, leaving the corner square empty; two flanges meeting at right
  angles there, with an edge-to-edge rip, each stop the minimal clearance (0.02 mm) short of the other's inner
  face. Inner flange alignment moves the definition's edge out by thickness × tan(angle / 2) with `opEdgeChange`,
  which must give the edge a new transient id: std compares the edge's query before and after to see it moved.
  Onshape keeps the faces at the ends of walls and bends separate where they're coplanar.
- **Sheet metal can't be moved or seen.** `opTransform` refuses sheet metal parts (SHEET_METAL_PARTS_PROHIBITED);
  outside sheet metal features `qEverything` and `qCreatedBy` don't find the definition sheets (attribute queries
  do), and never find a flat pattern. (Batch 012.)
- **Tracking a sketch entity** takes the sketch's IMPRINT entities derived from it (its regions' edges along it)
  as well, and follows them into surfaces extracted from the regions: the flange builds its wall that way.
  (Batch 012, c006.)

- **More sheet metal** (batch 013). A bend along a line wraps the bend's strip of the definition onto a cylinder
  (distances taken at the allowance radius), so the definition holds the rounded bend itself. A rip on an L's
  only bend leaves two parts, both active. Ending a model leaves ordinary parts, which then move. A tab facing
  against its wall is mirrored onto it and merged: its face and the wall's become one. qPatternInstances finds a
  pattern's named instances; a face split's new edges depend on the tool faces they lie on; opPattern's copies
  keep their attributes and are tracked to what they copy; deleting faces of a sheet leaves a body for each piece,
  the edges that lost a neighbour changed; unions of sheets face one way throughout.

- **Hems and rips** (batch 014). A hem sketches its profile and extrudes it, tracking the profile arc's end
  point ("initialHemArc.end", counterclockwise) into the side edge the extrusion sweeps from it: sketch curves'
  end vertices are named `<id>.start` and `<id>.end`, and extrusions track their faces and edges to the profile.
  A converted box without bends is five parts, every joint a rip. A flange on its wall covers the wall's edge
  only where the rips leave it, 0.02 mm short of each end; the wall is cut back to the rips there, with no
  relief. A rip across a plate from corner to corner leaves it whole. Mitred flanges (30°) fail in Onshape, and
  the plate their case leaves can't be deleted outside a sheet metal feature, so it keeps its default name.

- **Corner breaks and unfolding** (batch 015). A corner break rounds or chamfers the wall's corner in the
  definition itself (in the flat, `@sheetMetalApplyInFlat`), and std finds the changed model by tracking the old
  one, so it's a new body. Unfolding marks bends `unfolded` and a wall `holdFaceForUnfold`; the part then has the
  bend laid flat, (inner radius + K-factor × thickness) × angle long (2.9845 mm for a 90° bend of radius 1, 2 mm
  thick, K 0.45), the far wall carried round into the held wall's plane. Refolding gives the folded part back.

- **Jogs** (batch 013). When the offset is too small for the bend angle, std turns both bends down until they meet
  with no wall between (a 5 mm jog anchored outside, 2 mm thick, radius 1: 75.52°, from cos θ = 1 − 3 / 4). It
  splits the plate up front, stretches the strip it cuts out to the bends' flat lengths, and joins it back with
  `eraseImprintedEdges : false`, which keeps the strip's faces apart from the plate's on one plane. The bends' own
  splits then cut nothing: `ownExistingImprints` makes the edges already on the tools theirs, and no face is
  `qSplitBy` them. The second bend line is a spline fitted through two points, which `evLine` takes as a line.

- **Sheet metal holes** (batch 017). A hole registers its tool bodies on the walls they cut (`cuttingToolBodyIds`)
  and the update cuts them from the definition sheet: std then looks for the circular edges on the sheet made by
  the update. The tools stay in the model, hidden like the definition (they aren't counted as parts). For std to
  find the walls a tool cuts, `evCollision` of bodies reports each pair of faces that meet.
- **Fillets and chamfers of sheet metal corners** (batch 017) go through std's attribute-based corner break: the
  break is kept on the definition vertex (`cornerBreaks`, with the wall it rounds) and the part's wall is rounded
  there.
- **Part entities are made by what made the definition's** (batch 017): `qCreatedBy` of an extrude that cut a
  sheet metal plate finds the hole's wall in the part.
- **Make joint** (batch 017) extends both walls to the plane bisecting them (`opExtendSheetBody` with
  `extendMethod : EXTEND_TO_SURFACE`), then marks the edge they now share. A butt rip ("direction 1") runs the
  first wall on to the other's outer face; the other stops the minimal clearance (0.02 mm) short of the first's
  inner face, as at an edge rip.
- **Bend relief overrides** (batch 017): a sized rectangle reaches its depth in from where the wall is cut back for
  the bend (4 mm deep, radius 1: from x = 39 to 35), as wide as the model's relief; a tear cuts nothing.
- **Sheet metal lofts** (batch 017): between two straight lines, one flat wall facing along the first profile
  crossed with the way to the second.

- **Counterbored sheet metal holes** (batch 018): the flat pattern takes only what goes through (the 4 mm hole),
  and the counterbore is cut from the part.
- **Isoparametric curves round a cylinder** (batch 018): Onshape's u runs from the cylinder's coordinate system x
  axis towards its y axis; for an extruded circle round z that's from −x towards +y (u = 0.25 at (0, 10), 0.6666 at
  (5, −8.66)). Equal spacing round it is at i / n.
- **Extending a cylindrical sheet** (batch 018) carries it on round the cylinder (5 mm of arc), not along tangents.
- **Mate connectors between entities** (batch 018) sit halfway from the inferred origin to the other face, turned as
  the inferred frame; **realigned** ones take z along the primary entity and x along the secondary.

- **Twisted sweeps** (batch 019) turn the profile right-handed about the path's direction, in proportion to the
  distance along it; **scaled** ones grow it in proportion too (a circle makes an exact cone).
- **Ruled surfaces at an angle from a vector** (batch 019) turn the vector about the edge, towards the outside of the
  reference face.
- **A two-point fit spline** behaves as a line: rib profiles extended to the part (by such splines) merge with the
  profile's line.
- **Sized obround bend reliefs** (batch 019) reach their depth in from where the wall is cut back, as sized
  rectangles do; an extended relief changed nothing on a partial flange's end.

- **Two-offset sheet metal chamfers** (batch 020): the first width runs along the edge leaving the corner as the
  wall's loop goes (on a 40 × 30 plate's corner at (40, 30): 6 mm along y = 30, 3 mm along x = 40, for widths 3 and
  6). **Conic corner breaks** are set back by the radius along both edges; Onshape's B-spline puts the shoulder at
  0.69971 for rho 0.7.
- **A mate connector's given normal** (`specifyNormal`) is for meshes: on a face, Onshape ignores it.
- **Offsetting a cylinder's top edge across its top** (batch 020, c008) made no curve in Onshape, without an error.
- **Section parts by several planes**, like `sectionPart`, delete the parts of a Part Studio they don't section.

- **Sweeps keeping their orientation** (batch 021): Onshape's side surfaces are B-splines 1.6e-5 short in area of
  the exact surfaces (the path moved to each profile edge and extruded along it: 742.8419 mm² by hand, Onshape
  742.8303).
- **Trimming a curve by another curve** (batch 021) cuts it where they cross and keeps the side the help point is on.
- **Hems at a closed corner** (batch 021) extrude their pieces from mitre planes (up-to start bounds) and leave the
  plate whole, with two small cylindrical pieces where the hem arcs meet.

- **Closed corners** (batch 016, c013): each bend runs on to the corner, its end where a plane parallel to the base
  wall's normal cuts the sheet. The plane passes through a point on the bend's tangent line on the base wall, just
  short of where the two tangent lines meet, and through the point where the wall beyond stops (its rip setback,
  y = 29.98). The part's end face runs radially across the thickness from there (a B-spline in Onshape): both bend
  cylinders are 29.6194 mm long on average, as a sine law gives. The base wall's corner is cut square to the
  diagonal, 0.01 mm in, so the bends stay the minimal clearance (0.02 mm) apart.

- **Sheet metal forms** (batch 022): the form's parts are placed at each location (the form's mate connector on
  it) and registered on the wall (`formedToolBodyIds`); the update takes the part to remove away from the wall's
  piece and joins the part to add, and they're hidden like the definition. Form tools must overlap the sheet, both
  parts of them. The placement mate connector stays, as a new body a sheet thickness from where it was placed, the
  way the material lies from the definition sheet ((20, 15, 2) to (20, 15, 0) on a plate's top; a flipped form's
  from (20, 15, 0) to (20, 15, −2)).

- **Unwrapping** (batch 046): `opWrap` from a cylinder's face onto a plane inverts the wrap: arc length round the
  cylinder from the anchor goes along the destination's anchor direction, height along the cylinder's outward
  normal × the source anchor direction (whichever way the face is turned). A face all the way round fails
  (`ROLL_FAILED`). A sheet extruded from an arc has its cylinder's origin at the near end and z along the extrude
  (a solid's is at the far end, z back). A fillet's cylinder (a block's upright edge rounded) has its origin at
  the edge's top, z down and x away from the corner's side (not yet matched; std anchors unwraps on it).
- **Composites derived** (batch 045): an open composite holding a sheet metal part comes across with its parts,
  the model kept; a closed one holding sheet metal is refused (`DERIVED_NO_CLOSED_COMPOSITE_WITH_SM_ERROR`); a
  closed composite of plain parts is instantiated. The copied composite holds the copies of its members.
- **Sheet metal lofts** (batch 044): a sketch's lines are wire bodies of their own, so a profile's edges chain
  where their ends coincide. A quad between matched points that isn't planar splits along its shorter diagonal,
  with a bend between the triangles. Where a bend meets the profiles' planes at a slant, Onshape ends it with
  B-spline faces (not yet matched).
- **Unfolding rolled walls** (batch 043): a rolled wall unrolls into the plane tangent to it along the held edge,
  as long as its arc at the neutral radius by the model's rolled k-factor (`kFactorRolled`, 0.5 by default): half a
  tube of radius 20 and 2 mm thick lies flat 21π mm long. A sketch line running on into a tangent arc makes a rip
  between the flat and rolled walls (two parts), each cut back by half the clearance.
- **Recognizing rounded and rolled sheet metal** (batch 042): `evOffsetDetection` pairs coaxial cylinders a
  thickness apart as well as planes; of several thicknesses the commonest is used, the thinnest of those as common
  (an L 2 mm thick at its base and 3 mm at its upright keeps only its base); side0 starts from the first pair's
  larger face (the base's full bottom). std extracts side0 and turns a cylinder between two walls into a bend of
  its radius; a half tube becomes a rolled wall.
- **Assembly sections** (batch 041): std copies each target part to its instances by `opPattern`, cuts the copies,
  and moves the results back. A multiple planes section needs a `bbox`, and each plane cuts square to its x axis,
  through its origin.
- **Ruled surfaces along curved edges** (batch 040): normal to a face, the rulings follow the face's normal along
  the edge (from a round's top arc they fan out in the top's plane, making a planar sheet); a width override
  blends from the default along the edge by length, smoothly (3t² − 2t³) or linearly. Onshape's far edge is an
  approximation (areas within 0.005 mm² of exact). An angle override, or one along a vector, on a curved edge
  fails in Onshape. `evSurfaceDefinition` gives every surface its `surfaceType` (std tells extruded walls by it).
- **Extruded sheets extended** (batch 039) stay extrusions: past the curve's end, the curve is extended as
  `trimCurve` extends it and extruded again; along the extrusion, the extrusion is made longer. Keeping curvature,
  the distance is measured straight from the old edge to the new one, not along the curve as `trimCurve` measures
  it (an extruded spline's extension 1.24e-4 mm longer along it than 5 mm). A natural spline end has no curvature,
  so both ways match.
- **Tags** (batch 038): a standard content tag sets the body purpose attribute; a PCB hole tag sets a
  `pcbStudioHole` attribute on each hole's faces (designator, purpose, owner, plating; its start and end faces as
  queries). `getAttributes` without a name returns neither, in Onshape as in the engine.
- **Section parts' leftovers** (batch 037): the first step of a section, deleting every body but its targets,
  leaves the default planes and sketches in place, in Onshape as in the engine; an aligned section's tracking
  query then takes them in, copying, rotating and cutting them with the parts. Their pieces are ordinary sheets:
  a construction plane cut by a boolean isn't construction any more, and a copy of a sketch's sheet is still a
  sketch's.
- **Curves extended** (batch 036) come out as cubic B-splines, the extension more spans of the curve's. An arc or
  an elliptical arc (batch 038) is first made the clamped cubic spline through points along it, with its exact
  tangents at the ends, parametrized by the angle (an arc's from its start, an ellipse's its own, from the major
  axis): its range is halved, and each half again, until for every span the curve's greatest distance from its
  centre times the span's angle to the fourth is at most 0.365 mm (between 0.357 and 0.376, by the ellipses). An
  arc's spans come out equal (r = 20 mm: 11.25°; r = 10: 22.5°; r = 7 through 120°: 15°); a 20 × 10 ellipse's
  quarter has six of 11.25° and one of 22.5° at its flat end. Straight on, a straight span follows a triple knot, as fast as the curve ends. Curving on, a blend comes
  first: the last span's polynomial with a cubic term added, joining with matching curvature (one knot), ending
  with none and turned 3° from the curve's end (its parameter length the same whatever the distance); then the
  straight span, for what's left of the distance or up to the limit. A fit spline's natural end has no curvature,
  so it goes straight on either way. A trimmed arc stays an arc; in a chain, only the end edge changes. At
  its start (batch 038), a curve is extended the same way, mirrored, its own parameters kept (the extension's
  negative); a B-spline trimmed by a plane is the same B-spline restricted, its parameters kept; a straight limit
  curve stands for the plane through it square to the curve's plane.
- **Splines with derivatives at inner points** (batch 035, from routing curves): Onshape's knots are at the
  centripetal parameters, as without them, and each inner derivative adds one more knot a third of the way into
  the longer of the spans either side of its point, but never into the first span (through five points with the
  first span 2.4 times the second, the second point's knot still goes after it); ends without a derivative are
  natural. Std scales a routing curve's derivatives by its points' bounding box diagonal, and needs `bendRadius`
  even for a spline.
- **A line and a three point sketch arc** meant to be tangent meet 3e-8 radians off, in Onshape as in the engine:
  std's frames don't count that as tangent (1e-8), so they mitre the segments and don't merge them. A block's
  rounded top edges are tangent exactly. A cut list given a frame's closed composite segment lists it as one item,
  its length the segments' together (std copies the composite with its bodies by `opPattern`, dissolves the copy
  and unions the bodies); given the segments, it lists them apart.
- **Decals on a full cylinder** (batch 035): std tells a face that goes all the way round by evaluating it at
  parameters (0, 0) and (1, 1); Onshape evaluates the far end of the period exactly at the seam, so the decal is
  centred at the cylinder's x axis. (Batch 011's probes printed only the decals' frame origins.)
- **Sweeps round sharp corners** (batch 034): a profile left free sweeps round a sharp corner of its path and is
  mitred there (a 6 mm square along an L of 100 and 60 mm lines: 36 mm² × 160 mm, 8 planar faces, the L's top
  and bottom one face each). Locked to a face, it can't: Onshape finds the path invalid (`SWEEP_INVALID_PATH`).
  Along a path rounded at the corner, a locked sweep's faces on one plane are one face too.
- **ANSI threads** (batch 034): std's lookup table parser can't evaluate a "16 tpi" pitch (`tpi` isn't among its
  units) and catches the error, in Onshape as on the engine; the thread is built from the table all the same.
- **Sheet metal form patterns** (batch 033): std copies each form's parts and placement mate connector by the
  pattern's transforms (found by the ids the wall's form records, and by tracking the copies) and registers them;
  the copied mate connectors stay where they were copied to, at the material. The form's own faces on the part are
  created by the form feature. A cup tool's part to remove doesn't cut into its boss here, in Onshape as in the
  engine.
- **Sheet metal hole patterns** (batch 032): std finds a hole's faces on the part by the names `@opHole` gave its
  tool's faces (the part's faces cut by a registered tool carry them), attributes them, and on a face pattern asks
  `evSheetMetalHoleToolBodies` for the tools, copies them by the pattern's transforms (`PROPAGATE_NEW_HOLE`) and
  registers the copies. Registering, it first copies the hole's own tools (`PROPAGATE_SAME_HOLE`), so they stay the
  hole's for `qOpHoleFace`. It looks a hole's depths up under the sheet metal definition, and attributes the flat
  pattern's faces too. A countersunk hole's shaft comes out 0.00126 mm² short in the engine (known, small).
- **External threads** (batch 031) split the shaft at the thread's depth, chamfer its end, or cut an undercut by
  splitting the shaft face twice and offsetting the piece between inward; the thread is a hole attribute on the
  shaft's face (`isExternalThread`, major and minor diameters, pitch).
- **Recognizing sheet metal** (batch 031): `evOffsetDetection` pairs the faces a thickness apart; std extracts one
  side as the definition sheet and rounds its sharp joins into bends of the given radius.
- **Frames** (batch 030): limited by a part, a frame's end is trimmed to the part's face it runs into; locked to
  a planar face, its profile turns only about the face's normal (std sweeps with `LOCK_FACES`); alignment point 0
  puts the path at the profile's corner. A cut list column override for all frames shows in the table.
- **Tear drop hems** (batch 029): std draws the profile and lets the sketch solver place it: the hem's arc fixed
  (centre and radius; its end free to move round), a line from its end tangent to it, a construction cap square to
  the line at its far end, its inner point the tip gap plus a thickness from the plate and its outer point the
  hem's width from the arc's start. Sketch arcs' `.start` and `.end` run counterclockwise whatever order they were
  given in, and std leaves the cap out with `qConstructionFilter` on the sketch's edges: construction curves' edges
  are construction, though their wire bodies aren't (batch 004). Solved, the arc sweeps 197.6° round a 1 mm radius
  before the line leaves it, for a 10 mm hem on a 2 mm plate.
- **Thin lofts** (batch 028) loft a sheet facing outward from its loops (as extruded sheets face) and thicken it
  with offset faces meeting where they intersect: sharp corners, 16 planar faces between two squares (OCCT's arc
  joins round them). Without `trimEnds` the ends are square to the walls; with it they're cut to construction
  planes through the profiles (`sideGeometryRule` SUPPLIED), which needs the walls extended first.
- **Refilleting** (batch 028): changing upright fillets' radius with `reFillet` takes away the fillet round the top
  that meets them and makes it again at its own radius.
- **Fit splines matching curvature** (batch 027): with a first and second derivative at the start, Onshape's
  cubic has knots averaged from the centripetal parameters with the start's repeated once per condition: through
  three points, [0, 0, 0, 0, t₁/3, (t₁ + 1)/3, 1, 1, 1, 1], the far end natural. Closed fit splines are periodic
  (std's form lists the first three poles again and its knots run a period past each end); raised in degree by edit
  curve, std hands back Bézier spans (each inner knot as often as the degree) from the second point, with clamped
  knots, still flagged periodic.
- **Offset curves Onshape finds no result for** (batches 020 and 027) warn `OFFSET_CURVE_ON_FACE_CHAIN_YIELDED_NO_RESULT`
  and make nothing: a cylinder's top edge across its top; a chamfer's edge on a block's top, extended. A block's
  front top edge across its top (batch 005) and a cylinder's top edge down its side work.
- **Feature patterns that reapply the feature** (batch 026) run it once per instance with the instance's transform
  pushed (`setFeaturePatternInstanceData`); std's extrude moves its profile by the remainder transform (all of it
  when the profile isn't made inside the pattern), so a boss up to a slanted plane ends at its own height in each
  instance. An instance that lands off the block becomes a part of its own.
- **Clearance and tapped holes** (batch 026) belong to the newer hole: a legacy hole with `hasClearance` fails in
  std (it compares the V2 diameters, undefined there). The V3 hole's standard table must agree with the definition,
  `majorDiameter` included (Onshape's dialog fills it in). The hole steps from the clearance diameter to the tap
  drill where it enters the last part within its depth.
- **Section parts** (batch 025) delete every body they aren't given, sketches included. A jog section removes the
  side of its line towards the sketch plane's −x, whichever way the line's points run, not the +x side its
  documentation names; a partial section of a line across the whole part cuts as a full one does.
- **Revolve start bounds** (batch 024). With a blind end and a start bound up to a face or vertex, the revolve
  runs from the end's angle back to where the bound is first met going *forward* from the profile: a plane y = −5
  below a profile revolving about z from +x is met forward at 194.5° (the far side), so the revolve passes through
  the block below and stops 165.5° back. A positive offset turns the bound forward (10° short of a vertex behind
  the profile), as an end bound's offset does (further for the end).
- **Ruled surface vertex overrides** (batch 024). An override's ruling at a vertex blends into the default along
  the edge by 3t² − 2t³ (with `useCubicInterpolation`, the default): Onshape's far edge's poles fit it. Rulings in
  one plane make a planar face (Onshape's is a plane, its far edge a 51-pole B-spline approximation).
- **Wrapping onto a cone** (batch 024): isometric, rolling the plane round the cone's apex from the anchor's
  generator; the rectangle's image matches Onshape's to the probes' tolerance.

- **Derived sheet metal parts** (batch 023): with `preserveActiveSheetMetal`, `importDerived` merges the source's
  model (definition sheet with its wall and joint attributes, and its parts) and, unless it lands where it was,
  `sheetMetalTransform` moves the definition and rebuilds the parts, which stay active: a flange can be added to
  them. Each copy is its own model, even when two copies come from one source: their association ids are the
  copies' own. Placement at a base mate connector puts the source's mate connector (owned by the part) at the
  origin. Sheet metal parts can't be moved with `opTransform` (`SHEET_METAL_PARTS_PROHIBITED`).

- **Bend reliefs** (batch 016). Where a bend covers only part of its base wall's edge, the rest of the edge is
  cut back as the bend is, and each end of the bend gets a slot in the base wall: as wide as the width scale ×
  the thickness, reaching from the edge to the depth scale × the bend's inner radius at the centre of an
  obround's round end (a rectangle goes half its width further; a tear cuts nothing more). The depth doesn't
  depend on the thickness (1, 2 and 3 mm with a 1 mm radius agree), but triples with a 3 mm radius. The flange
  isn't notched.
- **Corner reliefs** (batch 016) are cut in the flat pattern, centred where the two bends' centre lines cross:
  round, a circle of radius scale × flat length / √2; rectangular, a square of side scale × flat length; sized,
  the given diameter or width. This fits the base wall's relief faces in every case to 1e-3 mm. The hole goes
  through the base wall, the flanges (unfolded back to the base wall's plane) and the bends, which Onshape builds
  flat, cuts and wraps round the bend's cylinder, leaving B-spline faces.

## STEP export

- **Part names** are written on each `MANIFOLD_SOLID_BREP` (`MANIFOLD_SOLID_BREP('c030.1', ...)`), as well as on
  the products. (Batch 001.)
- **Format details:** AP242, lengths in metres, written with STEP Tools' ST-Developer.
- **Repeatable:** exporting the same Part Studio twice gives identical data apart from the timestamp.
- **No seam edge on full cylinders:** an extruded circle is 3 faces, 2 circular edges and 2 vertices. Parasolid's
  periodic faces come through without the seam edge that OpenCascade's own modelling would add. (Batch 001,
  `tools/fixtures/step-summary.mjs`.) When OpenCascade reads the file it adds the seam, as a B-spline, while the
  engine's extruded cylinders have a straight seam. So `fs-fixtures` counts seam edges apart from the curve
  types. (Slice 5.)
- **Curved parts read back a little bigger.** Read through OpenCascade, the tight bounding box of an Onshape part
  with curved faces is 1e-7 m bigger on every side than the same part built by the engine; planar parts agree
  exactly. 1e-7 m is the parts' tolerance. The comparison allows 1 µm. (Slice 5.)
- **Some closed edges are split in two.** Onshape's STEP of a ring added to a cylinder has 8 circular edges where
  Onshape's own topology (the probe line) has 6. So for batches with probe lines, entity counts come from the
  probes, not the STEP. (Batch 002, c014.)
- **Voids** are written as `BREP_WITH_VOIDS`, a kind of `MANIFOLD_SOLID_BREP`, with the name on it. (Batch 002,
  c013.)
- **Only solids and sheets are exported**; construction planes and points aren't. A sheet is a named
  `SHELL_BASED_SURFACE_MODEL`. (Batches 002 and 003.)
- **The engine's own STEP** is AP242 in metres, with each part's name on its product and, like Onshape's, on its
  `MANIFOLD_SOLID_BREP`. OpenCascade writes shapes as millimetres unless the document's length unit is set to
  metres.
