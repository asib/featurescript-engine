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
  feature. Caught, either error's message is just "Execution error". (Batch 011, c009, c014.) So is a call no
  overload takes, at run time: batch 062 c003's boundary surface without `uReference`, for which std finds no
  `createProfileConditions` taking its arguments.
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
- **Up to next stops at the first face of any part**, whatever the boolean scope, even when the profile lies on a
  part's face (batch 004, c026), but passes by a part the profile starts inside (batch 148 c002).
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
  side runs along x moves only across it; symmetric faces (rectangles, discs) keep the box's middle. A curved edge
  that is a B-spline reaches the box with its control points, as Parasolid boxes curves (batch 116 c004: an aligned
  section's leftover piece of a plane, bounded by a section curve that is a cubic in Onshape, the cubic through the
  ends of the engine's quartic with its end derivatives; its lowest pole sets the box's side, 0.05 mm past the
  curve, which is why the engine was 0.025 mm off before, batch 037 c004).
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
  mm³; it is skipped when it changes a face's area by more than 0.1%. Each hem's end is where its inner arc meets
  the mitre plane, carried out radially: y = 30 + cos θ − 0.0141 round the bend, so the hems meet corner to corner
  and leave the square between them empty. Std then adds a patch at the hem's root (`adjustToAvoidStepFaces`), up
  to a plane through the plate's corner and the end's point 45° round, which only lies past the end for the first
  0.83°: a sliver 0.014 mm across, between the plate's corner edge and the hem (0.058 mm², a cylinder in Onshape's
  STEP). It is built from an arc sheet extruded at radius 3 and scaled back to radius 1 about its axis
  (`scaleNonuniformly(1, s, s)`): OpenCascade's general transform converts the cylinder to a rational B-spline and
  fits the edges' curves on it loosely, leaving them 1e-4 m tolerant, which swallowed the sliver (the plate's
  corner was cut back 0.0063 mm instead); the engine projects them again from the exact 3D curves (1e-7). The
  hem's end, split where the patch touches it 45° round, is one edge again in Onshape: the engine joins edges left
  on one ellipse after a boolean, as OpenCascade's same-domain unifier doesn't. Onshape's end face is its own
  cubic fit (two spans, its poles up to 0.05 mm off the true arc): the volume is 2.3e-6 off the exact one. Its
  slivers are B-splines (batch 115 c002: 0.057553 mm² each; the engine's planes 0.057552), which its STEP writes as
  cylinders of other areas (0.0572, 0.0584).

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
  B-spline faces (matched since batch 077).
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
  definition's plane from the folded part. In the engine's terms (std puts this plate's definition on its top,
  z = 2) the flat is the folded wall moved a thickness further towards its material: faces on the definition's
  plane lie on the flat's near face (z = 0), faces a thickness off it on its far face (z = −2). The engine builds
  that flat as a hidden body of each model (walls cut back by the setbacks and laid flat, bends their flat length
  between), carrying the model's association attribute (std finds a flat sketch's model by it). Batch 064: a 60°
  bend's flat starts the setback (the sheet's radius times tan 30°) short of the definition's corner, so the
  base's flat face ends at 29.42265; a flange bent down and a channel's other flange lie as that model has them.
- **Curvature fillets past 0.979** (batch 055 c019; 063 c001–c003): the section still passes through the
  shoulder, 0.125 α₂ + 0.375 (α₃ + β₃) = (1 − m)/2, but α₂ and β₃ follow a law not found: α₂ = 0.038195,
  0.0206525, 0.0107388, 0.005 and β₃ = 0.0006525, 0.0031158, 0.003103, 0.0014263 at m = 0.98, 0.985, 0.99, 0.995
  (α₂ = 1 − m exactly at 0.995; a cubic through them turns negative just outside them, so it isn't the law).
  Magnitude 1 is out of range (`PARAMETER_OUT_OF_RANGE`). Batch 064 (0.975 to 0.9975) shows constraints switching
  on and off: from 0.9925, α₂ = 1 − m exactly (so α₃ + β₃ = 1 − m; β₃/(1 − m) = 0.3217, 0.2853, 0.1471 at 0.9925,
  0.995, 0.9975); from 0.9825 to 0.9875, α₃ = 2(1 − m)/3 exactly with α₂ + 3β₃ = 2(1 − m), the first regime's
  family split otherwise (α₂ = 0.027748, 0.0206525, 0.0160015); at 0.975 and 0.9775, α₂ still the third regime's
  but β₃ about 7.3e-5 (0.000073, 0.000074) where its line gives 1.7e-4 and 2.9e-5; 0.98 and 0.99 lie between.
  The construction looks like a constrained optimisation whose objective isn't known. Batch 113 fills in every
  0.00125: interpolating the samples every 0.0025 put the section within 1.2e-3 mm of Onshape's at each midpoint
  (worst at 0.98375, where α₂ turns), 1e-5 mm in the smooth stretches. At 0.99875 β₃ is below nothing (−0.0001145,
  α₂ = 1 − m), the section dipping 3.7e-9 m past the faces. The section isn't the same in the corner's frame at
  every angle there: at 0.99 a 60° corner's and a 120° corner's are alike (α₂ 0.0122813, β₃ 0.0025927, to 1e-6),
  a right angle's different (0.0107388, 0.0031030), as if it depended on the angle through cos² (batch 114: 45°
  and 135° alike too, α₂ 0.0385481, β₃ −0.000822; 75° 0.0106566, 0.0031293, nearer the right angle than 60° is;
  a 60° corner's from 0.975 to 0.99875 differs from a right angle's up to 0.9925 and is the same from 0.99375). None of the
  energies tried (∫κ², ∫κ'², max κ, ∫|C''|², ∫|C'''|²) under the constraints seen reproduces it. From 0.9975 the
  section lies within micrometres of the corner: OpenCascade's distances and booleans fail on such a fillet face
  (a point on it measures 4 mm from it), so the engine can't cut it, and can't compare 0.98375 and 0.985 with
  Onshape's model shape for shape (their volumes, areas and boxes match Onshape's probes to 3e-6).
- **Polygon lofts normal to a profile** (batch 063 c005–c007): the derivative is along the normal, at each corner
  the magnitude times 0.9 of the chord along the normal plus 0.1 of its length (as the engine had fitted); along
  the edge it is blended between the corners by a rule not found: halfway it is the corners' mean, but at a third
  of the way it lies 0.2498 to 0.2695 of the way between them, more where it rises (smoothstep gives 0.2593).
  Against a smoothstep blend of the corners' values the leftover is the same on every edge of a loft, symmetric
  and nothing at the ends and middle (0.0017, 0.0022, 0.0014 at 1/8, 1/4, 1/3 for the off-axis squares; 0.0003 at
  1/3 for the rectangles), so it looks like Onshape's fit of the sides; the engine blends by smoothstep.
  Onshape's sides are fits of eight C1 cubic spans along the edge, a cubic across, natural at the free end.
- **Fills of arcs and boundary surfaces of arcs** (batch 063 c008, c009, c011): the fill of four arcs is
  Onshape's own fit, bicubic with four C1 spans each way, its edges' rows 0.05 mm off the arcs at the knots and not
  a Coons patch of them (0.1 off). A boundary surface through two arcs each way is the bilinearly blended Coons
  patch of its four boundaries (Onshape's eight- and four-span fits of the arcs), to 7e-7 mm (c009). One normal to
  its first profile's plane is quadratic across: it leaves the profile along the chord to each point's partner
  with the chord's part across the profile within the plane taken out (the part along the normal is the chord's,
  20 mm; along the arc, within 0.01 of the chord's in Onshape's fit), and the far end's derivative is twice the
  chord less that (c011). Tangent to the plane, the same with the part along the normal taken out: two arcs over
  each other leave nothing at their ends, and Onshape fails (`CANNOT_MAKE_BSPLINESURFACE`, c010). Batch 117: normal
  at the second profile alone, the mirror image (c003); at both, the cubic leaving each profile with its own
  (c001). Along a direction given, the derivative leaving the profile is the chord less its part across the profile
  within the plane of the direction and the profile's tangent, as normal to the profile, but the free end isn't the
  quadratic's (c004); square to a direction the profile doesn't lie square to, Onshape fails
  (`BSURF_INCONSISTENT_CONDITIONS_ERROR`, c005). Magnitudes other than 1 aren't the chord's parts scaled: where
  the chord lies along the normal the derivative is the magnitude times it, but elsewhere it falls short (magnitude
  2: 39.2309 where twice the chord's part is 40) and the other end moves to make up for it (c002, c006: 0.5 and
  1.5 shift both ends by 0.3845, half of magnitude 2's 0.7691); no rule found. From a level sheet's edge to an arc (batch 160 c006, c007), the
  curve across the middle at magnitudes 2 and 0.5 is as high as at 1 (its poles 0, 0, 4.667, 9 up), but its middle
  poles both shift along the derivative by 0.1107 times (1 − magnitude), so it leaves the sheet at 19.67 for 2, not
  20. Their poles (batch 161 c008, c009): the rows next to the surface's ends are the magnitude-scaled quadratic
  plus the correction from the ends exactly, as at magnitude 1; the interior rows' middle poles both shift by a
  further term, along x the same whether the arc rises 4 or 2 (−0.2977 at the third of seven rows, −0.0172 at the
  middle), along y in proportion to the row's chord (−0.0351 of it). So the term comes from the sheet's edge and its
  parametrization, not the arc; what it is isn't found. Row by row of poles it is linear in
  the magnitude: the leaving derivative is m times the magnitude-1 one plus (1 − m) times a small term of its own (at
  the third row (0.0043, 1.6778, −0.0024) from c002 and c006 alike, rows from the ends 0, 1.678, 0.438, 0.878, 0.716
  along y, oscillating as an interpolating fit's poles do), the far end's the same with the second profile's own
  term; with the far end free (c002), its derivative is twice the chord less the leaving one, plus (m − 1) times the
  chord's part along the normal. What the small term fits isn't found: halfway along it comes to 0.770 along y,
  20 sin²θ for the chord's angle θ off the normal there (0.769), but along the profile it wiggles (1.05 at an
  eighth, 0.72 at a quarter) where 20 sin²θ climbs smoothly (0.15, 0.43). Between two straight lines (batch
  121) it is simple: each curve across is the cubic leaving a conditioned line with the magnitude times its
  projected chord (10 and 40 at 0.5 and 2, whatever the second line's height), along a direction given the same
  projection (onto the plane of the direction and the line), and reaching a free line along the chord itself, not
  the quadratic's derivative; lines in one plane make a plane. So the free end between arcs depends on more than
  the chord at that point.
- **A survey of B-spline cases** (batch 065): a sweep twisted or scaled along a fit spline matches, within 1.0e-4
  (twisted) and 2.5e-4 (scaled) of the volume, a loft through copies of the profile carried along the path by the
  rotation-minimising frame, turned and grown in proportion to the path's parameter (by its length, the scaled
  one is 1.4e-3 off). Isoclines on a loft's B-spline sides are one wire round it, an edge a face from corner to
  corner, running along direction × normal (anticlockwise about the direction), traced to 1e-6. A sheet lofted
  between arcs extended past one of them is its surface carried on along the rulings (Onshape's is degree one
  across, its knots running past 1), bounded the distance on along each ruling (its fitted edge within 0.0014 mm),
  its straight sides keeping vertices where the old corners were. An arc trimmed by a circle stays an arc, from
  the kept end (by the help point) to where they meet. An offset up a sphere from its equator fails in Onshape.
- **A second survey of B-spline cases** (batch 066): an isocline split cuts each of a loft's B-spline sides in two
  along its isocline (the engine draws the traced curve on the surface and splits with the general splitter, so
  the cuts meet at the shared edges). A curve extended straight on up to another curve runs to where the ray first
  meets it (an arc up to a circle). An edge offset across a B-spline face is the geodesic offset (its middle the
  distance up the face along the geodesic square to the edge), trimmed where it crosses the face's edges, running
  as the face's loop runs the edge. A fit spline takes a start derivative alone; a sweep twisted along a line and a
  spline after it fails (`SWEEP_FAILED`). A ruled surface normal to a B-spline face from one of its edges faces
  along the edge as the face's loop runs it × the ruling. A lofted sheet extended past its straight side carries
  its arcs on straight, along the surface, by the distance. Onshape's loft runs its edges along its sides' u and
  lists the sides before the caps; the engine now sews its exact sides first, so its faces come in that order.
- **A third survey** (batch 067): an `fCone`'s top circle offset down its side makes no curve in Onshape (no error):
  its offset evidently goes off the face, though an extruded cylinder's top edge goes down its side (batch 027) and
  `fCone` revolves its profile; the rule isn't found. The edge where a cylinder's top round meets the top, offset
  across the round, is the torus's parallel at tube angle d/r (radius 7.9177 for d = 2, r = 4), Onshape's closed
  curve starting 7.5° round. An arc extended with its curvature up to a circle meets it within the curvature blend
  (y = 9.9925, between the arc and the straight line). A ruled surface normal to a cone from its top circle is a
  conical band 4 mm along the normal (Onshape's a fit, degree one across). A block's top replaced up to a sheet
  lofted between arcs above it fails in Onshape (`DIRECT_EDIT_REPLACE_FACE_FAILED`).
- **Sheet metal checked in batch 068**: walls tangent to a rolled wall (sheet metal extruded along a line, an arc
  and a line) are parts of their own (ripped at the joints), so a hold face on a flat wall isn't on the rolled
  wall's part (`SHEET_METAL_UNFOLD_HOLD_ENTITY_NOT_ON_PART`, in Onshape and the engine). An open box shelled from
  a block isn't recognized as sheet metal (`SHEET_METAL_CANNOT_RECOGNIZE_PARTS`; three sharp walls meet at each
  corner), though converting the block into an open box works (batch 012).
- **A flange beyond a rolled wall** (batch 069): a 10 mm flange from a half tube's straight edge runs in towards
  its axis, joined by a bend; std finds the bend's angle from a detached fillet of the definition's edge
  (`opFillet` with `createDetachedSurface`: only the fillet's surface, as a sheet). Unfolding the rolled wall
  holding the flange lays it straight on from the bend, its middle's length (65.94 mm for radius 20, thickness 2);
  `sheetMetalRefold` of the part leaves it unfolded; unfolding everything lays the part flat. The bend: in the
  plane across the edge its circle (the sheet's radius, 1 here) touches the flange's line on the wall's side and
  the rolled wall's circle (inside it, the bend being on the wall's concave side: its centre 20 − 1 from the axis,
  at (18.9737, 1)); the rolled wall is cut back to where the circle touches it (0.0527 rad, its faces spanning
  3.0889), the flange to where it touches its line, and the bend turns between the two, 1.6234 rad (90° and the
  cut-back); the flange's far end is its distance from the rolled wall's outer face (x = 22 − 10). The engine
  builds the folded part so exactly. Unfolded, the rolled wall lies flat from where the bend leaves it (the
  tangent point, not the definition's corner), as long as the arc left past it at the neutral radius, and the
  bend unfolds against the wall's tangent plane there; so the unfold, the refold and the whole flat match too.
- **Sketch conics extended** (batch 070 c003, c004): a conic (rho 0.7, 20 mm) becomes the clamped cubic spline
  through it at its knots, in its own parameter (0 to 1), with its exact end derivatives, as a conic fillet's
  section does; every span whose middle lies more than 1 µm from the conic is halved and the spline fitted again
  (its knots 1/32 apart at the ends, 1/8 in the middle; 0.69 to 0.91 µm would do). Straight on, a span after a
  triple knot, as for any cubic. Curving on, 10 mm is within the blend: the extension is one span of the blend's
  polynomial (p₀ + p₁x + p₂x²/2 − p₂x³/(6Δ)) as far as 10 mm along it, its knot at the old end single. Its Δ is 1,
  the conic's whole parameter range, where turning 3° would take 1.57: the blend runs on for at most the curve's
  range (one case shows it). std's `evCurveDefinition` of a sketch conic isn't a `BSplineCurve`.
- **A sketch's Bézier comes with its control polygon** (batch 071 c008): construction lines between its points, each
  a wire body as the curve is, so a curve made of the sketch's edges is closed (the Bézier, then the lines in order)
  and can't be extended: `MOVE_CURVE_BOUNDARY_EXTEND_REQUIRES_OPEN_CURVE` (batch 070 c001, c002, a quadratic; 071
  c009, a quartic). That curve keeps the sketch's edges in their order and directions.
- **Half a torus tilted to the direction** (batch 070 c005): its 20° isoclines are two open curves from one cut end
  to the other, each running along n × d (the outward normal crossed with the direction).
- **Both sides of a curved parting sheet, matching at it** (batch 070 c006): `BODY_DRAFT_MITER_FAILED`. Each side's
  faces turn about their far edges and meet the curved sheet at different lines.
- **Sweeps locked to curved faces** (batch 070 c007–c009): round a frustum's top circle locked to its conical side,
  and a square tube frame along a cylinder's rim locked to its side, are revolves. Up a half cylinder's straight
  edge locked to its side, the profile is carried along the edge; Onshape's sides are B-splines, linear across
  each profile edge and a cubic of evenly spaced poles along the path (its STEP writes them as planes).
- **A flex PCB unfolded** (batch 070 c010): `pcbEnd` with every bend unfolded holding the plate's top keeps one bend
  line on the part: from (39.864, 0, 0.2) to (39.864, 30, 0.2), 90°, radius 1, bending up. It comes from the flat
  pattern's bend centreline, a wire body with the bend edge's association attribute, on the flat's face where the
  definition lies, along the middle of the bend's strip, running along that face's outward normal × the way
  across the strip; `evSheetMetalFlatTransformation` of the top is the plate's move into the flat (down a
  thickness).
- **Half a conical shell** (2 mm, its sides parallel) isn't recognized as sheet metal:
  `SHEET_METAL_CANNOT_RECOGNIZE_PARTS` (batch 070 c011).
- **A hole cut in a rolled wall's flat** (batch 070 c012): an 8 mm circle in the middle of a half tube's flat (the
  sketch on `qCorrespondingInFlat` of its outer face) wraps back onto the tube at the neutral radius: the faces
  lose (22/21) and (20/21) of its area (Onshape's 0.017% more: its fit of the curve), and the hole's wall is a
  B-spline across the thickness.
- **Face blends limited by planes** (batch 071 c001, c002): each limit plane keeps the side its normal points to;
  the blend runs where all of them do (planes at x = 10 and x = 20 both facing +x: x ≥ 20). The sides are notched
  only there, meeting at their old edge beyond, and a planar face closes each end a plane limits, between the
  sides and the blend (r² − πr²/4: 5.365 mm² for 5 mm). A cap sheet across the middle of a blend, the sides running
  on past it both ways, fails: `FACE_BLEND_INCONSISTENT_DATA` (c003).
- **Face blends between a floor and walls standing on it** (batch 071 c004–c007): along the walls' base curves
  offset the radius over the floor, a quarter cylinder along each line and a quarter torus round each arc (a full
  one round a cylinder), inside or outside by the wall's flip; the floor keeps only what lies beyond the offset
  curves on the blend's side, the walls what lies more than the radius up.
- **Half a tilted torus split at its isoclines** (batch 071 c010): two toroidal faces (349.35 and 242.82 mm²), the
  curves running end to end, the end circles each in two arcs.
- **A flex PCB with one of two flanges' bends unfolded** (batch 071 c011) keeps both bend lines; where the bends meet
  at the corner each line stops at the other's setback (from (39.864, 0) to (39.864, 29), and from (39, 29.864)
  to (0, 29.864)).
- **A hole cut in the flat of a rolled wall with a flange** (batch 071 c012): a 6 mm circle in the middle of the flat
  falls in the rolled wall and wraps back onto it at the neutral radius, as on a lone rolled wall (070 c012); the
  flat is the part with every bend unfolded and the rolled wall unrolled from the bend's tangent point. Its areas
  0.016 mm² over the exact wrap's, as there.
- **Bodies cut from sheet metal** (batch 072 c003–c005): std cuts by the outline of the tool's part within the sheet
  (`opCreateOutline`), projected onto the definition along its normal, and cuts that straight through: a sphere of
  radius 6 about a plate's top makes a cylindrical hole of radius 6; a cylinder leaning 30° makes a hole of its
  two ends' ellipses and the lines joining them (Onshape splits it at points of its own); a box across a rolled
  wall makes a hole whose sides run radially, as wide as the box at the wall's inner face (±14.48° for a 10 mm box
  on radius 20).
