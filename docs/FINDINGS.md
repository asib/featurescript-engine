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
- **Cones and spheres extended** (batch 039) are carried on along the surface, each point of the edge the distance
  along the surface's circle or line through it, square to the edge: round a quarter cone (radii 10 to 5), every
  point goes 5 mm round its own circle, so the new edge is a curve turning 0.5 rad at radius 10 and 1 rad at
  radius 5 (Onshape's box reaches exactly −10 sin 0.5); past its narrow end, 5 mm along the generators; past a
  sphere band's top, 5 mm along the meridians (to latitude 88.648°). Onshape's new edges are approximations, a
  little off that (areas within 0.03 mm²). A sphere extended round doesn't follow it (1.106 rad at radius 5).
  Past a sphere's top, Onshape keeps the old corner at the far end of the revolve as a vertex (the meridian there
  in two edges), though not at the near end, nor on a cone.
- **`evPlane`'s origin** is the middle of a box round the face in its plane's axes, but on each side where the
  face only touches that box at a point (a vertex, or where a curve turns) rather than along a straight edge, the
  box reaches 0.6% of its size further out (batch 004 c034, batch 024 c008–c010, to 1e-7). A filled right
  triangle with 30 and 12 mm legs along the axes gets (15.09, 6.036), not (15, 6); a ruled trapezoid whose long
  side runs along x moves only across it; symmetric faces (rectangles, discs) keep the box's middle.
- **Through all** (batch 007, c007) goes as far as the diagonal of the box round the Part Studio's parts (not
  construction or sketch bodies: a sketched 100 mm axis line in an earlier case doesn't count) plus 0.01 mm: a
  hole cut down through all from z = 12 makes a cylinder whose coordinate system sits at the far end, z =
  −2088.98596, with the parts so far 2100.97596 across.
