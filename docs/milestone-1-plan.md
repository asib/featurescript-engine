# Milestone 1 plan: std's extrude running locally

**Objective:** std's own `extrude.fs` and `sketch.fs`, unmodified at Onshape's current release, run on our Rust
interpreter with the `@` built-ins they need written on OpenCascade. For about 100 sketch-and-extrude cases, our
engine builds the same model that Onshape's engine built.

**How Onshape is used:** only to generate fixtures. Onshape builds each test case once, and we export the
resulting model file from its UI and commit it to the repo. Tests never touch Onshape. They run the same
FeatureScript on our engine and compare the models it builds against the fixtures, looking for discrepancies.

**Done means:** `cargo test` builds every case locally and compares each model against its Onshape fixture. Each
case either matches within tolerance or appears in a list of explained kernel differences. Load time and time per
case are measured and written down.

## Assumptions

- **std is copied from Onshape's UI, not through the API.** A script drives the logged-in Chrome through every
  Feature Studio tab of the latest released version of Onshape's public std document, and saves the text from
  each tab's editor. That is ordinary browsing and makes no API calls. The community mirror stopped at version 2960
  in May 2026, so it is only used to check the copy. The Feature Studio that generates fixtures imports the same
  std version we copied.
  *Done:* std 3112.0 (2026-10-30) is in `std/`, with 277 files and 150,805 lines. Since 2985, each file carries the
  version it last changed in, not the release version. Every import points at the copied file, so the interpreter
  can resolve `onshape/std/<file>` to `std/<file>` and ignore the version, as long as `tools/check-std.mjs`
  passes.
- **Onshape access rules.** We never create API keys and never call Onshape's REST API directly. The only API use
  allowed is the API Explorer page, in the logged-in browser, and as rarely as possible.
- **Sketches give explicit coordinates and have no constraints or dimensions.** With nothing to solve, `skSolve`
  only has to join edges into loops and split them into regions, as Onshape does. A real solver (planegcs) comes in
  a later milestone.
- **Extrude features in scope:**
  - profiles made of lines, arcs and circles, including regions with holes and several regions per sketch;
  - blind, symmetric and two-direction extents;
  - new, add, remove and intersect operations, against bodies an earlier step of the same case created.

  Out of scope for now: draft, thin extrudes, "up to" extents, constrained sketches, feature patterns and selections
  made in the UI.
- **Fixtures are STEP files.** Onshape exports STEP from the UI, and OpenCascade reads STEP natively. That makes
  STEP the practical exact (B-rep) format for comparing models. Parasolid (`.x_t`) would keep Onshape's own
  topology exactly, but free OpenCascade can't read it, so that would need a reader of our own. Meshes such as STL
  lose too much detail to compare faces and edges.
- **The fixture document is private.** The account is on the Student plan, which allows private documents; check
  this when creating it.

## Steps

1. **Vendor std and set up the workspace.** Copy the latest released std into `std/` with
   `tools/browser/fetch-std.mjs`, with its licence and a `NOTICE` naming the document version it came from (done:
   3112.0). Create a Cargo workspace with a first crate `fs-lang` for the lexer, parser and syntax tree; add
   further crates when a step needs them. *Check:* `cargo build` passes and `node tools/check-std.mjs std` reports
   no problems.
2. **Parse all of std.** Write a lexer and parser for the full FeatureScript grammar that produce a syntax tree
   with source positions. *Check:* a test parses all 277 files with no errors, and a pretty-print round trip gives
   back the same tree. Record how long a full parse takes; if it is slow, plan to cache parsed code.
   *Done* (crate `fs-lang`): all 277 files parse and survive the round trip. In a release build, lexing and
   parsing all of std (150k lines, 827k tokens) takes about 67 ms
   (`cargo run --release -p fs-lang --example parse_std`), so no cache of parsed code is needed yet. Onshape
   publishes no operator precedence table, so the table at the top of `parser.rs` is an assumption. It is based
   on C conventions and on how std writes its expressions, and step 3 has cases that check it.
