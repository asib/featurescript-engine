# Milestone 1 plan: std's extrude running locally

**Objective:** std's own `extrude.fs` and `sketch.fs`, unmodified at version 2960, run on our Rust interpreter with
the `@` built-ins they need written on OpenCascade. About 100 sketch-and-extrude cases give the same measurements
locally as in Onshape, and Onshape's results come from a single batch.

**Done means:** `cargo test` runs the whole batch locally and checks it against the stored Onshape results. Each
case either matches within tolerance or appears in a list of explained kernel differences. Load time and time per
case are measured and written down.

## Assumptions

- **std 2960 from the community mirror, not from Onshape.** We use the mirror's `main` branch, which keeps the
  version numbers in imports, so getting std costs no Onshape calls. Fetching the current std from Onshape would
  cost about 266 API calls per version, one for each file. The oracle's test Feature Studio imports
  `onshape/std/common.fs` at version `"2960.0"`, so Onshape and our engine run the same FeatureScript. The
  version checks inside std pass the same way on both, because no 2960 check names a version later than 2960. The
  batch also records `getCurrentVersion(context)` to show which version Onshape's own built-ins run at.
- **Sketches give explicit coordinates and have no constraints or dimensions.** With nothing to solve, `skSolve`
  only has to join edges into loops and split them into regions, as Onshape does. A real solver (planegcs) comes in
  a later milestone.
- **Extrude features in scope:**
  - profiles made of lines, arcs and circles, including regions with holes and several regions per sketch;
  - blind, symmetric and two-direction extents;
  - new, add, remove and intersect operations, against bodies an earlier step of the same case created.

  Out of scope for now: draft, thin extrudes, "up to" extents, constrained sketches, feature patterns and selections
  made in the UI.
- **The test document is private.** The account is on the Student plan, which allows private documents; check
  this when creating it.

## Steps

1. **Vendor std and set up the workspace.** Copy std 2960 from the mirror's `main` branch into `std/`, with its
   licence and a `NOTICE` naming the source commit. Create a Cargo workspace with a first crate `fs-lang` for the
   lexer, parser and syntax tree; add further crates when a step needs them. *Check:* `cargo build` passes and std
   is committed unchanged.
2. **Parse all of std.** Write a lexer and parser for the full FeatureScript grammar that produce a syntax tree
   with source positions. *Check:* a test parses all 265 files with no errors, and a pretty-print round trip gives
   back the same tree. Record how long a full parse takes; if it is slow, plan to cache parsed code.
3. **Build the oracle harness and batch 001.**
   - A generator with a fixed seed writes about 100 cases as FeatureScript. Each case builds a sketch and runs one
     or two `extrude` calls.
   - A measurement feature (`harness.fs`, using only std functions) runs every case in its own id namespace and
     catches each case's error. For each case it records:
     - the error or status;
     - body count, volume, area and bounding box;
     - face, edge and vertex counts by geometry type;
     - a sorted list of faces, each with its type, area, centroid and normal;
     - result counts for the queries extrude depends on (`qCapEntity`, `qNonCapEntity`, `qCreatedBy`).

     It deletes each case's bodies before the next case starts.
   - Small Node scripts drive the logged-in Chrome over its debugging port. Once, they create the test document;
     after that they paste the Feature Studio text, add the feature, and read the results back.
   - Results come back with one API call per batch: `evalFeatureScript` reads a variable the harness set. Before
     using it, try reading the output from the page itself, which costs no API call. Keep whichever is reliable.

   *Check:* `tests/oracle/batch-001/` holds the cases and Onshape's results, together with the std version,
   Onshape's build and the date. The README records how many API calls the batch cost.
4. **Interpret std.** Write a tree-walking interpreter for the full language:
   - versioned imports;
   - types with check predicates;
   - overloading of functions by argument type, and of operators;
   - value semantics for arrays and maps, `box`, lambdas, enums and preconditions;
   - `try`/`catch` and regen errors.

   Every `@` built-in goes through one registry. A built-in we haven't written throws `not implemented: @name` and
   is logged. Write the non-geometric built-ins (maths, strings, maps, matrices) as std calls them. *Check:*
   `common.fs` loads, and units, vector and transform arithmetic give the expected values. Running the batch-001
   cases prints the exact list of geometric built-ins still missing. Record the load time.
5. **Write the OpenCascade layer and the geometric built-ins.**
   - A crate `occt-sys` holds `cxx` wrappers that link the system OCCT 7.9.2. It covers:
     - points, curves and surfaces, plus the builders for edges, wires and faces;
     - prisms;
     - booleans with their history (`Generated`, `Modified`, `IsDeleted`);
     - topology exploration, mass properties and bounding boxes.
   - On top of that, write the built-ins that step 4's list asks for. These are expected to include:
     - the sketch entities, and `skSolve` (loops, holes and region splitting);
     - `opExtrude`, `opBoolean` and `opDeleteBodies`;
     - `evaluateQuery`, for the query types the extrude path uses;
     - the `ev…` measurements the harness calls.
   - Add entity identity and "created by" tracking, carried through each operation's history.

   *Check:* every batch-001 case runs to the end locally, or fails with the same error as in Onshape.
6. **Compare and close the gaps.**
   - Run `harness.fs` locally on the same cases. Diff the result against Onshape's: counts must match exactly,
     volume and area to a relative 1e-6, and faces are paired by type and centroid.
   - Make the batch a `cargo test`.
   - Write each convention we discover in `docs/FINDINGS.md`, with how it was found.

   *Check:* the "done" condition above is met.

Steps 1–2 are slice 1. Step 3 is slice 2: it is independent of the interpreter, and it tests the oracle assumptions
early, while the browser session is fresh. Steps 4, 5 and 6 are one slice each. Each slice is committed and pushed
to `main`, then reviewed before the next one starts.

## Risks and decisions

- **Pinning std to 2960 may not work.** Onshape might refuse the older import, or the recorded
  `getCurrentVersion` might show behaviour we can't match. Then fetch the current std once, about 266 read calls,
  but only after the user agrees.
- **Seam edges.** OpenCascade puts a seam edge on every cylindrical face, and Parasolid doesn't, so extruded
  circles give different edge counts. The native layer has to hide seam edges from queries and measurements.
  Expect more kernel differences like this one; each gets a note in `FINDINGS.md`.
- **Onshape's time limit for regenerating a feature.** If one feature can't run 100 cases in time, split the
  batch into several features in the same Part Studio, and read them all back with the same single call.
- **Interpreter speed.** About 94k lines of std load on every run. Step 2 measures parsing and step 4 measures
  loading; if either is too slow, add a cache of parsed code before going further.
- **Exact geometry checks** need a STEP export of the batch, which takes a few more API calls. They are left out of
  this milestone; per-face measurements come first.
