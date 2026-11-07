# featurescript-engine

A local FeatureScript engine. The aim is to run Onshape's own standard library (`std`) unchanged on a local
interpreter, with the `@` built-ins it calls (modelling operations, queries, measurements, sketch solving)
implemented on OpenCascade. Part Studio code could then be rebuilt locally in a fraction of the time an Onshape
regeneration takes. Onshape's own engine supplies the expected results, as model files.

Status: std 3112.0 is vendored in [std/](std/NOTICE.md), and the `fs-lang` crate parses all of it. Fixture batch
001 (106 sketch-and-extrude cases with Onshape's STEP) is in [fixtures/batch-001](fixtures/batch-001/README.md).
The `fs-interp` crate interprets std: all of it loads in about 120 ms. The `occt` crate binds the system's
OpenCascade, and `fs-geom` implements the geometry built-ins on it: std's own `extrude` runs all of batch 001 in
about 0.35 s, building the 100 parts Onshape built and failing the other six as Onshape did. Comparing the parts
in detail with Onshape's STEP comes next. What we've learned about Onshape is in
[docs/FINDINGS.md](docs/FINDINGS.md). `cargo test` runs the tests (OCCT 7.9's `libocct-*-dev` packages must be
installed). The background and reasoning are in
[docs/discussion-log.md](docs/discussion-log.md); the current plan is
[docs/milestone-1-plan.md](docs/milestone-1-plan.md).

## Decisions so far

- **Language:** Rust, for both the interpreter and the native layer.
- **Kernel:** OpenCascade (OCCT) 7.9, linked from the system package rather than built from source. The bindings
  are thin `cxx` wrappers; [`opencascade-sys`](https://github.com/bschwind/opencascade-rs) is the reference for how
  to write them.
- **std:** MIT licence, copyright PTC. Keep the notice with any copy in this repo. The local copy must be the same
  version as the Onshape it is tested against, because std branches on its version in hundreds of places.
- **Fixtures:** Onshape builds each test case once, and we export the model as STEP from its UI. The export is
  done through a Chrome on the dev machine that the user has logged in to. Tests compare our engine's models
  against these fixtures and never touch Onshape.

## Rules for using Onshape

- Onshape is only used to generate fixtures and to copy std. Tests never run in Onshape or call it.
- Never create Onshape API keys and never call Onshape's REST API directly. The only API use allowed is the API
  Explorer page in the logged-in browser, and so far nothing has needed it.
- Prefer the plain UI: std is copied by reading each Feature Studio tab, and fixtures are exported from the export
  menu.
- Build each batch of cases in one go: one generator feature builds every case, and one export saves them all.
- Commit every fixture, so a case is never built in Onshape twice.

## Milestone 1

Run std's own `extrude.fs` unmodified on the local interpreter, writing only the `@` built-ins the extrude path
needs. Check that about 100 sketch-and-extrude cases build the same models as Onshape's STEP fixtures.