- **Conic and asymmetric fillets** (batch 003, c013 and c014; batch 052) have a rational quadratic cross section
  from a point set back along one face to one along the other, the edge its middle control point. Its setbacks
  are the feet on the faces of a centre `d1` from face 1 and `d2` from face 2: `s1 = (d2 + d1 cos α) / sin α`
  along face 1 (α the angle between the faces; at a right angle `s1 = d2`). A conic fillet's centre is the
  circular fillet's (`d1 = d2 =` the radius), its weight `rho / (1 - rho)`. An asymmetric fillet's centre is
  `radius` from the face whose normal leans most along x, then y, then z (the face that orients fillet cylinders,
  batch 047) and `otherRadius` from the other at a convex edge, the other way round at a concave one (flipped,
  swapped); weighted as a circle's arc, `sin(α / 2)`: a quarter ellipse at a right angle (c014's 2 × 5 mm passes
  through (2, 5)(1 − 1/√2)), weight 0.5 at 60° (setbacks 3√3 and 4√3 for 2 and 5). At an obtuse corner with
  uneven radii a setback can come out negative (120°, 2 and 5 mm: −0.58 mm): Onshape fails (`FILLET_FAILED`).
  Equal radii make a plain circular fillet, a cylinder. Onshape's surface is a non-rational cubic B-spline swept
  along the edge, through the conic at its knots in its own parameter (to 1e-12, uneven knots too) with its exact
  end derivatives (`2w` times the leg). The knots start at the ends; each round, every span whose middle is more
  than 10 µm from the conic at the same parameter is halved and the spline fitted again: this gives all of
  Onshape's 13 knot vectors (1 span for rho 0.5, a parabola; 18 crowded at the ends for rho 0.9; 6, 8 and 10 for
  4 mm at rho 0.7 with radius 1, 4 and 10; 9 or 11 µm would miss some). Onshape's own volumes, areas and boxes of
  these fillets stray from its exported surfaces (which match the engine's) and from the exact conics: a 2 × 5
  section measures 2.143786 mm² along a convex edge, 2.144238 along a concave one, 2.146018 exactly, 2.147538 as
  exported.
- **Curvature fillets** (batch 053) have a non-rational quintic B-spline section on knots 0⁶ 0.5² 1⁶, the same
  for every size and angle in the oblique frame of the corner `c` and the two setback points `a`, `b` (the
  circular fillet's, or an asymmetric fillet's): as `c + α (a − c) + β (b − c)`, its eight poles are (1, 0),
  (1 − 0.3m, 0), (1 − 0.6m, 0), (α₃, β₃) and their mirror images, α₃ = 2(1 − m)/3, β₃ = 1/3 − 7m/15 (to 1e-11 in
  all ten cases). So it leaves each face straight (no second derivative), passes through the chord's middle moved
  `m` of the way to the corner (as rho moves a conic's shoulder), with no third derivative there. At m 0.8 β₃
  would be negative: it's held at 0.001, α₃ taking up the difference. At m 0.95 even that fails, and Onshape makes
  a cubic B-spline of 8 equal spans with the quintic's end derivative (3m times the setback) and shoulder, through
  some other curve.
- **Two conic or asymmetric fillets meeting at a corner** (batch 053, c013–c015: two top edges of a block, or a
  top edge and an upright one, with the third edge there left sharp) meet in a mitre: the block with each edge's
  own region taken away, as for one. Onshape's faces (17 edges, 8 faces, 11 vertices) and their areas match what
  that gives exactly (c013's fillets 153.7975 and 100.9583 mm² in its STEP; exact for its cubic sections 153.7967
  and 100.9582). Three filleted edges at a corner (c012) get a bicubic corner patch of 304 poles.
- **Fillets too big for their faces** (batch 003, c015): a 30 × 20 × 10 block's four upright edges rounded 15 mm.
  Onshape lets the fillets eat the faces between them: the result is the block with each edge's own region
  (between the edge and its arc, the arc's centre 15 mm in from both faces) taken away. The arcs of the corners
  along each 20 mm side cross at its middle, 0.858 mm in (15 − √200); the 30 mm sides shrink to lines; the corners
  at either end of a 30 mm side are rounded about one axis and become one cylinder. 4 faces, 6 edges, 4 vertices.
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
  plate whole, with two small cylindrical pieces where the hem arcs meet. Re-projecting the curves of tolerant
  edges onto faces before integrating (batch 026) put some of this part's bend faces a period away and lost 600
  mm³; it is skipped when it changes a face's area by more than 0.1%.

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

- **Body drafts on both sides** (batch 050): the part is split at the parting plane and each piece's steep faces
  turn about their edges farthest along the way it's pulled (the piece behind the plane against the pull): the top
  and bottom stay as they were and the part is widest at the plane. **Plane corners**: each drafted face keeps its
  sides in its neighbours' original planes, and the plane through the corner at the reference and those sides' far
  ends fills each corner.
- **Parting line drafts** (batch 049): from std 3013, `draft` hands `opDraft` the parting edges and sides
  (`inferReferences`, `inferReferenceOptions`) instead of the faces: each edge's two faces are ordered by
  steepness, the face's normal · (the edge's direction in it × the pull), the one along the pull first; one sided
  drafts take the first (or the second), symmetric and two sided ones both, the face along the pull at the
  negative or second angle. Each face turns about the plane through its edge square to the pull. Faces split from
  one face share its surface, so each needs a surface of its own before OCCT drafts it.
- **Twisted and scaled sweeps along an arc** (batch 048): the profile is carried round the arc's axis, turned
  right-handed about the path's direction and grown about the path's point in proportion to the distance along
  it. Onshape's surfaces lie up to about a micrometre inside that, as along a straight path (batch 019): a 6 × 4
  rectangle twisted a quarter turn along a quarter circle of radius 30 has 1130.611 mm³, against 1130.973 exactly.
- **Fillets' cylinders** (batch 047): z runs along the filleted edge as the edge runs in the loop (anticlockwise
  about the normal) of its neighbouring face whose normal leans most along x, then y, then z; x is y × z (x × z for
  an edge along y); the origin is where the edge's line is placed, brought onto the axis: an edge an extrude swept
  from a vertex has its line at the far end (a block's upright edges, at the top), other edges at their middle.
- **Unwrapping** (batch 046): `opWrap` from a cylinder's face onto a plane inverts the wrap: arc length round the
  cylinder from the anchor goes along the destination's anchor direction, height along the cylinder's outward
  normal × the source anchor direction (whichever way the face is turned). A face all the way round fails
  (`ROLL_FAILED`). A sheet extruded from an arc has its cylinder's origin at the near end and z along the extrude
  (a solid's is at the far end, z back).
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

- **Face blends between planar sheets** (batch 054). The section is a fillet's (batches 052, 053): two radii
  set the points back as an asymmetric fillet's do, `radius` the centre's distance from side 1 (`switchAsymmetric`
  swaps them); width control makes the chord between the two points the width, side 2's setback `widthRatio` ×
  side 1's. Unlike a fillet's, the conic sections are exact: Onshape's areas are the exact conics' (a 5 × 3 mm
  quarter ellipse 6.38175 mm long, its exported 4-span cubic 6.38120), and its STEP holds cubic approximations of
  them (the conic's own parameter, uniform knots, through the conic at them with its end derivatives: 4 spans
  for c003, 8 for the rho 0.7 conic). The curvature section is the fillet's quintic, pole for pole. Untrimmed, the
  sides are still trimmed and attached, and the blend runs on past their ends: 3 mm at each end of 30 mm sheets
  (the engine takes a tenth of the length, a guess from that one case). Detached, the sides stay whole and the
  blend is a sheet of its own. Sheets apart: std's `wallsNormalAlignment` (since V2844) tests both normals
  against the vector from side 1 to side 2 with the same sign, so with the upright sheet 2 mm above the level one
  it flips the level sheet's sense away from the blend, and Onshape refuses it with
  `FACE_BLEND_WRONG_RIGHT_WALL_SENSE`. The engine checks each side's sense (its normal, flipped if
  `flipSideNNormal`) against the side of it the other side goes. Batch 055: the untrimmed run-on is a tenth of the
  sides' extent along the blend at each end (6 mm for 60 mm sheets, 3 mm with a 2 mm radius, 5 mm for sides
  spanning 50 mm between them). A short trim blends only where both sides run (20 mm of sheets along x 0..30 and
  10..50) and notches each side there; a long trim blends over both and cuts them back along their length.
  Trimmed to the walls, sides of unequal extent give the blend slanted ends, the blend running along each side as
  far as that side does: in the blend's parameters (angle, distance along it) each end is a cubic Bézier leaving
  each corner along the section, as fast as the chord between the two corners is long (the poles' angle
  √(2/3) = 12.247 / (3 × 5) in from each corner for c004; along the blend 3t² − 2t³ of the way), but no further
  than across the whole section (batch 056 c009: 1.637 of the conic's parameter held at 1). `widthRatio` is the
  ratio of the section's distances from side 1 and side 2 (as the two radii set them), not of the setbacks
  (batch 056 c012: at 60°, ratio 2 sets the sides back 5.2372 and 6.5465 mm, the chord 6); the two agree at 90°.
  At 120° a ratio of 2 or 0.5 leaves one side no setback, and Onshape fails (`FACE_BLEND_FAILED`, 055 c010, 056
  c013). Sheets apart blend
  as sheets that meet do once std's flip is made up for. A width-controlled blend at 120°, ratio 2, fails in
  Onshape (`FACE_BLEND_FAILED`, c010); the engine builds it.
- **Mass properties of rational surfaces** (batch 055): OCCT's adaptive integration can settle on a wrong value
  over a whole rational face and report no error (an exact conic extruded 30 mm: 419.1726 mm² for 419.1589, and
  418.62 or 419.22 at other tolerances). Over each of 4 × 4 pieces of every face it converges (to 1e-11, and on a
  scaled sphere), so areas and volumes are integrated over faces split that way.
- **Curvature fillets past magnitude 0.86** (batch 055 c016–c020) keep the quintic on knots 0⁶ 0.5² 1⁶ (Onshape's
  STEP of batch 053 c004 re-approximates it as a cubic; `evSurfaceDefinition` gives the quintic), with P1 still at
  1 − 0.3m and the middle through the shoulder (`0.125 α₂ + 0.375 (α₃ + β₃) = (1 − m)/2`). Past m = 5/7, where
  the formula's β₃ = 1/3 − 7m/15 goes negative, β₃ is a fortieth of it the other way, (7m − 5)/600 (batch 056: m
  0.72, 0.76, 0.8, 0.82, 0.84, and 055's 0.86, exactly), with α₂ still 1 − 0.6m and α₃ through the shoulder. From
  0.88, where that α₃ would fall below β₃ (the inner poles crossing the diagonal), α₂ falls too (0.449283,
  0.367067, 0.1615265, 0.010739 at 0.88, 0.9, 0.95, 0.99; β₃ 0.001744, 0.00139225, 0.0005127, 0.003103): no rule
  found; the curves stay convex. OCCT's boolean with the NURBS-converted cutting tool can come back unchanged
  without an error (m 0.82): the engine checks the volume changed and cuts with the extrusion otherwise. Batch 059
  confirms 0.87 on the first rule and shows the second regime: from 0.879 (where α₃ would fall below β₃) α₂ and β₃
  both fall linearly with m, α₂ = 4.0667892 − 4.1108024 m and β₃ = 0.01722228 − 0.01758906 m, α₃ through the
  shoulder (0.88, 0.9, 0.92, 0.95, 0.97 to 4e-7), until β₃ reaches nothing at 0.979; 0.99 is something else.
- **Offset curves on faces, which way** (batch 059): by default into the face being offset across, the other way
  flipped, except where that face is planar and the edge's other face is curved: then out of it. A cylinder's top
  edge goes outwards across its top (batch 020 c008) but down its side (027 c001); a block's top edge and the edge
  where a fillet meets the top go into the face they're offset across. That is what the edge's direction × the
  face's normal gives if the edge runs as the curved face's loop has it (as Onshape's kernel may orient it); the
  engine's own edge directions don't follow that, so it applies the rule as stated. Where the offset leaves the
  face Onshape makes no curve and raises no error. Its offset of a circle across a plane is an approximation,
  0.0015 mm short of the exact circle.
- **Intersection curves split** (batch 059 c001, c002; batch 060 c002–c005): each closed curve starts where it
  crosses the seam of the cylinder it winds round (the second group's; the curve near x = −10 crosses the first
  group's seam twice and isn't split there), and is split wherever it touches an edge bounding either face. Where
  a cylinder of radius 10 about z crosses one of 6 about x, an extruded cross cylinder 20 mm long ends exactly
  where it meets the base, touching each curve at (±10, 0, ±6): two edges each, whatever made the base. 40 mm
  long it doesn't, and each curve is one edge from its seam at (±10, 0, 6). The curves run along n₁ × n₂ (the first
  group's outward normal crossed with the second's); a split curve's pieces start from the seam on its frame's −y
  side; the curves come in order along the cross cylinder's extrude. They are nearly parametrised by length (the
  middle by parameter within 0.02 mm of the middle by length). With the groups swapped (batch 061 c006, c007)
  the curves run the other way, their pieces still from the seam's −y side.
- **Extruded cylinders' coordinate systems** (batch 060 c002–c005): z points back along the extrude from the far
  end, and x is (world y) × z, whatever the sketch circle's own axes: cylinders about x sketched with their x along
  y or along z both have x along +z. (Batch 007's cylinders about z, x = −x for z down and +x for z up, fit the
  same; batch 061: tilted axes too.) About y, x is (world x) × z. The engine starts each closed intersection curve
  from that x.
- **Lofts between polygons tangent to a profile's plane** (batch 056 c003; batch 060 c006–c008): each side is
  one bicubic B-spline: along the edge, knots at thirds and the edge's points at the Greville abscissae (0, 1/9,
  1/3, 2/3, 8/9, 1); across, one cubic span on [0, 1], natural at a free end (P₂ = (P₁ + P₃)/2). The derivative
  across at the tangent end is, along the edge, the natural cubic spline (zero second derivative at the corners)
  through its values at the knots u: the chord to the other profile there, projected into the profile's plane,
  times the magnitude, as long; in the direction of the corners' directions blended by h = 3u² − 2u³ (20/27 and
  7/27 at u = 1/3; the directions, not the derivatives, batch 061 c010's square off the axis). An end condition at
  the last profile is the same, P₂ = P₃ − D/3 (061 c008, c009). It holds to 1e-6 for rectangles, hexagons and
  squares, at magnitudes 1 and 2. Onshape lists the sides first, then the caps, the sides in no geometric order.
  Between triangles turned to each other, magnitude 0.5, Onshape fails (`LOFT_FAILED`, 061 c011). Through more
  profiles (062 c008: three squares) the sides are cubic across with a knot at each inner profile, and the "chord"
  is the start (or end) tangent of the natural spline through the profiles' points, projected; between two
  profiles that is the chord. The inner knots are spaced nearly as the chords between the edges' middles
  (0.408254 there, Onshape's 0.408307, by a rule not found).
- **Flat patterns of bent parts** (batch 061 c001–c003): `qCorrespondingInFlat` of a flanged plate's top gives
  the base wall's face in the flat: on the definition sheet's plane (z = 0 for a sheet thickened up), cut back
  from the sharp corner of the definition (y = 32, the flange's outer face with inner alignment) by the bend's
  setback (3, the radius at the sheet), so 0–29 across; its plane's origin is its middle, (20, 14.5, 0). The bend
  lies flat at the neutral radius, r + 0.45 T, for 2.985 mm at 90°, then the flange: a 4 mm slot from 9.5 to 39.5
  removes the base (19.5 long), the bend's ring sector and the flange's 7 mm (237.13 mm³); a tab over the flange's
  far edge (38.985 in the flat) extends the flange in its plane, to z = 25.515 (310.30 mm³). The engine maps a
  region drawn in the flat back onto the definition's walls: rigidly on each wall's side of its bends' middles,
  back from where the wall stops, and each half bend stretched across to the setback, so a cut across the bend
  reaches the definition's sharp edge; the part rebuilt from the definition matches Onshape's exactly.
- **Boundary surfaces and fills** (batch 062): a boundary surface between two arcs is ruled between Onshape's own
  non-rational cubic fits of them: eight uniform spans, uniform in angle, within 0.001 mm of the arcs (so its area
  is 0.003 mm² under the exact ruled surface's, batch 005 c019). The lines joining their ends as v profiles change
  nothing. A fill of four lines round a non-planar loop is their bilinear patch, u back along the first edge from
  its corner with the second, v along the second.
- **Flat patterns, more** (batch 062): a slot through a channel's flat (across both bends) leaves two parts, both
  created by its sheet metal feature. A circle across a bend is wrapped onto the bend, its wall there B-spline
  faces. The flange's outer face in the flat is on the flat's far side, z = −2 with normal −z (origin at its
  middle, (20, 35.4845, −2)), and the top's at z = 0: Onshape's flat pattern lies on the far side of the
  definition's plane from the folded part.
- **Curvature fillets past 0.979** (batch 055 c019; 063 c001–c003): the section still passes through the
  shoulder, 0.125 α₂ + 0.375 (α₃ + β₃) = (1 − m)/2, but α₂ and β₃ follow a law not found: α₂ = 0.038195,
  0.0206525, 0.0107388, 0.005 and β₃ = 0.0006525, 0.0031158, 0.003103, 0.0014263 at m = 0.98, 0.985, 0.99, 0.995
  (α₂ = 1 − m exactly at 0.995; a cubic through them turns negative just outside them, so it isn't the law).
  Magnitude 1 is out of range (`PARAMETER_OUT_OF_RANGE`).
- **Polygon lofts normal to a profile** (batch 063 c005–c007): the derivative is along the normal, at each corner
  the magnitude times 0.9 of the chord along the normal plus 0.1 of its length (as the engine had fitted); along
  the edge it is blended between the corners by a rule not found: halfway it is the corners' mean, but at a third
  of the way it lies 0.2498 to 0.2695 of the way between them, more where it rises (smoothstep gives 0.2593).
  Onshape's sides are fits of eight C1 cubic spans along the edge, a cubic across, natural at the free end.
- **Fills of arcs and boundary surfaces of arcs** (batch 063 c008, c009, c011) are Onshape's own fits: the fill
  of four arcs is bicubic with four C1 spans each way, its edges' rows 0.05 mm off the arcs at the knots and not a
  Coons patch of them (0.1 off); a boundary surface normal to its first profile has the distance between the
  profiles as its derivative there, but not a natural free end.
- **`derivedMirror` in a Part Studio feature** (batch 060 c001): Onshape fails with "Execution error". It builds
  its source twice, once by `@convert` and again in `instantiate` (no loaded context), which Onshape evidently
  refuses for a build function defined in the studio (it won't derive one source twice in a Part Studio, batch
  023). It is meant for assembly mirrors.
- **`evPlane` of nothing** raises `CANNOT_RESOLVE_ENTITIES` (batch 060 c009, c010), not `CANNOT_RESOLVE_PLANE`.
- **Extruding in a sheet metal flat pattern** (batch 059 c008, c009) works from a feature: a sketch on
  `qCorrespondingInFlat` of a plate's top face, extruded with `domain` `FLAT`, runs `SMFlatOp`. The sketch plane
  is the flat face's `evPlane`, origin at its middle (std's `newSketch` keeps it for flattened planes): a 10 mm
  circle at (20, 15) cuts a quarter circle out of the 40 × 30 plate's corner at (40, 30) (2360.73 mm³). A tab
  drawn wholly past the plate's edge adds nothing. The engine treats a plate without bends as its own flat
  pattern: sketches on its flat faces are marked as lying in its flat (`qSheetMetalFlatFilter`, `qPartsAttachedTo`,
  `queryContainsFlattenedSheetMetal`), and `@opSMFlatOperation` drops their regions onto the definition sheet and
  cuts or joins them, leaving out added regions that don't reach it.
- **Spheres extended round** (batch 055 c012–c015): the new edge's top corner (radius 5) turns 0.200754,
  0.406117 and 1.106271 rad for 1, 2 and 5 mm (d/r 0.2, 0.4, 1), the bottom (radius 10) exactly d/r; between,
  within 1% of d/r for 1 mm, drifting further above as the distance grows (5% at latitude 47° for 5 mm). Onshape's
  edge is a fit with dyadic knots (tolerance about 1e-5 m). 10 mm fails with `EXTEND_SELF_INTERSECTION`.
- **Loft end conditions** (batches 055, 056) make each side of a two-profile loft one cubic span across: rows P0
  (the first profile), P1 = P0 + D₀/3, P2 = P3 − D₁/3, P3 (the last), where an end left free has no second
  derivative (P2 midway between P1 and P3, say). The chord c from a point of the first profile to the matching
  point of the last gives the derivative: along a vector v (the profile's normal pointing at the other profile,
  or a direction given), the magnitude times (0.9 c·v + 0.1 |c|) along v (squares 20 apart, corners 21.2132 apart:
  20.12132; a direction at 45° to circles, c005, to 3e-7 of the volume); tangent to the profile's plane, the
  magnitude times c's part in the plane. On a square the derivative takes its value at each corner and goes from
  one to the next as 3t² − 2t³ (off axis, c001: the corners' 20.29129 and 20.06155 exactly, the side between two
  equal corners flat where c's length dips). On a circle, along a vector, it is c's at each point (off axis, c002,
  to 5e-7); tangent to the plane, the periodic C² cubic spline through its values at 0°, 90°, 180° and 270° (the
  coaxial circles' radial 6 mm dips to 33√2/8 at 45°, which puts Onshape's volume 0.27% over the exact field's;
  055 c026, 056 c004 to 1e-6). Tangent to a square's plane (056 c003) the derivative has the projected chord's
  length at the thirds of each edge but turns from it (25.7° from the edge's normal at a third where the chord is
  18.4°): no rule found. Normal to a direction (`NORMAL_DIRECTION`) fails in Onshape (`LOFT_INVALID`, c006).
  Onshape's profiles are cubic Hermite fits (64 spans for a circle), and a square's rows rise and fall along each
  edge by up to 0.0008 mm about the exact construction's (0.05 mm³ of volume). Matching the tangency of the faces
  next to the profile (batches 057, 058) takes the chord rule along where those faces meet at each corner (a
  frustum's leaning corner edge carried on: 0.9 × 20.3999 + 0.1 × 20.445 = 20.4044 mm, Onshape's exactly), from
  corner to corner in proportion (its sideways part 1.3088 a third of the way, as linear interpolation gives), and
  on a circle across its one face (a cylinder's top: 2e-8 of the volume). Onshape's sides there are one cubic along
  each edge and bulge between the corners (up to 1.4e-4 of the volume). Matching curvature (batch 057 c007) makes
  the sides the tangency-matched cubics raised to quintics across, with the second row from the matched end moved
  so the curvature there is the face's along the loft: P2 = 2 P1 − P0 + (κ |r'|² / 20) n (κ the face's normal
  curvature, none for a plane): exact at the corners. Between them Onshape's sides (quintic along each edge too)
  bulge by a rule not found: the second row 0.015 mm higher mid-edge, and at twice the magnitude the tangent case's
  rise is half as much (3.6e-4 of the volume).
- **Lofts through more than two profiles** (batch 004 c038, 057 c003) are, across the profiles, the natural cubic
  spline through them (no second derivative at the ends) at parameters spaced as the chords between them (three
  coaxial circles of radius 10, 14 and 8: 14110.2910 mm³ against Onshape's 14110.2866; uniform spacing gives
  14111.36); an end condition replaces the natural end. A point can be either end profile: a square or a circle to
  a point is a pyramid or a cone (ruled), a point through a circle to a point the spline through them. To a point,
  Onshape's rows bulge between corner and middle of each edge by up to 0.003 mm (c004). A circle lofted to a
  square is split where the rays from its centre through the square's corners meet it, an arc to each edge (batch
  004 c039: 12 edges, 8 vertices, the volume to 4e-8).
- **Sweeps twisted or scaled along a chain of lines and arcs** (batch 057 c009, c010): the twist and scale grow
  with the distance along the whole path (at the joint of a 20 mm line and a quarter circle of radius 20, the line's
  share, 0.389, of the quarter turn); the profile is carried along each edge in turn, and each edge's piece is one
  face per profile edge, kept a B-spline even where it is flat.
- **A torus sheet extended** (batch 057 c011, c012): round the torus, every point turns the distance over the
  torus's innermost radius (R − r: 5/7 rad for 5 mm, radii 10 and 3), so the new edge is a tube circle; round the
  tube, the distance over r.
- **Isoclines of a torus tilted to the direction** (batch 057 c013): n·d = sin(angle) for the outward normal, two
  closed curves round the torus (for each angle round it, two tube angles), 79.44052 and 47.10938 mm long for 20°
  with the direction 30° off the axis (Onshape's to 2e-7). Split along them (batch 058 c005), the torus is two
  faces, the bands between the curves either way round the tube, with 2 edges and no vertices (Onshape's faces
  have no seams); a general split cuts the bands up at the torus's seams (5 faces), so the engine builds each band
  from its loop in the torus's parameters.
- **Replace face** (batch 058): a planar face whose neighbours lean is replaced by extending them, the solid
  grown by the region inside all their planes and beyond the face, then cut by the new surface (a trapezoid
  prism's top, exactly); a plain cylinder's side replaced by a parallel cylinder off its axis is that cylinder
  between the caps.
- **A sweep locked to a cylinder's side along a circle round its axis** (batch 058 c004) turns the profile with
  the cylinder's normal: a revolve about the axis.
- **A draft parted by a cylindrical sheet** (batch 058 c001) works as a plane does: the part is split by the
  sheet, the piece on the pull side has its steep faces turned about their top edges (reaching 0.26 mm out where
  they meet the sheet, 4.96 mm below the top), and the pieces are joined again, leaving a cylindrical ledge on the
  sheet (11 faces, the volume to 6e-9).
- **Spheres extended round, more bands** (batch 056 c019–c021): no per-point rule can do: latitude 60° turns
  exactly d/r on a band from 30° to 60° but 1.106 rad on one from 0° to 60°, and a band from −30° to 30° turns 1.1%
  over d/r at both corners and 0.2% under at the equator: Onshape's new edge depends on the whole band.

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