3. **Generate the batch 001 fixtures in Onshape.**
   - A generator with a fixed seed writes about 100 cases into `fixtures/batch-001/cases.fs`. Each case is a
     FeatureScript function that builds a sketch and runs one or two `extrude` calls at the origin, with its booleans
     scoped to its own bodies. The same file runs unchanged in Onshape and on our engine.
   - A few cases check the precedence table the parser assumes. Each one sizes a box by an expression whose value
     depends on precedence, for example a depth of `(-2^2 + 5) * millimeter` or `("a" ~ 1 + 2 == "a3" ? 2 : 1) *
     millimeter`. The fixture's geometry then shows how Onshape evaluated it.
   - A small generator feature calls every case in its own id namespace, catching errors. It moves each case's
     bodies to that case's slot in a grid and names them after the case (`c017`). A case that fails in Onshape
     leaves no body, so the fixture records the failure too.
   - Small Node scripts drive the logged-in Chrome over its debugging port, through the UI only:
     - create the private fixture document, once;
     - paste the cases and the generator into a Feature Studio;
     - add the generator feature to a Part Studio;
     - export the Part Studio as STEP from the export menu, saving the download as
       `fixtures/batch-001/onshape.step`.

   *Check:* the STEP file holds one named part for each case that Onshape built. A note next to it records the std
   version, Onshape's build, the date and any cases that failed in Onshape. No API calls are made.
4. **Interpret std.** Write a tree-walking interpreter for the full language:
   - versioned imports;
   - types with check predicates;
   - overloading of functions by argument type, and of operators;
   - value semantics for arrays and maps, `box`, lambdas, enums and preconditions;
   - `try`/`catch` and regen errors.

   Every `@` built-in goes through one registry. A built-in we haven't written throws `not implemented: @name` and
   is logged. Write the non-geometric built-ins (maths, strings, maps, matrices) as std calls them. *Check:*
   `common.fs` loads, and units, vector and transform arithmetic give the expected values. Running the batch-001
   case functions prints the exact list of geometric built-ins still missing. Record the load time.
5. **Write the OpenCascade layer and the geometric built-ins.**
   - A crate `occt-sys` holds `cxx` wrappers that link the system OCCT 7.9.2. It covers:
     - points, curves and surfaces, plus the builders for edges, wires and faces;
     - prisms;
     - booleans with their history (`Generated`, `Modified`, `IsDeleted`);
     - topology exploration, mass properties and bounding boxes;
     - STEP reading and writing.
   - On top of that, write the built-ins that step 4's list asks for. These are expected to include:
     - the sketch entities, and `skSolve` (loops, holes and region splitting);
     - `opExtrude`, `opBoolean` and `opDeleteBodies`;
     - `evaluateQuery`, for the query types the extrude path uses;
     - the `ev…` measurements std itself calls.
   - Add entity identity and "created by" tracking, carried through each operation's history.

   *Check:* every batch-001 case runs to the end locally, or fails where Onshape's did.
6. **Compare against the fixtures and close the gaps.**
   - A comparison tool reads the fixture STEP with OpenCascade, splits it into cases by part name and moves each
     case back to the origin. It then compares each case with the model our engine built:
     - solid count;
     - volume, area and bounding box;
     - face counts by surface type and edge counts by curve type;
     - the volume of the difference between the two shapes, which catches shapes that differ in ways the totals
       hide.
   - Tolerances: counts must match exactly; volume and area to a relative 1e-6; the difference volume must be
     close to zero. A case missing from the fixture must also fail locally.
   - Make the batch a `cargo test`, and also write our engine's model as STEP so a failing case can be opened and
     inspected.
   - Write each convention we discover in `docs/FINDINGS.md`, with how it was found.

   *Check:* the "done" condition above is met.

Steps 1–2 are slice 1. Step 3 is slice 2: it is independent of the interpreter, and it tests the fixture workflow
early, while the browser session is fresh. Steps 4, 5 and 6 are one slice each. Each slice is committed and pushed
to `main`, then reviewed before the next one starts.

## Risks and decisions

- **The browser session can expire.** Restarting Chrome dropped the Onshape login once. Chrome is now set to
  restore its session on restart. If the login still lapses, the user logs in again through the noVNC page.
- **A model file shows geometry, not how it was made.** Query results, entity ids and error codes don't appear in
  a STEP file. Bugs in them show up only when they change the geometry, for example a fillet on the wrong edges.
  Each case should be designed so that the behaviour it tests changes the shape.
- **Seam edges and face splits.** OpenCascade puts a seam edge on every cylindrical face, and Parasolid doesn't.
  STEP export may add its own seams. Both sides of the comparison are read through OpenCascade, so counts should
  line up, but the first fixture will show whether they do. Expect more kernel differences like this one; each
  gets a note in `FINDINGS.md`.
- **Part names in the STEP export.** The comparison relies on Onshape writing each part's name into the STEP file.
  Check this on the first export; if names are lost, fall back to locating cases by their grid slot.
- **Onshape's time limit for regenerating a feature.** If one feature can't build 100 cases in time, split the
  batch across several features or Part Studios, each with its own export.
- **Interpreter speed.** About 94k lines of std load on every run. Step 2 measures parsing and step 4 measures
  loading; if either is too slow, add a cache of parsed code before going further.