- **A cut list of a tube along an arc** (batch 072 c001): Onshape's table has no length, and angles of 7e-9 and
  6e-8 rad: its curves drawn on the swept faces (its torus's periodic way) evidently run round the tube.
- **Sketch fit splines** through three and four points (batch 073 c001–c003) are the engine's: the clamped cubic
  through them at their chord lengths' fractions with natural ends, the same knots and poles. A tube swept along
  the three-point one (072 c002) differs only by Onshape's fit of the sweep: its top face 10 × 63.8446 mm² where
  the exact sweep's is 10 × 63.8271, its volume 3e-4 over.
- **A sweep twisted along a line and a spline** fails: `SWEEP_FAILED` (batch 072 c006).
- **No offset curve on a sphere** (batch 072 c007): a hemisphere's equator offset 3 mm up its sphere gives nothing,
  as a cone's top circle down its side doesn't (067 c001).
- **A variable conic fillet** (batch 072 c009): rho 0.7 from 2 to 5 mm along a block's top edge, smoothly: the
  fillet's area is the exact rational surface's (246.5854), the top and front lose exactly 40 × 3.5 mm², the end
  faces 0.002 and 0.003 mm² more than the exact conics' (Onshape's cubic fits of its sections).
- **Isoparametric curves on a quarter torus** (batch 073 c004, c005): three in the U direction give one, the tube's
  circle halfway round (the others lie on the ends); three in the V direction the circles at 60°, 180° and 300°
  round the tube, as the engine's. Trimmed, the face is periodic round its tube only ([false, true]); its surface
  both ways. Onshape's V curves run from the y axis to the x axis and its tube circle starts on the inside.
- **Variable fillets beyond a conic** (batch 073 c006–c009): curvature (magnitude 0.5) and conic sections changing
  smoothly or straight, at 90° and 60°, set the faces back as constant fillets do (1/tan(θ/2) times the radius);
  asymmetric, `vertexRadius` sets back the face that isn't leading (a block's front 2 to 4 mm, its top 3 to 5 by
  `vertexOtherRadius`), the other way round from a constant asymmetric fillet. The fillet faces' areas are the
  exact surfaces'; the end faces differ by Onshape's fits of the sections (to 0.013 mm²).
- **A loft from a square to one turned 45°** (batch 074 c001): each corner goes to the one 45° anticlockwise of it
  seen along the loft (from (10, 10) to (0, 7.07)); the side edges are lines. Connections given every vertex at a
  corner (the sketch lines' ends with the region's) fail with `LOFT_CONNECTION_MATCHING` (c002–c004); guides that
  miss a profile with `LOFT_GUIDE_PROFILE_INTERSECTION` (c005); circles of radius 3 at the ends of a quarter circle
  of radius 20, lofted along it with 5 sections, with `LOFT_FAILED` (c006).
- **Loft connections and guides** (batch 075): a connection of one region vertex on each profile matches those
  corners and the rest in turn (a square to one turned 45° either way; a square to a smaller one a quarter turn
  round: ruled B-spline sides, 3333.33 mm³). Straight guides along a square loft's corners leave it as it is; an arc
  guide bulging out from one side's middle curves that side only (378.19 mm²), the others staying planes. Along a
  straight path, circles of radius 10 and 5 loft to a curved side (5493.27 mm³, the cone's 5497.79); along a
  quarter circle, with the far circle facing either way, `LOFT_FAILED`.
- **Locked to a sphere** (batch 076 c001, c002): a sweep, or a frame, along a quarter of a tilted great circle,
  locked to the sphere, is a revolve about the circle's axis.
- **A quartic extended** (batch 076 c003, c004): a fit spline raised to degree 4 goes straight on as four evenly
  spaced poles after a fourfold knot, over 10 / |C′| of parameter; curving on, the same, its end having no
  curvature.
- **Offset curves extended** (batch 076 c005–c007): extended and trimmed 2 mm at each end, a block's top edge's
  offset runs from x = 2 to x = 38; a round's arc offset into the top, extended or splitting, makes nothing (it
  goes off the face, the top being planar and the round curved).
- **A thin loft's ends trimmed** (batch 076 c008): the walls run on to the profiles' planes and stop flat there
  (end rings of 86.71 and 45.48 mm² for a 1 mm wall from a 20 mm square to a 10 mm one).
- **Sheet metal lofts' facets and bend ends** (batch 044, looked at again): arcs of radius 20 and 10 (quarter
  circles) are each cut into 8 equal chords, the fewest keeping the larger arc's sagitta within the 0.1 mm chordal
  tolerance (0.096 mm; 7 would give 0.126). Where a bend meets the profiles at a slant (c002: a skew quad split
  along its shorter diagonal), each wall's tangent line runs on to that wall's own facet edges (the inner one from
  (29.8031, 0, 0) on the profile to (0, 9.9344, 19.8687) on the facet's edge x = 0), and the bend's ends join the
  two walls' corners with curves bulging beyond them (its inner face 8.7292 mm², more than any interpolation
  between the corners gives), closed by a twisted B-spline face each.
- **Sheet metal loft matches** (batch 077 c001–c003): `evTessellatedLoftMatches` gives the profiles' vertices
  only, an arc's tessellation points not among them (the loft cuts arcs into chords between matches itself). Each
  item is an edge and a parameter, a vertex too: on the edge ending there at 1, the first on the first edge at 0.
  Vertices pair by least total distance, so an L (0, 0)-(30, 0)-(30, 20) to a line (0, 10)-(40, 10) fans its corner
  and its end both to the line's end; a skew quad between two lines takes its shorter diagonal as a match of its
  own, between its two sides' pairs.
- **Slanted bend ends** (batch 077 c004, batch 044): the end face is ruled radially, each line square to the axis
  from the inner cylinder to the outer at one angle round it, so the inner and outer end curves lie at the same
  height along the axis at every angle. On the definition's side the curve is where the plane holding both walls'
  edges at the end vertex cuts the cylinder (044 c002's ends, c003's, c001's near end, 077 c004's both; offsets
  within 8e-6 mm). A single Bézier for a 13° bend (handles within 2e-4 mm of a circle's cubic carried onto the plane
  along the axis), a piecewise cubic for a 90° one. At 044 c001's far end, where the corner on one wall lies past
  the vertex along the axis and the other level with it, Onshape tilts the plane about the corners' chord into the
  bend instead: it meets the bend's edge 0.48 mm inside the vertex (h = 0.98(1 − cos θ) − 0.48 sin θ from the
  first wall, against 0.5(1 − cos θ) at the near end). Why, one case doesn't show; the arcs' far ends (044 c004)
  agree with it (none bulges past its corners).
- **A sheet metal loft added to a model** (batch 044 c005): std joins the loft's sheet to the model's definition
  sheet with a surface union, then makes bends of the two-sided edges `qCreatedBy` its loft. The joint edge, merged
  from the plate's edge and the loft's, must count as created by both (the engine's merged entities now keep every
  creating feature): the plate's far edge lofted to a line 20 mm up and 10 mm out becomes one part with a bend
  (4123.348 mm³).
