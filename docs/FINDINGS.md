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
- **Assumptions not yet checked against Onshape:** transient ids are strings (ours look like `"T12"`); the
  results of `@evaluateQuery` come in body order, then faces, edges and vertices; a solved sketch makes one sheet
  body for its regions, one wire body for its edges (and one for construction edges), and a point body for each
  `skPoint` and each circle or arc centre. std's sketch documentation describes those kinds of body but not how
  many.

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
- **Only solids are exported** in batch 002's file; construction planes and points aren't. (Batch 002.)
- **The engine's own STEP** is AP242 in metres, with each part's name on its product and, like Onshape's, on its
  `MANIFOLD_SOLID_BREP`. OpenCascade writes shapes as millimetres unless the document's length unit is set to
  metres.