- **Where a slanted bend's end tilts** (batch 078 c001–c003, c005): at the end of the bend's edge where neither
  wall's edge at the vertex leans in along the bend (one square to it and the other leaning out, at either end
  of the edge, or both leaning out, as at a square frustum's top corners) Onshape tilts the plane of the end curves
  about the corners' chord into the bend; where one leans in (044 c003's near end, with the other leaning out) or
  both do, it keeps the plane of the walls' edges. The tilted plane meets the bend's edge 0.48 mm inside the vertex
  for the L's (slope 1:2, radius 1, 2 mm thick, 90°), about 0.45 for the frustum's (from its faces' areas). What
  sets it, these don't show.
- **Sheet metal loft connections and closed profiles** (batch 078 c004, c005): a connection holds a pair of
  vertices in the least-distance matching, which runs in stretches between connections; a skew quad still takes
  its shorter diagonal. In each stretch a vertex is given on the edge starting there if it starts the stretch, else
  on the edge ending there, or as itself where the stretch has no edge of its profile; each connection ends its
  stretch as a match hidden from view, then starts the next. An L's corner connected to a line's start leaves the
  facets on either side of the connection in one plane, merged into one wall (the same part as without it). Two
  closed squares run anticlockwise about the way from the first to the second, so the sheet faces out and its
  thickness goes outward; a profile turned to run so keeps its first edge first, run backwards, and the rip is at
  that edge's (new) start: a rectangle's left side's second corner.
- **Variable non-circular fillets of concave edges, arcs and two edges** (batch 078 c006–c008): a concave edge
  gets the same wedge as a convex one, added rather than taken away; two edges apart each their own. Along an
  arc between a plane and a cylinder about it, each section is a straight edge's, its setbacks following the law
  over the arc's angle; Onshape's surface is a cubic fit of them (degree 2 across, its sections conics), straying up
  to 6e-4 mm off the cylinder, and runs a little past the arc's ends.
- **Face blend caps, cliff edges and hold lines** (batch 079 c001–c004): a cap past the end of one side, within the
  other, is inconsistent data (as one in the middle is, batch 071), flipped or not. A cliff edge nearer the corner
  than the blend would reach holds the ball: on a side 3 mm high, a 5 mm blend is a cylinder resting on its top
  edge and touching the other side sqrt(21) mm out, and the side under it goes. A tangent hold line on a side sets
  the blend back to it (4 mm up, a 4 mm blend), the side between going too; that blend's surface is a B-spline.
- **`moveCurveBoundary` needs its help point**: one naming no vertex (an elliptical arc from parameter 0 to π/2,
  1.57 turns of its period of 1, ends nowhere near the point asked for) fails with
  `MOVE_CURVE_BOUNDARY_SELECT_HELP_POINT` (batch 079 c006, c007).
- **A revolved spline sheet extended** (batch 079 c008): one face still, `REVOLVED` to `evSurfaceDefinition`. Its
  profile is the spline with the 5 mm straight on past its top appended as a cubic span (batch 115 c003: knots to
  1.219). Round past the side its bottom corner turns 0.5054 rad, not the 0.5 the side alone turns, and its top
  edge falls from 24.4928 at the old side to 23.9068 at the new, the new side reaching up the extended profile
  only 4.348 mm.
  Extended past its top edge and round past its side, it covers 216.07 mm² more; carrying each point 5 mm round its
  circle and the curve 5 mm straight on (the engine's) covers 220.04, rising 0.02 mm less and reaching 0.034 mm less
  far round at its widest. Neither uniform turns nor the curve's own polynomial carried on fit.
- **Face blend conic and inverse hold lines** (batch 080 c001, c002): a conic hold line 4 mm up a side sets that
  side back to it alone, the other keeping the radius's 5 mm: the section the circle's conic between them, an
  elliptical quarter 4 by 5 (its length 7.0904 mm, the ellipse's to the digit), a B-spline surface. An inverse
  tangent hold line on the same edge fails, `FACE_BLEND_DIMENSION_TOO_LARGE`: on a side's boundary there's nothing
  beyond it for the blend to run tangent to (between two of a side's faces it is passed over, batch 084 c002).
- **A circle's region has no vertex to connect** (batch 080 c003): a loft connection naming the vertex at a sketch
  circle's start finds none on its region, `LOFT_CONNECTION_MATCHING`.
- **Rolled walls cut and added to** (batch 080 c004, c005): a hole extruded across a rolled half tube's top is cut
  square to its definition sheet, the hole's outline on the inner cylinder and its walls radial (106.10 mm³ taken
  away that way, Onshape 105.98; its wall two B-spline faces and two planar slivers at its ends). A tab added in the
  flat pattern past the end of the flat wraps onto the cylinder at the neutral radius as part of the same wall,
  one cylindrical face inside and out (120 mm³ for 10 × 6 × 2).
- **A revolved sheet extended one way at a time** (batch 080 c007, c008): past its top edge alone the curve goes
  straight on its tangent, revolved (its end at radius 9.80572, height 24.492787, as the engine's; Onshape's new
  top edge its own fit, bulging 0.006 mm at its middle); round past its side alone each point goes 5 mm round its
  circle (0.5 rad at radius 10, 0.4167 at 12), the new side running up the curve. Only both at once differ (batch
  079 c008).
- **How far a slanted bend's end tilts** (batch 081, with 044 c001, 078 c001–c003): the plane through the end
  curves' corners meets the bend's edge λ inside the vertex, the sum over the two walls' edges at it of
  (T/2) t tan(φ/2) / (1 + t² r / 2(r + T)), where t is the slope at which the edge leans out along the bend (the
  tangent of its angle from the plane square to it, negative leaning in), r the definition's radius, T how far the
  other side of the material lies from it outward (negative when the material is inside), and φ the bend's angle;
  where the sum comes to nothing or less the plane holds both edges. For the L's (90°, one edge square): 0.48 (r 1,
  T 2, t 1/2), 0.4704 (r 2), 0.2348 (T 1), 0.2471 (t 1/4), 0.8569 (t 1), all to 6e-4 mm; with the material inside
  (r 3, T -2) the near end tilts 0.3645 and the far one not at all. The square frustum's top corners (86.6°, both
  edges leaning out) come within 3.6e-5 of the volume, the arcs' (11.25°) to the digit; edges leaning opposite ways
  (044 c003's near end) cancel.
- **Conic, curvature and asymmetric fillets of a circular edge** (batch 082 c001–c003): a cylinder's top edge
  filleted 3 mm takes a straight edge's section all round, swept about the axis (the asymmetric one 3 mm across the
  top, 2 down the side); Onshape's surfaces are its fits, their boundary circles enclosing the top 0.03 mm² more than
  the exact circle, the volumes to 8e-6.
- **A face blend side across two bodies** (batch 082 c004, c005): two upright sheets, each its own body, as one side
  fail with `FACE_BLEND_RIGHT_WALL_MULTIPLE_BODIES`, help point or not.
- **Circles lofted with a connection at points along them** (batch 082 c006): each circle starts at its connection
  (a quarter along the first, half along the second) and the loft runs start to start: a quarter turn's twist, the
  ruled solid between them (4188.79 mm³, Onshape's own surface 2.7e-5 under).
- **Face blends with two solutions** (batch 083 c001, c002): a floor and a Π-shaped sheet's two walls standing on it
  blend at both walls, the floor kept between them and the Π's roof with the rest; a help point near one wall makes
  Onshape fail (`FACE_BLEND_FAILED`). A side's coplanar faces (an upright split along its length) have their sense
  checked as one face's (`FACE_BLEND_WRONG_LEFT_WALL_SENSE`, c003–c006).
- **Hold lines and cliff edges inside a side** (batch 084 c001–c004): an edge between two of a side's faces (an
  upright split 4 mm up) is passed over as a tangent, inverse tangent or conic hold line and as a cliff edge alike:
  the plain blend is made, consuming the split, the side's faces above it one face. Only an edge on the side's
  boundary holds the blend (batches 079, 080).
- **A revolve about its own sketch edge** (batch 084 c005): a trapezoid revolved about its own edge on the axis,
  the axis queried as the sketch's edges through a point on it (the line's wire edge and its region's edge along
  it), fails in Onshape (`REVOLVE_SELECT_AXIS`). Batch 065 c005's quarter disc, queried the same way, failed with
  an "Execution error" instead: that case passes `RevolveBoundingType.FULL`, which isn't a value of the enum, and
  Onshape fails on the expression before the revolve runs (a missing enum value isn't undefined, as a missing map
  key is); the engine now does too.
- **Fillets of a concave circular edge, and a conic blend round a cylinder** (batch 084 c006, c007): the edge round
  a boss's foot takes the same curvature wedge, added; a floor and an upright cylinder blend with the conic section
  revolved round its axis, a B-spline surface in Onshape (to 5e-5 of its area).
- **Onshape's torus parameters** (batch 085 c002–c004, and 073 c004–c005): a torus a revolve makes has its
  coordinate system's z back along the revolve's axis and its x the world's y × z, as an extruded cylinder's (about
  z, x along -x; about x, along +z). Its u runs from x towards z × x, its v from the tube's inner equator:
  P(u, v) = C + (R − r cos v)(cos u x + sin u y) − r sin v z. OpenCascade's torus on the same axis runs u the other
  way and v from the outer equator, and its frame may be left-handed (a revolve about x). Isoparametric curves run
  in these parameters: equally spaced round a whole period at k / n from zero (across a part of one at k / (n − 1),
  the ends left out as the face's edges), tube circles from the inner equator round the tube, the others from x round
  the axis. The curve lying on a full torus's seam starts where the revolved profile circle starts (a sketch circle's
  -90°, a quarter turn back from its x, where the engine's circles start) instead.
- **A cone's side replaced by a cylinder, and two variable fillets meeting at a corner** (batch 085 c001, c005): the
  side goes over to the coaxial cylinder between the caps, which grow or shrink to it; variable conic fillets of a
  block's top front and top right edges, 4 mm where they meet, mitre there as constant ones do (to 5.4e-6 of the
  volume).
- **Planes of edges** (batch 086 c023): `evPlanarEdge` of an arc gives the circle's own coordinate system's plane
  (origin the centre); of a straight edge, a plane after all: origin the edge's start, x along it, the normal the
  world's z × it. `evPlanarEdges` of a face's loop (with a hole's circle) puts the origin in the middle of the
  edges' box, x canonical.
- **Rays and curvatures** (batch 086 c023): `evRaycast` meets a body's faces, nearest first, behind the origin
  too when asked (negative distances). A hit's parameter on a planar face runs as the plane's canonical axes
  across the face's box (v up the front face of a block, x canonical, y = normal × x); on a hole's cylinder as
  the cylinder's. `evFaceCurvatures` of a fillet's cylinder: the least curvature 0 along the cylinder's z axis
  (Onshape's frame for fillets, batch 047), the greatest 1/r along normal × that.
- **`evFilletRadius`** of a constant fillet's face is its radius (batch 086 c023).
- **A partial flange holding its adjacent edges** (the default, batch 086 c016): std splits the edge with
  `opSplitEdges` and finds its piece by `qSplitBy` and a point; the bend's joint attribute carries
  `holdAdjacentEdges`, and the plate beside the flange keeps its edge where it was (only the reliefs cut into it),
  where without it the pieces are cut back to the bend's setback.
- **Collinear sketch lines extruded** (batch 086 c017, 087 c001, c002): the sketch's region keeps an edge per line,
  but `opExtrude` sweeps one side face along collinear lines (its caps have one edge there), so a plate thickened
  into sheet metal (std extrudes the region and extracts the cap) has no vertex between them, and a rip between
  such points finds no corner (`SHEET_METAL_RIP_NO_CORNER`: `qClosestTo` ties two corners).
- **Edge directions** (batch 088, with 086 c004, 087 c003, c007): an extrude's side edges (swept from the profile's
  vertices) run from its end back to its start, up, down or symmetric; a revolve's edges swept round from the
  profile's vertices run back against the revolve; cap edges, circles and a cut hole's edges as the sketch's
  curves; a fillet's and a chamfer's edges as they run in their older faces' loops (anticlockwise about those
  faces' outward normals); an offset sheet's edges as the edges they're offset from. The engine records the way
  (`Info::sense`) and measures edges (`evEdgeTangentLines`, curvatures, `evLine`, `evPlanarEdge`) that way.
- **A tab across a corner** (batch 087 c004, with 105): on a box converted with rips up its corners, a tab on the
  front wall past the corner, subtracted from the right wall with 0.2 mm clearance, closes the corner's rip: std's
  `subtractTab` finds the rip by the tab's collisions with the part's faces at it, derips it (`@opDerip`), then
  subtracts and unites. The part's faces at a rip are the walls' ends across the thickness, and they carry the rip
  edge's association though they stand back from its line (here 0.02 mm); the engine now associates them, so the
  collision finds the rip and the tab matches Onshape's exactly.
- **A derip** (batch 105): `@opDerip`, in a sheet metal feature only ("Deripping can only be performed in a sheet
  metal scope"), is std's legacy derip (`createDeripOptions` with `@opEdgeChange`): each wall's side of the rip
  edge is moved within its plane onto the wall's end in the part (its signed distance from the edge), so the rip
  becomes a free edge on each wall, both still rips, and the bends beside it split, their short pieces one-sided
  but still bends. Rebuilding the parts then fails (`SHEET_METAL_REBUILD_ERROR`): a derip prepares for the feature
  that closes the gap. A conversion with a clearance tracks its bends through the offset surface it extracts (the
  engine lost them, so every edge became a rip): an offset copy's faces come from the faces they lie the offset
  from, its edges and vertices from those bounded by the same faces.
- **The last builtins** (batch 105): `validateToleranceSchema` finds no rules in a Part Studio's context or a new
  one (`TOLERANCE_SCHEMA_NO_RULES`, a `TemplateString`); `evMeshPoints` of bodies or faces that aren't meshes
  fails `NO_MESH_FOUND`, of nothing `CANNOT_RESOLVE_ENTITIES` naming `meshes`.
- **Help points and recognition refused** (batches 083 c001, 079 c005, 068 c003): a face blend given a help point
  where it has two solutions (a floor and a Π-shaped sheet's two walls; a floor through a cylinder lying along it,
  meeting it along two lines) fails,
  `FACE_BLEND_FAILED`, rather than pick one; an open box shelled from a block, three sharp walls meeting at each
  corner, isn't recognized as sheet metal (`evOffsetDetection` finds no group: `SHEET_METAL_CANNOT_RECOGNIZE_PARTS`).
- **Axes found by a point on a sketch's edges, and spines** (batch 089, with 084 c005, 065 c005, 074 c006, 075
  c007): `qContainsPoint` on a sketch's edges finds its line and its region's edge both, and a revolve about them
  fails, `REVOLVE_SELECT_AXIS` (evAxis takes one entity); the sketch entity alone works. A loft's sections added
  along a spine that isn't straight fail, `LOFT_FAILED` (with two sections `LOFT_INVALID`), whichever way the arc
  runs or however far past the profiles; along a straight spine the loft is the plain one.
- **Mate connectors by axis points, a cone's tip** (batch 090): top, middle and bottom axis points run along the
  face's coordinate system's z: an extruded cylinder's back along the extrude, a revolved cone's from its apex the
  way it widens (a frustum revolved about its own edge running down has z down, as one about an axis running up);
  so a cone's "bottom" is its narrow end's side. `POINT` on a cone's face is its tip, z along that. `LOOP_CENTER`
  and `VIRTUAL_SHARP` fail in FeatureScript (`MATECONNECTOR_QUERY_CSYS_FAILED`). A sphere's `CENTER` is the world
  frame at its centre.
- **An aligned section of a composite part** (batch 090 c010): the composite keeps its two bodies, cut, its shape
  theirs as they are after. The fixture document is in inches, so its default planes are 6 inches across (std's
  `PLANE_SIZE_BOUNDS`); the engine's harness now makes them so. The cut pieces of two default planes' rotated
  copies stay construction planes (see batch 092).
- **Construction planes cut** (batch 092, with 090 c010, 037 c004): a construction plane cut away by a subtraction
  (an extrude's cut, an aligned section's) stays construction, and so does a pattern copy of one; one kept only
  where a tool is (a crop view's subtraction of the tool's complement) is construction no longer, so batch 037's
  later aligned section copies the Top plane as a sheet.
- **Splits by self shadow** (batch 093, `opSplitBySelfShadow`): the view direction is the way the viewer looks;
  faces whose normal points against it are visible, faces edge-on to it invisible. Curved faces split along their
  silhouettes (a cylinder's two lines, a cone's two lines through its apex, at r · v⊥ = tan α (axis · v) for a
  cone widening along its axis, a sphere's great circle); front faces split where outlines (edges between a face in
  front and one not) cast shadows, a tower's on its step, a boss's and a dome's on their plates. The given bodies
  shadow each other, and a body not given shadows nothing. `qSplitBy(id, FACE, false)` finds the split faces' visible
  pieces, `true` the invisible ones. The new edges are split where they're deepest or shallowest along the view (a
  shadow of a circle or of a great circle at its far point), old edges only where new ones meet them. The shadows
  cast on the plates are Onshape's approximations: 0.0078 mm² over the boss's 150 mm², 0.028 over the dome's 65.06.
- **Uploaded files** (batches 094, 095; fixtures/blobs): a file uploaded to a document is a tab a Feature Studio can
  import (`Image::import(path : "<element id>", version : "<microversion>")`), its `BLOB_DATA` a map of the blob's
  type (`IMAGE`, `BREP` for a STEP file), data id, media type and an image's size in pixels, the document, element,
  version and microversion ids opaque (printed `BuiltIn: 2` and on, in the order the imports load). `skImage`
  bounds the image with four construction lines as wide as its corners are apart and as tall as the image's shape
  makes that, from the lower left corner, whatever the box's height. FeatureScript can't import a STEP file:
  `importForeign`'s precondition refuses a plain map as `CADImportData` and a plain string as `ForeignId` (`is`
  checks a custom type's tag, not its predicate), which fails as an "Execution error" (status `REGEN_ERROR`), as
  every failed precondition does in Onshape; `opImportForeign` wants a foreign id (`INVALID_INPUT`, faulty
  parameter `foreignId`), and a blob's data id isn't one it can import (`PARASOLID_IMPORT_FAILED`). Onshape's own
  upload makes a Part Studio whose Import feature depends on the file's tab by a reference (`<element id>::m<
  microversion>`, seen in the page's websocket frames); its parts are the file's, named as in it.
- **Part Studio tabs imported** (batch 096, with 060 c001): a Feature Studio imports a Part Studio tab
  (`Fp::import(path : "<element id>", version : "<microversion>")`) as a module whose `build` is a function value
  taking the configuration (`Fp::build({})`), usable as a `PartStudioData`'s build function. `derivedMirror` of one
  works, mirroring its parts across the xy plane (with a build function from the Feature Studio Onshape fails it,
  "Execution error"). The composite part Onshape's import makes is closed: deriving its solids finds none
  (`IMPORT_DERIVED_NO_PARTS`). Each source derives once per Part Studio; reading part names from the context of a
  running feature is refused (batch 011), so the batch counts them.
- **A rolled wall at a rip** (batch 018 c013's leftover parts): a rolled wall ripped from the flat walls it runs
  on into stops half the clearance short of each rip (0.01 mm), by a plane across its whole thickness square to the
  wall at the rip's edge, not along a radius (its end face 2.0000067 mm across a 2 mm wall, as that plane cuts the
  annulus); the engine now cuts the thickened rolled wall so.
- **A closed spline raised in degree** (batch 027 c005): std hands back clamped knots flagged periodic, and Onshape
  keeps the flag (`evCurveDefinition` says periodic, its knots and poles clamped); OCCT's curve can't be both, so
  the engine keeps the flag on the edge.
- **std's API builtins** (batches 100, 101): `opFlipOrientation` reverses a sheet's normals; `parseJson` leaves
  null out of maps; `evPointsDeviation` gives a point inside a solid 0, its nearest point the point itself;
  `evApproximateMassProperties` is exact for a block; `evTolerances` and `evMaxTolerance` fail from a Feature Studio
  ("Execution error"), `evFaults` gives a block none. `evFaceCurvatureDerivative` is the derivative of std's second
  fundamental form (minus each curvature times its direction's outer product) per length along a direction that
  must lie in the tangent plane (`NO_TANGENT_PLANE` otherwise, even leaning half out). An extruded cylinder's
  parameters run as a cone's and a torus's: u from the world's y × z round its z (back along the extrude), so the
  tangent plane's x points along that u (OCCT's runs the other way). `evApproximateBSplineSurface` of a whole
  cylinder is exact: two rational cubic half circles from that x (poles twice the radius out, weights 1, ⅓, ⅓, 1),
  flagged periodic with clamped knots, linear along z from the face's start, bounded by six parameter lines.
- **std's internal API functions** (batch 102): `getFeatureName` and `getParameterToleranceInfo` fail from a
  feature ("Execution error": no feature name during regeneration, and no tolerant parameter in a Feature
  Studio's feature), as does a `qCompressed` query of an empty string. `opBooleanedPattern` asks for a target body, then tool bodies,
  then an operation type (`CANNOT_RESOLVE_ENTITIES` with the field as faulty parameter for the first two,
  `INVALID_INPUT` for the third), and joins copies of the tool bodies by each transform to the target, leaving the
  tools where they were.
- **A Part Studio drawn in the UI** (batch 103, fixtures/batch-103/README.md): its tab's FeatureScript ("Show code")
  places each sketch entity by one `skSetInitialGuess` map, its calls giving no geometry, and finds planes and
  regions by compressed queries (`qCompressed`), whose encoding and evaluation the README gives. An open spline
  through points is the cubic at centripetal parameters with the guess's end derivatives (its handles are its second
  and second-last poles), a closed one periodic. A line its guess placed has Onshape's line at its start, where one
  given its ends has it at its middle. Every spline point and Bézier control point is a point body, a Bézier's
  control polygon construction lines, and an open spline's handles construction lines from its ends in a Part
  Studio's sketch but points in a Feature Studio's; closed curves' wire bodies come after the open ones'. A Part
  Studio's own code runs without its calls' preconditions (`skBezier` without points), and its functions can be
  values (`Ui::build`); a Feature Studio's can't (`St::abs`, "Execution error"). An entity given both its fields
  and a guess fails the solve ("Solve for sketch fails"), which adds nothing; `skBezier` given points makes its own
  `<id>.startSnap` and `<id>.endSnap` constraints, so a second of either is a duplicate.
- **Trimmed and used splines** (batch 104, fixtures/batch-104/README.md): a trimmed closed spline is written as a
  spline through its trim points and the points left between, with end derivatives, and is that cubic though it
  says `geometryIsPeriodic`. "Use" of a spline makes a spline by its poles (`skSplineSegment`, closed `skSpline`):
  its guess holds periodic, rational, degree, count, poles, the full knot vector and (open) the parameter range; a
  closed one's knots are unclamped with the degree's multiplicity at 0 and 1, its first pole repeated last, and
  Onshape flags it periodic.
- **Lofts and replacements Onshape refuses** (batch 108): tangent to a profile, a loft's side leaves each corner
  along the chord to its partner projected into the plane, and Onshape builds it only where that lies strictly
  within the corner's angle or the angle opposite it: triangles turned to each other fail tangent at either end at
  every magnitude (at the start one chord runs along an edge; at the end the chords leave both angles), but build
  with no condition or normal to the profile, and turned squares and unturned triangles build tangent. Onshape
  won't replace a face up to a sheet lofted between arcs, equal arcs too, however drawn or however wide (batch 111),
  though it will up to one lofted between lines, or up to the very same cylinder extruded from the arc (111 c001).
  Onshape's STEP shows a loft between equal arcs as that cylinder (batch 006), so its loft is a rational B-spline
  inside, simplified on export: what made the sheet counts, and extending it 5 mm at every edge first doesn't change
  that (108 c011). The engine marks the curved faces of lofts through arcs (`Info::lofted`; its own lofts between
  equal arcs are cylinders), keeps the mark when the sheet is extended, and refuses those faces as templates. A ruled
  sheet extended at several edges goes curves first, along the rulings, then the straight sides.
- **A UI-drawn sheet metal plate** (batch 107): its region query is compressed (`&<length>$<base64>`: zlib of a
  `%` query); a dotted string puts its literal pieces in the string table before itself, and `-<index>` is that
  table's string (batch 103's reading, a string's prefix, fitted only its own query); `T` and `F` are booleans.
  `derivedMirror` of a sheet metal model mirrors its definition and rebuilds it as the mirror image: the material
  stays on the mirrored side (the engine's mirrored sheets had their normals the other way round).
- **The last query types** (batch 106): `qUniqueVertices` keeps the earliest made of the vertices at a point (two
  blocks side by side keep the first one's shared corners); `qToleranceFilter` finds every edge and vertex past
  0 and 1e-9 m and none past 1e-5 m (the kernel's precision, 1e-8 m), faces and bodies never; `qIntersectsLine` and
  `qWithinRadius` find what touches the line or the ball (a line along an edge finds the edges and faces at its
  vertices too; radius 0 what passes through the point); `qCoEdge` and `qSourceMesh` evaluate to nothing; `qAxis`
  finds lines on the axis, circles about it and cylinders, cones, tori and revolved faces about it, either way along.
- **A sphere's principal directions** (batches 099, 109, 110, with 087 c008): every direction is principal, and
  Onshape's least is the normal crossed with the world axis cyclically before the normal's largest component (x's
  largest: z; y's: x; z's: y), the components weighed 1, about 1.035 and about 0.952 (batch 110: y wins against x
  from 0.96–0.97 of x, z against x past 1.05–1.06 of x and against y past 1.08–1.09 of y, for every sign and the
  third component 0.2); its greatest is the normal × that. It depends on the normal alone: the same at the same
  normal whatever the sphere's centre, radius or turn (batch 109: 48 normals on a sphere as made and turned two
  ways), so not on the sphere's parameters. Eberly's orthogonal complement and LAPACK's, Numerical Recipes' and
  Jacobi's eigen-solvers of n nᵀ or I − n nᵀ fit at most 64 of 120 points; this fits all. A revolved sphere's
  parameters run as a torus's: round its z, which is against the revolve's axis (099 c003).
- **Sketch text** (batch 091): Onshape draws the font's outlines as FreeType loads them unhinted at 100 pixels to
  the em, in 64ths of a pixel (font units × 6400 / units per em, rounded half away from zero; TrueType's implied
  points midway between those), the box's height 4608 of them (0.72 em), the baseline on the box's bottom, each
  glyph after the last by its rounded advance (no kerning: "Ab"'s b at A's advance). Straight pieces are lines,
  curved ones a degree-2 Bézier each. A construction frame is as tall as the box and as wide as the ink plus a
  quarter of a space (its 64ths, rounded down), with a line down its middle; the curves' wires come before the
  regions' sheets. The fonts are Open Sans 1.10 and Arimo 1.33 (Debian's packages), not the current releases (Open
  Sans 3's "I" is 200 to 370 units, Onshape's 201 to 371). The engine matches it exactly. Batch 097: spaces and
  punctuation advance as any glyph, "AV" isn't kerned either, a composite glyph ("é") is its parts' outlines, the
  text sits at the box's lower left corner however its corners are given, and `construction : true` leaves the
  glyphs as they are. Extruded, the Béziers nearest circular arcs become cylinders in Onshape (merged with tangent
  neighbours, by its kernel's own heuristic), and a Bézier along its chord a plane; several glyphs' regions come
  in an order of Onshape's own.
- **A flat cut crossing a bend** (batch 062 c006): the cut's piece on the bend's strip goes into the definition by
  the strip's stretch, so the definition's hole crosses the bend edge and splits it in two. The part: the hole's
  arcs in the walls beyond the setback lines (120° of the 6 mm circle each), the bend in two pieces, the hole's
  walls through the bend B-spline faces. The engine builds the bend once across its pieces, the hole's pieces in
  the walls' setback zones laid back onto the bend's flat strip (the stretch inverted) and cut from it before it's
  wrapped, each piece's two curves (one from each wall's side) joined first so the hole's walls are one face each
  as Onshape's (to 3e-7 of the volume; those walls are Onshape's own fits, 0.0004 mm² off). Its other pieces along the
  bend's line aren't stubs (no reliefs), and the side a wall lies on is probed 10 µm out (a wall rebuilt by the cut
  measures a micron off itself).
- **A failed case's leftover parts** (batches 014, 018, 060, 064, 086): a sheet metal part a failed case can't
  delete keeps Onshape's default name, "Part" and its place among the Part Studio's parts in order (named ones
  counted, flat patterns not): 086 c017's plate, after nine parts, is Part 10. Where the engine builds more or fewer
  parts before it (060's c001, 064's c003-c006), its number differs by as many.
- **A cone replaced by a cone** (batch 087 c005) fails in Onshape as in the engine
  (`DIRECT_EDIT_REPLACE_FACE_FAILED`).
- **A held partial flange aligned outside** (batch 087 c006): the flange's piece of the edge moves in by the
  thickness, joined to the held pieces by short steps; the plate beside it keeps its edge, rectangular reliefs
  cut in at the bend's ends as beside a stub.
- **Principal directions and parameters of revolved faces** (batch 087 c008-c010): where a face's curvatures
  differ, the least's direction has its largest component negative and the greatest's is the normal × it (a
  torus, a cone, a fillet's cylinder); a sphere's, all alike, are Onshape's own choice. On a whole torus or cone a
  ray hit's (and on a cone `evFaceTangentPlanes`') u runs round from the world's y × z (-x about z, as the torus's
  x), whatever the cone's coordinate system's x; a cone's v along its z (from a frustum's top). A vertical line's
  plane (`evPlanarEdge`) has its normal the line × x (a line along y, the line × -z).
- **Extrude start offsets** (batch 086 c018–c020): blind, back 5 mm; from a parallel face; from a circular edge
  (its plane through `evPlanarEdge`) all start the extrude there, as the engine's.
- **Shells, offsets, trims, helices and bridging curves** (batch 086): a filleted block's shell (the fillets'
  cylinders offset), a hollow sphere and an open frustum shelled outwards; a top and fillet offset together, a
  sphere inwards; a plane and a cylinder through it trimmed to each other (merged, and both other sides kept);
  helices round a cone, along a circle, by turns and pitch from 30°, from a start point; bridging curves with G3
  matches and by control points; tabs of a circle and on a flange: all as the engine's.
- **`derivedMirror` in a Part Studio feature** (batch 060 c001): Onshape fails with "Execution error". It builds
  its source twice, once by `@convert` and again in `instantiate` (no loaded context), which Onshape evidently
  refuses for a build function defined in the studio (it won't derive one source twice in a Part Studio, batch
  023). It is meant for assembly mirrors. A Part Studio tab's build function it allows (batch 096 c004). Building
  the source twice isn't the reason: importDerived of sheet metal builds it twice, and Onshape allows that (batch
  045), and so does building one directly and then instantiating it, or instantiating it twice (batch 098). What
  only derivedMirror does is read its source's part names (`derive`'s name suffix, `getProperty` on the built
  context), and Onshape won't read the properties of a context the Feature Studio's own code built while a feature
  runs, as it won't a context whose feature is running (batch 011); a tab's built context it reads (batch 096
  c001). The engine refuses that too.
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
  18.4°): no rule found. Normal to a direction (`NORMAL_DIRECTION`) makes the loft tangent to the plane square to the direction along
  the profile, so a profile not in such a plane is refused (`LOFT_INVALID`, c006: a circle in xy, the direction at
  45°); the engine refuses it too, and builds it where the profile lies square to the direction (060, 061).
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
  over d/r at both corners and 0.2% under at the equator: Onshape's new edge depends on the whole band. Batch 055's
  nine points on it wobble about the plane y = −R sin(d/R), where the sphere is d from the old edge across its
  surface (±0.007 mm at 1 mm, ±0.013 at 2), as a coarse fit would; at 5 mm it leaves that near the top. Its areas
  (batch 112: 157.1046 mm² at 2 mm, 189.8443 at 5) lie between that rule's (157.0744, 190.5228) and d/r round each
  circle's (156.9789, 188.3948). `evCurveDefinition` gives the edge as neither a B-spline nor a conic.
- **Geodesic offsets across a sphere and a cone** (batch 120): a sphere band's meridian offset 2 mm round the sphere
  is the small circle at y = R sin(d/R) = 1.98669, from the equator to latitude 60° (10.26322 mm; exact 10.26327).
  A half cone's ruling offset 3 mm round it is, developed into the plane, the line 3 mm from the ruling, stopping at
  the bottom edge: exactly 20.5063 mm, ending at (4.1822, 2.8351, 19.7893) at the top; Onshape's curve is its own fit
  of it, 0.006 mm off (20.5122, ending at z = 19.7951).
- **Sweeps locked to a sphere, and an elliptical sheet extended** (batch 119): a rectangle swept round a sphere along
  a great circle, straight or tilted, locked to it, is its revolve about the circle's axis (414.690 mm³), whatever
  the sphere's own axis. A sheet extruded from a quarter of a 20 × 10 ellipse, extended 5 mm past the ellipse's end,
  goes on round the ellipse 5 mm of arc length, keeping its curvature or not alike (to (−4.986609, 9.684185)); its
  new straight edge isn't a line to `qGeometry`.
- **Curved faces replaced** (batch 118): a sphere's face up to a concentric sphere moves to it, larger or smaller;
  a cylinder's side up to a coaxial cylinder of its own radius offset 2 mm becomes radius 12. The template is the
  query's first face whatever follows: a solid cylinder's faces as a template are its side. A cylinder's side up to a
  sphere about its middle becomes the ball between the caps' planes, a barrel (the caps grown to radius 11.18).
- **A guide crossing a loft's side** (batches 075 c005, 112 c001–c003): between a square and a smaller one, a guide
  from the middle of a bottom edge to the middle of the top one bends that side alone (the others stay planar).
  Across it the side is a cubic with one knot where the guide crosses (0.5), five poles: the side's straight edges,
  the plain side's poles at the Greville abscissae 1/6 and 5/6, and a middle one moved so the surface passes through
  the guide, S = L + N₂(u)(G(v) − L(t, v))/N₂(t), the guide taken at its own parameter (its arc's angle). Along it,
  each row is Onshape's C1 fit of eight spans (fewer for flatter guides). This matches Onshape's surfaces to its fit
  of the arc (1.0e-4, 4.7e-4, 1.8e-4 mm for bulges out 3 and 1 mm and in 1 mm).
- **A loft along a straight path** (batches 075 c006, 112 c004–c006): circles of radius 10 and 5 lofted along the
  line between their centres make Onshape's side a B-spline that is the cone along its height (the radius linear in
  z at every angle) and whose end circles are exact, but whose sections between pass through the circle only every
  45° (u = k/8) and dip up to 0.006 mm between: 5493.27, 5492.88 and 5492.56 mm³ through 3, 5 and 9 sections to the
  cone's 5497.79. What sets the dips, these don't show.
- **Everyday variants** (batch 122): an enclosure is one part for all the regions it closes that touch (a tube cut
  by two planes and a plane through its axis is one solid, the plane between the halves gone). A cylinder split
  through its axis has one cylindrical face per half; OCCT's seam would leave the half holding it in two faces, so
  the split merges a closed face's seam where it ends up between two faces of one surface. A block with its top
  edges chamfered, shelled with its top removed, is the plain 1 mm offset (its cavity 4286.66 mm³ by hand, as
  Onshape's); OCCT's thick solid fails where two chamfers meet a block's corner (four planes at a vertex), so an
  all-planar solid shelled inwards falls back to cutting it by its cavity built plane by plane (each vertex where
  its faces' planes, so offset, meet). Chamfers of a boss's foot, a hole's mouth and round a block's filleted top,
  drafts of a boss's side and a hole's wall, faces deleted and healed (a boss, a pocket, a cylinder's chamfer, a
  block's top fillets), a curved sheet and a sphere thickened, and a sphere split keeping one side match as built.
- **Which face takes an uneven chamfer's first distance** (batch 123, with 003 and 122): not the face left or right
  of the edge as it runs (a block's top front and top back edges run the same way and both put it on the top), nor
  any order of the faces' normals, but the older face: a block's top before a hole's wall or a boss's side; within
  an extrude, the side swept from the sketch's first curve, then the caps, then the other sides in the order the
  sketch made their curves (a rectangle's left, right, top, bottom: so a block's x = 0 side before the top, the top
  before the other sides, x = 30 before y = 0 and y = 20; a cylinder's side before its caps). An offset and angle
  chamfer measures its distance on the same face. The engine ranks the faces by the oldest entity each came from,
  taking a region's edges as the sketch curves on them and a cap just after its extrude's oldest side.
- **Everyday modelling** (batch 124): where a boolean or an extrude up to a curved face leaves an intersection curve,
  OCCT cuts it into pieces (B-splines meeting at vertices only they share) and Parasolid keeps one edge: a boss up to
  a lying cylinder's side has two edges, a cross hole through a lying cylinder four. The engine concatenates such
  B-splines after booleans and on an extrude's piece up to a face: chains of B-spline edges between the same two
  faces meeting at vertices only they share are concatenated (each piece turned to run along the chain), the curves
  on both faces projected again, kept only where the joined edge is as tight as its pieces. (`BRepLib_FuseEdges`'s
  own concatenation joined a cross hole's lower mouth with a vertex 3 mm out of tolerance, and changes the
  tolerances of the shape it is given in place; it now joins conics only.)
  A fillet of the circle where a block's top meets a spherical pocket is a torus; OCCT's fillet breaks where the
  sphere's seam meets the circle (six B-spline scraps), so the engine revolves the section (the arc tangent to the
  plane's line and the sphere's circle) round the circle's axis and cuts it away, exactly Onshape's (2244.82030 mm²).
  A profile swept along a helix keeps its place in the path's Frenet frame (a square's box stays ±11.0011 about
  the axis); OCCT's default corrected Frenet frame turns it against the torsion, so along a path that leaves its
  plane with no straight stretch the engine sweeps with the Frenet frame. Onshape's helix is its own fit, 7 µm off
  the engine's. A tee of pipes (radius 5 and 4, the fillet reaching the larger pipe's equator) has its joint filleted
  as batch 128's tees are (OCCT's fillet three faces; filled as part of the solid's closing), and its chamfer as
  batch 129 found Onshape makes them (within 0.005 mm³).
- **Sphere rims, helices square to the path** (batch 125): a dome (a ball united on a block) filleted at its
  concave foot fills a torus, the arc beyond both faces; the engine revolves it as for a pocket's rim. A pocket's rim
  chamfered 1 mm equally is a cone through the point 1 mm out along the plane and the point 1 mm away on the sphere
  in a straight line (16.28845 mm³ removed, as Onshape's; by arc length it would be 16.27195): the engine revolves
  that line. Sweeps along helices square to the path, both hands, match to Onshape's helix fit. A cross hole's
  mouth filleted 0.5 mm: OCCT's fillet comes out with edges 0.7 mm out of tolerance, so where a fillet does that, a
  closed edge is rounded as part of the solid's morphological opening (eroded by the radius in intersection mode,
  its intersection curves joined, grown back with round joins: every ball of that radius inside it), the sliver
  the opening takes off along the edge cut away: one face, as Onshape's (to 7e-5 mm² and 2e-5 mm³).
- **Shells OCCT's thick solid can't make** (batches 124 c009, 125 c007): a tee of pipes open at its ends and a
  hemisphere open at its flat face come out of `BRepOffsetAPI_MakeThickSolid` invalid, and the failed attempt leaves
  the solid it was given damaged in place (its edges' tolerances and curves changed: a second attempt hands back
  the solid unshelled, and booleans with it go wrong). So the thick solid works on a copy; failing that, an
  all-planar solid is cut by its cavity built plane by plane, and otherwise by the solid offset face by face
  (`BRepOffset_MakeOffset`, the removed faces outwards, the others inwards, every offset carried on past where the
  faces were): the inner sphere then runs on to the rim, as Onshape's (567.58107 mm³ exactly), the tee's inner
  pipes out through the ends (1446.0982 mm³, seven faces).
- **`qSplitBy` for faces** (batch 126 c008): std drafts a symmetric extrude by splitting it at the sketch plane
  (`opSplitPart` with the plane) and drafting `qSplitBy(split, FACE, false)` and `(…, true)` each its own way: the
  pieces of the faces the split cut in two, each on its side (the sides' halves, not the caps or the cut face). The
  engine records those pieces with their side. Faces moved on curved faces (a hole's wall, a boss's side), heals of
  a cross hole through a lying cylinder and of a spherical pocket, feature patterns of a boss round a cylinder's top
  and of a hole along a block, a merged mirror, a 270° revolve and a subtraction keeping its tool match as built.
- **Features in sequence** (batch 127): shells of a filleted block with a hole, fillets inside a shelled box, a rib
  across one, a shelled box mirrored and split, and a drafted face with a hole in it match as built. Three engine
  fixes: `qContainsPoint` finds a closed edge by any of the pieces OCCT split it into at a seam vertex (the circle
  where two balls meet, its point on the hidden piece); that circle, between two spheres centred on its axis, is
  filleted as a plane's rim round a sphere is, the torus tangent to both circles in the half plane (OCCT's fillet
  fails there); and a top thickened with the fillet on its edge has one wall at each end, the walls on one plane from
  neighbouring edges merged (the caps kept as they are).
- **Joints of curved faces** (batch 128): a concave joint between cylinders (a tee of a pipe of radius 10 and a
  branch of radius 3; a boss rising from a lying cylinder) filleted 1 mm: OCCT's fillet comes out in three faces with
  edges a millimetre out, so the engine fills it as part of the solid's morphological closing (grown by the radius
  with sharp joins, so the joint stays an edge, mended where OCCT leaves a face's wires out of order, then shrunk
  back with round joins, which round it), the material added along the joint united with the solid: one face, as
  Onshape's (to 1e-4 mm²). A shell is accepted only whole (a point the thickness in from each face kept lies on its
  boundary): OCCT's offsets of the tee lose the branch's bore and come out valid. So the engine builds that cavity
  from the surfaces: each kept face's offset a thickness in and each removed face's own, bounded to cover the solid,
  split against each other into cells (`BOPAlgo_MakerVolume`), the cells inside the solid farther than the
  thickness from every kept face cut away, then their walls and the joint's pieces merged: 7 faces and 8 edges,
  3131.54 mm³, as Onshape's. A pipe joint crossing both pipes' seams is shown as one edge, as Onshape's (its pieces
  between the same two faces, running on smoothly). A capsule shelled open at its base and a shelled box's window
  filleted match as built. Crossing pipes' joints: OCCT's fillet makes three faces a joint and the closing adds
  nothing, so the engine builds the fillet from sections, the ball's arc at each of 96 points round the joint
  (lofted, closed off past the faces, filled), one face a joint as Onshape's (to 1e-4 mm²). (The tee's chamfer: batch
  129.)
- **Where equal chamfers of curved edges meet their faces** (batch 129): along circles between surfaces of revolution
  (a spherical pocket's rim, a cone standing on a plate, its foot and its top) the feet are the width from the edge in
  a straight line square to it, as batch 125 found. Along intersection curves (a tee's joint at 1 and 2 mm, a boss
  rising from a lying cylinder, a cross hole's mouth) they are where a ball of that radius touching both faces touches
  them, the fillet's edges: at the tee's side (y = 3) the branch's foot is at z = √105 = 10.247 and the pipe's at
  (3.6364, 9.3154), the ball's, exactly; the chamfer is the ruled surface joining the points each ball touches (std's
  default `ChamferMethod.FACE_OFFSET`, "Offset"). At a square corner the two agree. OCCT's chamfer puts the feet the
  width away as along circles (the tee's 1 mm chamfer added 10.18 mm³ to Onshape's 7.27). The engine builds such
  chamfers from the fillet of their width, its face swapped for the ruled surface between its edges (each point on
  one edge paired with the point its ball touches on the other face): within 0.03 mm³ of Onshape's, whose surface is
  its own fit. Batches 124 c008 and 128 c002, the tees' chamfers, match so too.
  Batch 130: along straight edges at 60°, 120° and a 120° concave corner, the feet are the width from the edge too
  (the ball's would be cot(α/2) of it), as OCCT's; uneven chamfers of a cone's foot match. Uneven chamfers along an
  intersection curve follow the ball's rule with the point off each face by its own amount: with offsets d₁ (on the
  first face, the older) and d₂ the point is d₂ off the first face and d₁ off the second (a square corner's feet d₁
  and d₂ from the edge), with d at θ it is d·tan θ off the first and d off the second (12.68 and 3.892 mm³ added,
  as Onshape's; the other way round 12.60 and 3.83). The engine finds that point at each of 96 places round the
  edge, lofts the chamfer's sections (closed off past the faces, the loft's sides sewn into a ring) and cuts or
  fills: within 0.02 mm³ of Onshape's.
- **Sheet metal past one of each** (batch 131): bends aligned to either hold line, the inner, outer and middle lines,
  slanted 45°, with their own radius; jogs anchored inside and at the nominal line, twice the thickness, keeping
  their material; square, rectangular, round, closed and simple corners; rips with edge and butt joints: all match.
  A jog with a wall between its bends: std's second bend line is a spline fitted through two points, and the wall
  comes out of the definition sheet with its edges straight B-splines; swept through the thickness they made
  surfaces of extrusion where Onshape's walls are planes, so the engine remakes straight B-spline and Bézier edges
  of a planar face as lines before sweeping it. The round relief and the closed corner match within 0.01 (as in
  batches 013 and 016).
- **Non-circular fillets of curved edges** (batch 132): along a tee's joint, a boss's foot on a pipe, a spherical
  pocket's rim, a cone's foot and the circle where two balls meet, Onshape's conic, curvature and uneven fillets have
  their feet where the rolling ball touches the faces (its centre `d₁` and `d₂` off them; within 1e-6 mm), and
  between them the section of a straight edge's fillet in the plane through the ball's centre and feet, round the
  corner where the faces' tangent lines across meet. Its faces are its own fits, 4e-5 to 7e-5 of their areas under
  the exact surfaces (as batch 082's conic round a cylinder's top). An uneven fillet's `radius` round a tee's joint is
  measured from the pipe, round a cone frustum's top from the top (batch 135 gives the rule). The engine builds these fillets from sections at 96 places round the edge, lofted and cut or
  filled; an uneven one's rational sections, their weights changing round the edge, are fitted with cubics through
  17 points with their end derivatives as they are (`GeomAPI_Interpolate::Load` rescales them unless told not to,
  and the cubic strayed 6 µm from the section: 1.5e-3 of the area).
- **Uneven fillets of circles; sketch constraints** (batch 135): an uneven fillet's `radius` is measured from the face
  in whose loop the edge runs against Onshape's direction for it, convex or concave: an extruded cylinder's top edge
  from the cylinder and its bottom edge from the bottom (the circles run anticlockwise about the sketch's normal); a
  revolved part's from the other face (its circles run back against the revolve, batch 088): fCone's top from the
  top, its bottom from the cone, a revolved cylinder's top from the top; a hole's rim on its sketch's plane and a
  boss's foot (the tool's own circles) from the plate; a drafted extrude's top from the cone. A new edge where a
  boolean's tool crosses its target runs along n_target × n_tool (the result's outward normals): a tee's joint from
  the pipe (batch 132), a hole's far rim anticlockwise as batch 088 found. The engine gives such edges that sense and
  keeps building the sections from the leading face (from the other a pocket's rim came out unfilleted), the
  offsets swapped where Onshape measures from the other face. Onshape's own volumes of these fillets are up to
  0.13 mm³ over the exact (an extruded cylinder's top, 1 and 2 mm: 6256.8203 mm³ by Pappus, Onshape's 6256.9501,
  the engine's 6256.8154).
  Sketches drawn off and pulled into place: Onshape solves fully determined ones exactly (lengths, distances, equal
  sides, radius, diameter, concentric, a slot of tangent arcs, a 30-40-50 triangle). `ANGLE` given `localFirst`,
  `localSecond` and `angle`, and `MIDPOINT` given `localFirst` (a point) and `localSecond` (a line), change nothing
  (a sketch drawn in Onshape writes a midpoint as `localMidpoint`, `localEntity1`, `localEntity2`, batch 104).
  `MIRROR` of two lines puts the second on the first's reflection, wherever along it, its length and the projection
  of its middle kept.
- **How Onshape's sketch solver moves an underdetermined sketch** (batch 136, and 135 c016, c018, c019): it holds
  each entity's direction, length or radius before any entity's place, and older entities before newer ones,
  moving as little as it can in that order: a point put on a free line moves, not the line; of two free lines
  joined, the second moves, whole; a free line lengthened keeps its start and moves its end (a free point coincident
  with the end goes with it; one coincident with the start stays); a free line made horizontal or parallel turns
  about its middle; a free circle given a radius keeps its centre. The engine solves so: a line's unknowns are where
  its middle started, its direction and its length, its start that middle less half its starting length along the
  direction; the unknowns are grouped (directions, lengths and radii oldest entity first, then places oldest first)
  and each step projects the start onto the constraints, linearised where the solution has got to, one group at a
  time: each group moving as little as the groups after it let it (std's tear drop hem, batch 029, solves so too).
  Gauss–Newton steps finish it off. Sketch curves constrained to each other come out in the order the sketch made
  them (136 c015: a circle, then a line normal to it); in a sketch drawn in Onshape, closed curves after open ones
  (batch 103).
- **Sweeps locked to curved faces** (batch 137): along a helix round a cylinder or a cone frustum, locked to it,
  Onshape fails (`SWEEP_FAILED`): its helix is its own fit, 7 µm off the face (batch 124). Along an edge of the face
  (the curve a split left round a cylinder, across its seam) or a circle on it (a small circle split from a sphere by
  a tilted plane) it runs, the profile turning with the face's normal along the path. The engine refuses a locked
  path edge that is neither one of the faces' edges nor a line, circle or ellipse; it carries the profile in the frame
  of the path's tangent and the face's normal, a copy at each of 192 points along it, and lofts through them (a ring
  for a closed path), as OCCT's sweep on a support drifts (round the cylinder 7.8e-4 of the area off, lopsided). The
  ring round the cylinder is exact: 21.040594 mm³, the path 21.040590 mm long, the square 1 mm² (Pappus); Onshape's
  is 21.055754 mm³, its path 21.0366 mm. `evDistance` gave a revolved cone's raw OCCT parameters, which
  `evFaceTangentPlane` reads in the cone's own frame (a point's parameter evaluated on the far side): it now gives
  them as `evRaycast` does. The normal projection's ruled sheet (batch 133) took its two curves at their default
  chord-length parameters, longer outside a cylinder than inside, so its rulings tilted and a circle's projection
  round the cylinder came out 6 µm inside the exact curve; both now go through the edge's own parameters (0.05 µm).
- **Frames locked to curved faces** (batch 138): round a closed edge (the curve a circle splits a cylinder's side
  along, a sphere's small circle) a frame fails in Onshape with `FRAME_CANDIDATE_FACES`: std trims each segment at
  its sweep's start and end caps, and a ring has none. Round the four edges a rectangle splits a cylinder's side along
  it mitres the corners: Onshape's split gives lines up the side and arcs round it (its STEP's curves), and the arcs
  run whole across the cylinder's seam. The engine now gives projected edges lying on lines and circles those curves
  (within 0.1 µm, the curves on the faces refitted), joins a split's B-spline pieces only where they run on smoothly
  (it had joined the rectangle's four sides into one edge), replaces a planar end of a body revolved about an axis
  in its plane (a segment round a cylinder) by turning the body on round the axis before cutting it, and evaluates an
  edge OCCT split at seam vertices (`evEdgeTangentLine`, `evLine`, a locked sweep's path) as the one edge Onshape has,
  its pieces joined. Std orients each segment's profile from its edge's ends and directions, and chains the edges
  into a path from where they meet: with the split's edges running and ordered as the tool edges they came from
  (batch 150), an arc split at the seam ending where its last piece does, an arc swept round its circle the way it
  runs from the end the profile is at (std puts it at the far end of an edge the closed path runs backwards along),
  and an edge carried on through a mitre's column one edge, every segment is Onshape's (area, volume and bounds to
  1e-6; Onshape's STEP of the arc segments reads back 4e-3 small).
- **Ruled surfaces' overrides along a curved edge; feature patterns of separate faces** (batch 139): along a curved
  edge Onshape blends the rulings at the edge's two ends as vectors held in the frame that turns with the edge (the
  ruling at angle 0, the one a quarter turn from it, the edge's tangent), with smoothstep weights (`useCubicInterpolation`):
  a tangent surface with its ruling normal at one end has its far edge pulled in between (77.2845 mm², not the
  93.49 of blending the angle and the width apart, which agree with it when only the width changes, batch 040); one
  end's ruling up to a point blends the vector to that point in the end's frame (120.492 mm²). The engine blends so.
  A feature pattern of a hole feature that cut a sketch's two circles patterns the holes' faces (not
  `fullFeaturePattern`): the engine made one tool of the two separate holes and patterned only one; each set of faces
  joined by edges is now its own tool. `dummyFeature` does nothing; `jogSectionPartInternal` sections as
  `jogSectionPart` does after clearing sheet metal data (a sheet metal L comes out a plain part).
- **Everyday parts, a third round** (batch 140): a hinge leaf, a sheet metal box of four flanges, a swept and
  shelled pipe elbow with flanges, a bottle, a swept handle, patterned counterbored holes and a drafted block with a
  boss match as built. A tooth added to a disc's rim and patterned without `fullFeaturePattern` fails in Onshape
  (`PATTERN_SWITCH_TO_PER_INSTANCE`, its top and bottom merged into the disc's faces), as in the engine.
- **Everyday parts, a fourth round** (batch 141): a hole grid patterned in two directions, a split block filleted,
  an L intersected with a cylinder and countersunk holes match as built. A sheet metal channel's hole patterned along
  it (a face pattern of the hole, which std re-cuts in the definition sheet as an edge pattern): the definition
  sheet, which the extrude had made by turning OCCT's shell over as a whole, came back from the boolean with its
  faces the shell's own way, and the part was rebuilt with its material inside the channel; the engine keeps the
  sheet facing as it did. A square lofted to a circle and shelled open at both ends (c003), its sides meeting at the
  square's corners and running on tangent at the circle: OCCT's thick solid fails (its offsets joined by
  intersection or by arcs, all intersected or not), and so does a cavity cut from the sides' offset surfaces split
  against each other (where they meet tangent, the split's edges take tolerances of a millimetre or more). The
  engine now builds the cavity itself: each pair of neighbouring sides' offsets (their surfaces carried on past
  their edges, as Onshape's walls are: their knots run from −0.009 to 1.009 across and up to 1.031 along) meet along
  a curve found point by point at 33 even heights by damped least squares, which holds steady where they run on
  tangent; each wall is fitted through rows at those heights from one curve to the other, the cavity runs a little
  past each end, where planes close it, and the solid less it is the shell: 24 edges, 10 faces and 16 vertices as
  Onshape's, 3278.5712 mm³ where the exact shell, its sections' areas integrated apart, is 3278.5696, its end rings
  to 1e-7 of the exact ones. Onshape's measures are its own fits': its volume 3279.0920 (0.52 mm³, 1.6e-4 over),
  its STEP's 3277.37, its inner sides there 510.93 to 511.00 mm² for four equal sides (0.27 mm² short at the
  bottom ring), so the case is compared loosely.
- **Everyday parts, a fifth round** (batch 142): a pulley, a grooved shaft with a cross hole, a sheet metal bracket
  with holes patterned along its flange, a slotted disc, a mirrored and united block, a snap box and a pocket
  filleted along tangent chains match as built; a loft through a square, a circle and a square to Onshape's fits
  (4.2e-5 of its area).
- **Everyday sheet metal** (batch 143): holes patterned along a box's walls, in a 3 × 2 grid on a plate and along
  a channel's legs, a slotted flange hemmed, a jog with holes on both sides and a plate's corners rounded with a hole
  patterned round it match as built. std patterns a sheet metal hole by its edges in the definition sheet (an edge
  pattern, re-cut at each instance); a hole cut through two walls has edges bounding two faces, each now re-cut in
  its own face's plane.
- **Everyday surfacing and direct edits** (batch 144): a fit spline revolved and shelled into a vase, a cable swept
  along a 3D fit spline, a block lengthened by moving its end face, a fillet deleted, a face thickened onto its block
  and a drafted cylinder filleted and holed match as built (Onshape measures the B-spline bodies to 4e-4: the cable's
  exact volume, πr²L, is the engine's). A sheet lofted from an open profile faces along the profile (as its edge
  runs) crossed with the way to the next profile, as sheet metal lofts do: between two arcs (counterclockwise about
  their sketches' normals) it faces their centres, so thickening goes in towards them. Replacing a planar face
  changes only the face's own column, growing it up to the plane or cutting it back: a boss on a block's top outlasts
  the top replaced by a plane below the boss's top (the engine had cut away everything past the plane).
- **Everyday parts, a sixth round** (batch 145): a hex nut chamfered by a revolved cone, a bolt, a spoked wheel, a
  bottle (a loft through three circles with a neck, shelled), a ribbed L bracket, a lipped box and a notched knob
  match as built. A hinge leaf's knuckle patterned as a feature fails in Onshape too (`PATTERN_SWITCH_TO_PER_INSTANCE`):
  the knuckle's end and the plate's merge into one face the knuckle made. A face pattern of a notch cut down a
  cylinder's side, whose open boundary isn't planar, is the region between the body and the body healed of the
  notch. OCCT won't offset a loft's sides through three profiles built as Hermite spans (taken for C0) until their
  knots come down to multiplicity two (they're C1). OCCT's fillet of the knob's top edges is right but 1.3e-5 out
  of tolerance; the opening fallback then rounded every convex edge its slivers ran along (the notches' corners and
  the bottom too, as the offsets happened to join them, run to run), and now gives way where a sliver runs along an
  edge not given.
- **Everyday parts, a seventh round** (batch 146): an enclosure with rounded corners shelled and bossed, a pipe
  elbow swept and shelled open at both ends, a flanged pipe with patterned bolt holes and a filleted joint, a lid
  split off a filleted box, a box drafted from its bottom, a pocket mirrored as a feature, a gear-like disc of tooth
  parts patterned and united, and a keyed, chamfered shaft all match as built (to 1e-7).
- **Everyday parts, an eighth round** (batch 147): a boss up to a tilted plane, a ring revolved 270° with a cap's
  edge filleted, a split shaft collar, a swept handle and holes drilled square to a slanted face match as built.
  Known gaps: a cam's lines, drawn a little off and held tangent to two undimensioned circles, solve differently:
  Onshape keeps both lines' directions and the first circle's radius exactly, lengthens the lines, grows the second
  circle (8 to 8.08) and moves both circles, leaving the whole symmetric about y = 0.05; the engine keeps both radii
  and the lines' lengths and turns the second line (no ordering of the engine's groups tried reproduces Onshape's).
  A drafted pocket's floor edges filleted: OCCT patches the two corners at one end (B-spline faces), where Onshape's
  fillets meet along where they cross, as OCCT's do at the other end. Where OCCT's fillet comes out with more faces
  than one per edge, straight edges between planes with no three meeting at a vertex are now rounded by a tool per
  edge (the corner between the faces' feet and the ball's arc, carried along the edge and cut back by the planes at
  its ends on the edge's side), cut from or added to the solid: Onshape's faces, edges and vertices, its area to
  3e-5 mm².
- **Everyday parts, a ninth round** (batch 148): a pillow block with its bore chamfered by distance and angle, a dome
  revolved and shelled, a bracket mirrored and united, a pin patterned as a part and united, an offset surface
  thickened, a rib normal to its sketch and a two-distance chamfer match as built. A column extruded up to next from
  inside another part (the first case's plate, built at the same place) runs on to its own roof in Onshape: up to
  next passes by parts the profile starts inside.
- **Everyday sheet metal, a second round** (batch 149): a tray with lips flanged off its walls' top edges, a Z
  bracket extruded from a sketch with holes in its web and legs, a rolled hem and a tear-drop hem match as built.
  A tab bent along a line across it: the bend's strip wrapped onto its cylinder came out inside out (the rebuilt
  loops ran as their first edges happened to), so the bend wasn't united with the walls; a wrapped face with a
  negative area is now rebuilt with its loops reversed, and the bend matches.
- **Probes of splits and sketches** (batch 150): a cylinder's side split by a rectangle projected normal to it has
  edges running as the tool edges they came from (`skRectangle` draws all four sides rising: the arcs run round
  and the lines up as those sides do), and the engine now runs them so. Sketches drawn a little off: a circle and a
  line held tangent (either drawn first), a line's start held on a circle and a tilted line held tangent to one all
  move the circle's centre and leave the line; of two circles held tangent the second moves; of two lines with ends
  held together the second slides (keeping its length and direction). Where lines sit is now held before where
  circles, arcs and points do in the engine's solver. Two dimensioned circles joined by lines held tangent to both
  (c008), where Onshape leaves one line exactly where it was, turns the other about its middle and moves both circles
  a little (0.05), match since batch 155.
- **A chamfer varying from run to run** (batch 129 c004, a cross hole's mouth chamfered): built from the fillet of
  its width, its face swapped for the surface ruled between the fillet's edges, the rulings taken from whichever edge
  OCCT listed first, which varied with memory layout (one run in ten or so, 0.54 mm² of area off Onshape's). It
  now starts from the longer edge, as the runs matching Onshape did.
- **Everyday parts, a tenth round** (batch 151): a revolve up to a plane through its axis, a boss patterned round a
  disc as faces, a pocket mirrored as faces, a boss up to another part with an offset, a block intersected with a
  cylinder, a cylinder's side thickened into a tube, a split with its top piece deleted and a draft carried round a
  filleted block by tangent propagation all match as built (to 1e-13).
- **Routing curves from CSV data** (batch 152): std reads a routing curve's CSV rows into its points in its editing
  logic, when its Process button is pressed (`routingCurveEditLogic`), each row's three columns from the start column
  in the unit chosen, a row repeated whole read once, a row too short refused; run so and then built, a polyline with
  bends, a spline through centimetre rows with a label column, and the rest match Onshape's.
- **Fills of non-planar loops** (batch 153): Onshape fills a loop of four edges with the bilinearly blended Coons
  patch of the four, each arc taken evenly by angle: arcs rising 5 over a rectangle's sides meet 10 up in the middle
  (734.18900 mm² by hand, Onshape's 734.18917), an arc up and one down with lines between give 653.9672 (653.9666);
  it faces as the bilinear patch between the corners (u back along the first edge from the second, v along the
  second). The engine fills so now, the patch from its fits of the edges (0.0033 mm² off for the arcs); it had used
  OCCT's filling wherever the corners lay in a plane. A three-sided loop is filled by a rule not found: no Coons
  patch with a side collapsed to a corner (405 mm² to Onshape's 389.91); the engine's is OCCT's (393.17).
- **Dimensioned circles and tangent lines settling** (batch 154): one line drawn a little off held tangent to two
  dimensioned circles stays where it was, before or after them, and each circle moves the shortest way onto it,
  along its normal (0.0514). The engine's least-norm step had divided by a rounding-sized eigenvalue of a rank-one
  Gram matrix (1e-17, above its absolute floor) and sent the circles sliding 0.24 along the line too; eigenvalues
  under 1e-12 of the largest are now left out, and the one-line cases match. With two lines held tangent Onshape keeps
  one line where it was and moves the circles the shortest way onto it, then the other line onto them: along its
  normal for one circle, turned about its middle for two (batch 155 shows which line: the one starting first).
  Known gap: without radius dimensions (c005, batch 147's cam) Onshape keeps the first circle's radius and grows the
  second's (8 to 8.08), sliding the other line straight up; the engine keeps both radii and turns the line.
- **Which of two lines holds** (batch 155): Onshape keeps the line whose start comes first by x and then by y (the
  lower of two level lines, the left of two turned a quarter turn), whatever its name, the order it was drawn or
  constrained in, or how far off it was drawn; batch 150 c007's line through the origin is that one too. In a sketch
  with nothing fixed the engine's solver now holds that line hardest (with something fixed, as std's tear-drop hem
  and a mirror across a fixed axis are, lines' places still hold oldest first, batches 029, 135 c019, 149 c005); the other lines keep their lengths and give way after the circles and points,
  turning about their middles only where sliding won't do, then sliding along their main axis and last across it
  (a level line slides straight up, batch 154 c006; two lines' ends held together, the second slides, 150 c007).
  Batch 150's dimensioned cam (c008) now matches. A line held only tangent to circles slides along itself with
  them: Onshape carries it as far as their centres move on average, then turns it about where it so carried crosses
  its new place, each of its points keeping its distance from there. Drawn 0.2 further off than the held line (c005),
  its middle moves along the normal halfway between its old and new directions (0.1943 across, 0.0006 along its new
  direction); with the held line drawn further off (c006), the circles go 0.19 to it and the other line goes as far
  with them (0.0882 along itself). The engine now does the same after solving, and every case matches.
- **Circles without radius dimensions settling** (batch 156): one circle between two lines and two circles under one
  line settle as the engine's do (the circles' centres move, their radii hold). With two lines and two circles, an
  undimensioned circle's radius gives way before the other line turns: only the first circle dimensioned, the second
  grows (8 to 8.0799) and the other line slides straight up 0.1; only the second dimensioned, the first shrinks (15 to
  14.9201) and the line slides down; neither, the second grows, whichever circle was drawn first. The changed
  circle's centre lies on the lines' bisector, and its radius changes four fifths of what it would with its centre
  held there, the centre moving along the bisector for the rest. Batch 160, at other angles between the lines (the
  sines of their half-angles 0.12 to 0.40), finds the share cos²(2θ) for lines 2θ apart, to 1e-5. The engine now gives such a
  circle (nothing but tangencies to two lines, and the ends of those lines held on it; not the first of them by x
  where no circle has a radius given) its place and radius after the other lines' turns, and settles it so between
  the lines after solving. Batch 147's cam, its lines' ends held on the circles, needs the lines' ends to slide where
  they're held on a circle the line is tangent to (the held line keeps only where it lies and its direction); a line's
  end held on a circle it isn't tangent to still holds, the circle moving to it (batch 150 c004). The cam matches now.
- **Fills with tangency conditions** (batch 157): across a gap between a level sheet and one rising at 26.6° (lines
  along its sides), a fill tangent to the rising sheet only is the plane, as Onshape's plain fill is (that plane
  already continues the sheet); Onshape faces it up (0, 0.894 up), the engine down. The engine now fills flat where a
  fill's tangent edges lie along flat faces in the loop's plane, its edges listed as std lists them (those with no
  tangency first), so c003 is c004 again. Tangent to both, Onshape's heights
  across the gap are within 0.02 mm of the cubic leaving each edge along its sheet with the chord's length (11.18),
  but they vary along the gap (0.509 at 5 along, 0.476 at 10, both 2.5 in), which the setup doesn't, and the middle of
  its parameters lies 3e-4 mm off centre: a surface of its own fitting. The engine doesn't build tangent fills yet
  (`edgesG1`). Known gap.
- **Which way flat fills face** (batch 158): a flat loop of sketch lines filled faces as the loop runs, anticlockwise
  about its normal, in the top plane, an upright one and a tilted one, drawn either way, and whatever order its edges
  are listed in; the engine's faces so, its edges now taken as Onshape's run. Batch 159: a rectangle drawn half each
  way faces as the side listed first runs, even against three others (c009–c011). Batch 157's gap between a level
  sheet and a rising one faces up however its edges are listed, the level sheet's, the rising one's or a line first
  (159 c006–c008): as if sewn to the level sheet, its loop running that sheet's edge against the sheet's own loop,
  though both sheet edges run the same way (+y) and the rising sheet's face points down. The engine now faces a flat
  fill with sheets' edges in its loop as sewn to the oldest of those sheets, and batch 157's flat fills match.
- **Boundary surfaces at magnitudes other than 1, found** (batches 162, 163): the extra term appears between
  straight profiles too, wherever the knots across leave interior rows of poles; it is (magnitude − 1) times the
  derivative times a fraction set by the knots alone (two even spans 25/29 at the middle row; three 0.933610; four
  0.910689, 0.994839; five 0.917563, 0.973058), whichever way the profiles run or are listed. Onshape fits the
  derivative leaving a conditioned profile, as poles across, to the magnitude times the magnitude-1 derivative at
  every knot, in least squares with a pull of 0.04 toward the magnitude-1 derivative's poles, the first two and last
  two held exact; this reproduces every fraction to 1e-6 and batch 161 c008's poles to 3e-6, and batch 117's arcs
  along their normal exactly (their small parts along the profile 0.01 off, the targets there presumably the exact
  projected chord at the knots). Only the conditioned end moves: with both ends conditioned the other's poles stay
  (163 c008); with the other end free, its quadratic's middle pole moves with it. Onshape builds these surfaces pole by
  pole on the knots across the profiles: a line raised to a cubic on the other's knots; an arc as its own cubic of
  even spans over even angles (four for batch 159's 87°), a least-squares fit with its ends and end derivatives exact
  (to 9e-6 of Onshape's poles). The engine now builds boundary surfaces conditioned tangent to planes so, between
  lines, arcs and cubic splines, and batches 159 and 160 match exactly (163 c001–c004, c006–c009; 161 c007–c009; 162
  c001, c004–c009). On uneven knots (162 c002, c003; 163 c005) the fit is sampled not at the knots but at the fitted
  poles' Greville abscissae (the same points on even knots): with one free pole the fraction is S/(S + 0.04), S the
  square of its basis function there, which gives 0.858215 and 0.854636 as Onshape's; every case of 162 and 163 now
  matches. Normal-to-profile conditions keep the engine's sampled construction (batches 063,
  117, loosely): there Onshape's magnitude-1 derivative, pole by pole, has its first two and last two rows the exact
  projected chord's value and slope at the ends (to 2e-6, batch 117 c001), and its interior within 5e-3 of a least-
  squares fit of the exact projected chord at the knots pulled 0.04 toward the pole chords projected across the
  control polygon; what exactly it is pulled toward isn't found. At magnitudes other than 1 the engine now builds
  normal-to-profile conditions between curved profiles pole by pole too, the magnitude-1 derivative there the chord
  less its part across the profile within its plane, fitted as above and then fitted to the magnitude as between
  lines: batch 117 c002 (magnitude 2) matches Onshape's body to 1e-3 (its poles 5e-3 apart), c006 (0.5 and 1.5) to
  0.0056 mm² of its area, and batch 171 c004 (magnitude 2) to 0.035 mm², as near as its magnitude-1 cases. Along a
  direction given (117 c004), the free end that isn't the quadratic's is batch 159 c005's: the quadratic plus each
  end's correction (what the cubic reaching the free profile along the chord differs from it by) blended along the
  profiles, its derivative across the middle (0, 20.082, 3.836) against Onshape's (0, 20.0821, 3.8352) where the
  quadratic's is (0, 22.36, −0.62) and the chord (0, 20, 4); Onshape's surface, an adaptive fit (knots halving toward
  the ends), is 734.8770 mm² and the engine's so 734.8187. Taking its fit as the magnitude's (Greville points,
  pull 0.04), the anchor Onshape's poles imply is no projection of the pole chords (its third row (0.016, 20, −0.001)
  against the chord's (0.907, 20, 1.567)), and no direction projects the third pole chord onto Onshape's row: its
  interior comes from a fit of another kind. A sheet extruded from a straight spline is a plane in Onshape, its edges splines still; the engine's
  is now, and a flat face of any kind matched next to a profile counts as its plane (163 c007–c009).
- **Fills meeting curvature conditions and guides as planes** (batch 161 c005, c006): batch 157's gap filled with
  curvature continuity to the rising sheet (which the plane continues) is the plane, as is a rectangle filled with a
  guide point in its plane. The engine now lets curvature edges through where the flat fill continues their faces, as
  tangent ones, and guides that lie in the flat fill's plane; it takes std's `guideEntities` (it had read only
  `guideVertices`, which std's fill doesn't pass, so guides were ignored) and refuses other guides.
- **Onshape's own fills** (batch 161 c001–c004): its three-sided fills and fills tangent to sheets they don't
  continue are bicubic B-splines over a plane, their knots in metres across the loop's extent in that plane (batch
  157's gap: ±10.0025 mm along it, ±5.59 across), refined unevenly (spacings halving toward some edges): adaptive fits.
  A thin plate (least bending energy, positions on every edge, slopes on the tangent ones, solved by a converged spline
  Ritz method) dips 0.11 to 0.16 mm below Onshape's heights, so it isn't Onshape's surface (an earlier, unconverged
  difference grid had seemed within 0.07). Across the middle of the gap Onshape's is near the cubic with chord-length
  tangents, and toward the side lines it dips less (96% as much a quarter of the way along): no Coons or Gordon
  blend of those tried gives that. Known gap.
- **Boundary surfaces leaving short arcs normal to their planes** (batch 171): on a 45° arc (two spans, one row free
  inside) the derivative leaving the arc is the chord less its part across the arc at the ends, to 1e-6, and lies in
  the plane of the plane's normal and the arc's tangent throughout; but its part along the tangent runs 18 to 60%
  larger in between than the chord's own (a quarter along c001's arc, 1.395 against 0.955), so it is neither the exact
  projection interpolated at the Greville points (as tangent conditions are), nor fitted to it by least squares with
  or without the ends' values and slopes held. Not linear or cosine in u between the ends either. Known gap: the
  engine's sampled construction stays (its areas within 0.04 mm² of Onshape's), and magnitudes other than 1 on normal
  conditions stay refused.
- **Lofts from profiles of several faces** (batch 169): a region split by a line across it, both faces taken as one
  profile, lofts as the whole region: the faces merged and their boundary's collinear edges made one each (a split
  rectangle to a rectangle, or to another split one, is the plain six-faced loft). The engine now merges them so. A
  20 × 10 rectangle lofted to a circle had been 0.23% of the volume off Onshape's, split or not; batch 170's poles
  show why: Onshape splits the circle into arcs as long as each other, from where the ray through the corner first
  round anticlockwise from the circle's start meets it, each side ruled to one (the rectangle's corners (10, 5) and
  (−10, −5) keep their angles, 26.57° and 206.57°; the other two go to 116.57° and 296.57°, not their own 153.43° and
  333.43°). A square's corners are at its quarters anyway (batch 004 c039). The engine now splits so, and matches.
- **Trimmed conic segments; sheets up to curved sheets** (batch 168): a sketch conic's `lowParameter` and
  `highParameter` change nothing in Onshape: the conic runs from its start to its end, peaking at rho times the control
  point's height (5, 3 and 8 for rho 0.5, 0.3, 0.8), and std gets no B-spline for it, a parabola (weight 1, which OCCT
  keeps non-rational) included. The engine now ignores them (it refused them) and reports every sketch conic so. A
  surface's second direction up to a curved sheet ends on it, as up to a face.
- **Face patterns of bosses and pockets together; sheets up to curved faces** (batch 167): a boss's and a pocket's
  faces patterned together come out as the bosses added and the pockets taken away; where a boss's copy meets a
  pocket (a quarter turn standing it over the pocket, their faces touching) Onshape fails (`PATTERN_FACE_FAILED`), as
  the engine now does. A surface extruded from a line with its second direction up to a lying cylinder ends on the
  cylinder, as a solid's does; the engine sweeps it from far behind and keeps the faces the profile lies on.
- **Boundary surfaces on profiles of several edges** (batch 166): edges in a row along one line are one line to
  Onshape, the surface as from a single edge, both for the conditioned profile and the other (c001, c002, c004); a
  sheet extruded from two such lines is one face too, as a region's collinear lines sweep one (the engine's had made
  two). Along faces in different planes (a level sheet's edge then a dipping one's, c003) the surface gets a knot where
  the planes change, and the derivative leaving the profile is a spline whose first two and last two poles are its
  value and slope at the ends and the rest its values at their Greville points, the earlier face's at the junction
  (there −2.5 along z, putting the curve level at the junction, the dipping face's slope beyond). On one plane that is
  the pole-by-pole projection the engine used, so it builds every tangent condition so now.
- **Variable fillets with radii at points** (batch 165): along the edge the radius runs through the ends' radii and
  the points' (at fractions of the edge's length along Onshape's way of it). Smooth, each stretch between two is the
  S curve 3s² − 2s³, level at both (2 to 4 halfway: 2.056 at a tenth, 2.208 at a fifth). "Straight" is level at each
  point and bends nowhere at the edge's ends: from an end 1.5s − 0.5s³, to an end 1.5s² − 0.5s³, between two points
  the S curve (2 to 4 halfway: 2.299, 2.592); with no points it is the line it was. Between planes the fillet is
  exactly the B-spline surface whose rows carry those spans' Bézier radii, one face; the engine now builds it so
  (joining spans at triple knots), and samples the law for OCCT elsewhere.
- **Revolves bounded at both ends, curved second-direction bounds, tangent propagation** (batch 164): with both
  bounds up to something, each bound is where it's first met going forward from the profile (as batch 024 found for
  a start bound alone), and the revolve runs forward from the start's to the end's: up to planes y = 5 and y = −5
  either side of the profile, from 194.5° on round to 374.5°, half the ring. A start bound in the end's direction
  (`startOppositeDirection`) behaves the same: a point 20° on with the end 60° on, the revolve runs from 20° to 60°.
  An extrude's second direction up to a curved face or a curved sheet ends on it as an end bound does (Onshape's box
  of the one to a cylinder stops 1.6e-3 mm short of its lowest point, its own bound on that curve). Sheet metal
  thickened from a flat with tangent propagation takes in the tangent quarter turn as a rolled wall, the two parts
  0.02 apart, as the engine's do once its surface extraction propagates.
- **Which way an extruded sheet's edges run** (batch 159 c012): its profile's copy and the far edge run as the
  profile does; the side edges swept from the profile's ends run out from the profile along the extrude, where a
  solid's run back from the far end (batch 088). The engine now runs a sheet's so.
- **Boundary surfaces matching flat faces, and between a line and an arc** (batch 159): matching the tangency of a
  flat face next to a profile is tangent to the face's plane, as tangent to a profile's own plane is (c001–c004, also
  at magnitude 2 and both ends). From a level sheet's edge up to an arc whose ends are lower than its middle (c005),
  one end free, Onshape's curve across the middle is 14v² − 5v³ high: the quadratic's 9v² plus 5v² − 5v³, the
  cubic reaching the line through the arc's ends along the chord less the quadratic there. So its surface is the
  quadratic's raised to a cubic plus a correction blended along the profiles from their ends, each end's what the
  curve across there (as between straight profiles) differs from the quadratic by; where those agree, as between
  batch 063 c011's arcs meeting at their ends, it is the quadratic, cubic across as Onshape's poles are. The engine
  builds it so, to Onshape's arc fit (0.0015 mm²). Given as v profiles only (c013, c014), the same surfaces.
- **Faces split normal to curved targets** (batch 133): a sketch projected normal to the target goes along the
  target's normal through each of its points onto the target's surface: a circle and a rectangle beside a cylinder, a
  circle above a sphere, a circle beside a cone frustum. The rectangle taller than the cylinder splits its side across
  the seam, into two faces as Onshape's (the seam merged away). The engine builds each projection as a ruled sheet
  between the curve's points carried each way along the normals, splits the face with it, then merges faces across
  edges on the target's seams.
- **Everyday parts** (batch 134): a flange with patterned bolt holes, a ribbed bracket, a keyed shaft, a slotted plate
  chamfered round its slot, a pocket filleted at its corners and floor, and a cup shelled open match exactly.

- **Face blend options round a cylinder; chamfers; swept profiles** (batches 172, 173). Between a floor and an
  upright cylinder: detached, the blend (the torus) is a sheet of its own and both sides stay whole (172 c001); two
  radii set the ball's centre `radius` above the floor and `secondRadius` out from the wall, so it touches the wall
  `radius` up and the floor `secondRadius` out, a quarter ellipse swept round (c002, c006, switched); a width is the
  chord across, 2√2 up and out for 4 (c003); untrimmed, the blend round a closed wall has no ends and is the trimmed
  one (c004); limited by a plane through the axis Onshape can't attach it (`FACE_BLEND_CANNOT_ATTACH`, c005); a
  chamfer section is the straight line across, a cone round the cylinder (c008) and a plane between planar sheets
  (c009, 5√2 wide for 5 mm). Onshape's B-spline blends round the cylinder (two radii, conic, curvature, swept) are
  its own: round the axis a cubic through the circle every 22.5° from 45°, matching its tangent there (knots the
  angles in radians, doubled) for the rational sections and interpolating it C2 (uniform knots) for curvature ones,
  0.02–0.03 mm² under the exact surface; and the floor's edge along them is a curve 1e-4 longer than its circle
  (`OTHER`), the floor 0.11–0.14 mm² smaller than with the exact circle, so these cases are compared to 0.15. A swept
  profile takes the rolling ball's circle in the plane square to the spine and carries it straight along the
  corner: along a spine parallel to the corner the blend is the rolling ball's (c010; a B-spline, the section exact
  and a cubic along running a tenth past each end), along a spine slanting across it (by 20.1°) that circle seen
  along the corner, a quarter ellipse touching the level sheet 4.6944 out and the upright 5 up (c011); along the
  cylinder's base circle the rolling ball's again (173 c005). The engine builds all of these, its blends round the
  cylinder exact, those along lines with Onshape's poles. Checked further (batch 173 c001): Onshape's blend face is
  the rational section across times, round the axis, the C1 cubic through the circle every 22.5° from 45° with its
  exact tangent (r per radian), each section pole carried round so: 484.07038 mm² against Onshape's 484.070338 (the
  exact revolve 484.0978). Its floor edge is not that surface's floor row, though: the row is 3.3e-5 shorter than
  the circle and the edge 1.0e-4 longer at floor radii 15 and 13 (enclosing 2.0e-4 more), as a circle 1e-4 of its
  radius out, by a rule not found; so the engine keeps its exact blends and these cases stay loose. (173's probe
  lines stop in its faces' weights, Onshape's output cut short, so their poles can't be compared whole.)

- **Lofts along a straight path through sections, found** (batches 075 c006, 112 c004–c006). Onshape adds the
  sections at even steps along the loft, k/(N + 1) for N sections (its surface has N + 1 spans along: 4, 6 and 10
  for 3, 5 and 9), each the plain loft's circle there as a cubic through it every 45° from the circles' starts with
  its exact tangent (r per radian), which dips 9.8019e-4 of the radius between: at every height that is a section
  Onshape's radius every 22.5° is 9.81e-4 short. Along the loft it runs a natural cubic spline through the profiles
  (exact circles) and those sections at even parameters: between sections its radius there is the spline's to 4e-7
  (0.9989356 of the cone's at a third of the way for 3 sections, the spline's 0.9989360; not-a-knot 0.9989684,
  clamped to the cone's slope 0.9988823). Between equal circles Onshape's loft stays a cylinder (089 c009), its
  sections presumably cut from the plain loft's surface, which is a cylinder there and a B-spline otherwise. The
  engine now builds it so (round the axis fitted C1 through 129 points, along it the spline exact), and these cases
  match on their bodies.
- **Lofts with guides elsewhere** (batch 174; batch 112's poles again). Between a 20 mm square and a 10 mm one 20
  above: guides crossing two sides (opposite or neighbouring) bend each alone, as one does (c001, c002). A guide
  crossing a side off its middle (c003: in the plane y = −2.5, a fraction 0.375 along the bottom edge and 0.25 along
  the top; c004: a quarter along both) puts the knot across at the mean of its two fractions, doubled; the bottom
  and top edges are each their straight line with that knot at the guide's end, each part keeping its own speed and
  the knot the geometric mean of the two (to 1e-6); the poles across are ruled between theirs, the two about the knot
  moved by the guide's offset from its chord, so that the surface passes through the guide at its own parameter and
  keeps the ruled side's slope across it. Only through both edges' middles is the knot single (C2 there: the
  middle pole moved by the offset over its basis function, batch 112's rule). Along a corner (c005), both sides
  there are one cubic span across, run as their bottom edges run, the two poles at the guide's end moved by the
  offset; Onshape's edge along the guide is no B-spline to std. Along the guide each row is a C1 cubic through
  even stations, the fewest spans (a power of two) whose cubic through the guide's points and tangents keeps within
  1e-3 mm of it: 8, 4 and 2 along batch 112's arcs of 111°, 73° and 22° (1.1e-4 to 4.7e-4 mm off, half as many
  1.8e-3 to 7.5e-3), 8 along the corner's 121°; along a fit spline Onshape places its own knots (0.4375, 0.5 for
  c004's). Between coaxial circles a guide in a plane through their axis is revolved (c007): an arc gives a
  torus, here a spindle torus (its circle's centre 2.06 mm across the axis, `DEGENERATE_TOROIDAL_SURFACE` in
  Onshape's STEP), the circles starting at the guide. With the loft normal to its first profile (c006), Onshape refits
  every side's knots across (sixteenths across the bent side), but under the fit the bent side is exact: its
  sections along the loft, each the cubic from the bottom edge's point with the first profile's derivative D(u)
  there and a free (natural) end at the top, plus the guide's offset from the middle section blended across as
  without conditions, S = P(u, v) + N₂(u)(G(v) − P(t, v))/N₂(t), to 1e-5 mm. D(u) is the condition's at the
  corners ((0, 0, 20.12), the plain loft's) and the guide's own tangent where it meets the profile ((15.99, 0,
  18.18): 24.21 mm, its arc's length), and between them neither a blend of those two vectors (its x and z parts
  go apart: 0.196 and 0.207 of the way an eighth along) nor a polynomial; it runs on smoothly round the profile's
  corners onto the neighbouring sides (along them, out of their planes not at all: their poles move within them,
  their shapes as without the guide) and leaves the far side alone. The engine builds the bent side so on its own
  plain loft's sides, D constant along each, 0.14% of the volume under Onshape's (6006.09 mm³ to 6014.34): Onshape's
  D is its plain loft's rule, not yet found (batch 056 c001's). Through three squares (c008) the bent side is the
  plain loft's with the same single-knot bend, the guide's parameter carried evenly between where it meets each
  profile (its middle-profile point at the loft's knot there, to 4e-8), each stretch fitted apart (2 and 4 spans
  along its 46° and 56°, within 1e-3 mm as above). The engine builds it so, but its plain loft through three
  polygons puts the middle profile at 0.4393 where Onshape's is at 0.4511 (and batch 062 c008's at 0.4083), by a
  rule not yet found (the chords at the edges' middles come nearest, 0.4519 and 0.4083), so c008 is 0.14% off.
  Ruled out on these two (0.451083927 and 062 c008's 0.408306989, which has an end condition): the chords at the
  corners, chords averaged along the edges (at their ends, middles, 2 to 8 even points, by mean, RMS or mean of
  fractions), the chords raised to a power (no one power fits both), and arc lengths of the natural spline through
  the edges' middles, chord-spaced or iterated; batches 175–179 below narrow it. The engine builds c001–c005 and
  c007 to Onshape's poles (c004 to 0.007 mm³), and batch 075 c005 and 112 c001–c003 now exactly; c006 and c008 as
  above.

## Lofts through three or more profiles: where the middle profiles go (batches 175–179)

- **The surface** across the profiles is the natural cubic spline through them (second derivative 0 at both ends, to
  1e-5), one knot at each middle profile, which the surface passes through at that knot (to 4e-7). End conditions
  don't move the knots: batch 175 c002 (free) has 062 c008's knot (tangent to its first profile) to 1e-9.
- **Each stretch between two profiles has a length F of its own,** and the knots are the running sums of the F's,
  normalised: batch 176's stretches alone predict 175 c008's four-square knots, 0.324054909, to 4e-8. F depends only on
  how the points move between the two profiles, not on the profiles' size (a 20 mm square growing 2.5 mm each side
  over a 10 mm rise as a 40 mm one, 176 c001 and c006, to 1e-9), not on the order (c009), and a shrink as the same
  growth (c005).
- **F is the chord when every point moves alike:** a square moved across (176 c007, F = √125 exactly), coaxial circles
  growing (177 c007), and 175 c006, whose two stretches' chords are 1:3 at every point (knot 0.25 exactly).
- **Otherwise F sits just above the smallest chord** between corresponding points (the edges' middles for growing
  squares), by an amount growing as the fourth power of the spread: squares growing Δh each side over a rise of 10
  (176) give F = 10.308978 (Δh 2.5), 11.194486 (5), 12.547485 (7.5, from 175 c001), 14.239562 (10), 18.247850 (15,
  from 175 c007), 22.710443 (20); Δh 2.5 over 20 gives 20.155811. With m the smallest chord and c the corner's,
  (F − m)·m/(c − m)² runs from 0.1405 to 0.1340.
- **It isn't a function of the chords' lengths alone.** No weighting of the corresponding chords (any positive weights
  along the edges, power means, a one- or two-point rule, the chords at Chebyshev, Gauss or Lobatto points fitted by
  a polynomial and minimised), no arc length (of the natural spline through the edges' middles, iterated, or along
  Onshape's own surface at any column), no surface area share and no bending or stretching energy fits. Sheets
  through three straight lines (single faces, 178 and 179: the first stretch moving the line 5 across, the second
  rising 10 while each point moves along the line by a + w·x, x from −1 to 1) give, for a line growing evenly,
  F = 10.000151 (w 1.25), 10.002343 (2.5), 10.033441 (5), 10.143541 (7.5), 10.373700 (10), 11.247854 (15),
  12.624814 (20), 16.434734 (30): (F² − m²)/m² is (w/m)⁴/8 to leading order. Growing from one end (a = w = 2.5,
  178 c003) gives 10.019309 against 10.002343 evenly, though its chords span the same lengths; growing while moving
  along (a 1.25, 5, 10 at w 2.5) gives 10.005113, 10.393572, 12.703054, the last two each the chord about 0.8666 of
  the way from the middle to the nearer end (√3/2 is 0.8660).
- **A closed polygon whose faces are all alike is the single face with w scaled by √3/2:** the squares above to
  1.5e-7 at Δh/rise 1/8, 7e-7 at 1/4, drifting to 4e-4 at 2; triangles growing (177 c008) to 3e-6. Rectangles don't
  follow it: one growing one way (177 c004, F = 10.004390; 177 c005, 10.061360) or both ways (176 c008, 10.352939)
  sit further above their smallest chord than either rule gives, and a square growing from a fixed corner (177 c006)
  gives 10.004769. The rule behind all this isn't found.

## Loft connections between a polygon and a circle (batch 180)

- **One connection**, a polygon's corner or a point along its edge to a point round a circle: the polygon's corners
  and the connection point (where it's along an edge) divide the circle into as many equal arcs, from the connected
  point round the same way (anticlockwise along the way from the first profile to the second), each taken to the
  next in turn (c001, c002 twisted a quarter, c004 the circle first, c005 a hexagon, c006 an edge's middle: five arcs
  of 72°, the edge taking two). Each side is ruled (degree 1 across) between its edge and its arcs, both as cubic
  Hermite spans, four for each arc: the arc's from its own tangent, as long as the span's angle times the radius over
  three (not the best fit's (4/3)·tan(θ/4)·r), the edge's evenly, the knots doubled. The circle gets a vertex at each
  point, the connection's too (c006: 13 edges). The engine builds this pole for pole (to 1.7e-4 mm); Onshape's
  circular end, though, measures 314.1631 mm² where its sides' arcs bound 314.1386 (and the exact circle 314.1593),
  so the areas are 0.025 mm² and the volumes up to 0.09 mm³ apart.
- **Two connections** (c003, the square's opposite corners to an eighth and five eighths round; 181 c007, c008):
  round from each connection to the next, the circle's arc between them is shared evenly among the polygon's edges
  between (181 c008: 135° over two edges, 225° over two), and each arc takes eight spans (halved till close: batch
  182 below).
  Onshape's circular end measures 314.1595 mm² (its sides' finer arcs bound nearly the exact circle); the engine's
  within 0.002 (0.01 mm³ of 181 c008's volume).
- **A triangle to a square**, corner to corner (c007): the triangle's edge opposite the connected corner split at its
  middle, a vertex there, the square's fourth corner taken to it; as polygons with different numbers of corners
  below.

## Lofts between polygons with different numbers of corners (batch 181, 180 c007)

- From the first profile's first corner (Onshape's: where the engine's loop's first edge ends) to the nearest corner of
  the other, of two as near the later round (c001: a triangle's at 90° to a square's at 45°, not 135°; c003: to a
  hexagon's at 120°, not 60°), both polygons run anticlockwise along the way from the first to the second and are
  measured by the fraction of their perimeters from there. Each corner of the polygon with fewer goes to the other's
  corner nearest it so measured (in order; of two as near the earlier), and each of the other's corners left over to
  the middle of the edge of the fewer it falls in, a vertex there (c001 and c002 differ: a triangle to a square splits
  the triangle's edge opposite its first corner, a square to a triangle the one after; a triangle to a hexagon every
  edge; a square to a pentagon its top edge, at its middle though the fraction would put it 2 mm off). A connection
  fixes the first pair (c006, 180 c007). The sides between straight edges are ruled; twisted ones Onshape writes cubic
  along the edges (poles at thirds), linear across, both from 0 to 1. The engine builds these exactly.
- **A polygon connected to a circle twice** (c007, c008, 180 c003): as in batch 180's section above; the engine builds
  them so.
- **Several connections between polygons** (batch 182 c001–c003): each stretch from one connection to the next is
  matched as a whole loft is above, by fractions of the whole perimeter from the first connection, summed edge by edge
  (c002: a triangle to a square, the square's corner between the connections taken to the middle of the triangle's
  edge; c003: two squares a quarter turn round). Where a stretch's corner falls exactly between two of the other's
  (c001: a square to a hexagon, connected at opposite corners), Onshape's rounding picks the earlier in one stretch and
  the later in the other: the same sums in Python's floating point do, the engine's (its hexagon's corners from its
  sketch) the earlier in both.
- **Circles connected twice** (182 c004, from their starts and from half round the first to a third round the
  second): Onshape's side is one periodic face from the first connection, each circle split at the connection points
  (4 edges, 4 vertices). Its u puts each connection at the mean of the circles' fractions round from the first (5/12,
  between a half and a third); each circle's angle is a cubic in u from one connection to the next (Hermite), its slope
  at each the geometric mean of the stretches' either side, each its fraction over its u (√(1.2 × 0.857) = 1.0142
  turns per unit of u round the first circle, √(0.8 × 1.143) = 0.9562 round the second). Spans: 16 to the first
  stretch, 32 to the second (below). The engine builds this pole for pole; Onshape's ends, though, measure 314.1594
  and 314.1599 mm² where its side's edges bound 314.1581 and 314.1591.
- **More connections to a circle** (182 c005, a pentagon three times; c006, a square at a corner and along an edge):
  the arcs as above, and the same correspondence along each side: c006's side along y = −10, the connection at its
  middle, puts it at u = 39/76, the mean of the edge's half and the arcs' 75° of 142.5°, the edge's and the arc's
  fractions each a cubic in u either side, sloped at the connection as above and at the corners as their own stretch.
  Each span from one connection or corner to the next is halved until its cubic at its middle lies within a tolerance
  of the curve: 0.11 µm with more than one connection (c006's spans 0.100 and 0.107 µm off kept, 0.113 and 0.144
  halved; c005's 54° arc in four spans, its 72° and 81° in eight; two connections' 90° and 67.5° arcs in eight, 181
  c007, c008), ten times that, 1.1 µm, with one (186 c003: 1.00 µm kept, 1.15 halved; batch 180's 90° arcs in four
  spans 0.62 µm off). The engine builds these so.

## Lofts with connections, checked (batch 186)

- The rules above hold on cases they weren't found on: circles of radii 10 and 6 connected twice (c001), a hexagon to
  a circle of radius 15 twice (c005), a triangle to one of radius 5 at a corner and 0.3 along an edge (c006), and a
  square to a circle once, a quarter along an edge (c003: the connection at u = 0.375, the mean of the edge's 0.25 and
  the arcs' half; its spans halved to 1.1 µm, seven and six either side).
- **Where a periodic side starts:** circles connected three times (c002: from their starts, 0.3 to 0.25 and 0.7 to
  0.6) give a side starting at the third connection, which ends the longest stretch (0.375 of the mean fractions; 182
  c004's two connections start at the first, which ends the longer, 0.583). The engine starts there.
- **A square connected at every corner** (c004, to 0.1, 0.3, 0.6 and 0.85 round): Onshape runs every side clockwise
  round the loft, where every other connected loft so far runs anticlockwise; the surfaces are otherwise the
  engine's (areas within 3e-5 mm²). Why isn't known: compared on the body.
- Sketch text: Roboto Slab Bold (1.100263, as its Regular), and "Rf&9" in Roboto Slab, Noto Sans regular and bold
  italic, Noto Serif italic and bold, Arimo Bold, Allerta Stencil and Droid Sans Mono, match.

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
  metres. Known gap: two balls united and the circle where they meet filleted (batch 132 c010), the larger sphere's
  seam ending on the fillet's edge, which OCCT splits in two there: read back, every face keeps its area and
  orientation but the part's volume comes out 3099 mm³ for 5895 (the sphere's trimmed region taken on the other side
  of its seam, it seems). Onshape's STEP of the same part reads back right.
