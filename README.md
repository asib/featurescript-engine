# featurescript-engine

A local FeatureScript engine. The aim is to run Onshape's own standard library (`std`) unchanged on a local
interpreter, with the `@` built-ins it calls (modelling operations, queries, measurements, sketch solving)
implemented on OpenCascade. Part Studio code could then be rebuilt locally in a fraction of the time an Onshape
regeneration takes. Onshape itself is the test oracle.

Status: just started; there is no code yet. The background and reasoning are in
[docs/discussion-log.md](docs/discussion-log.md); the current plan is
[docs/milestone-1-plan.md](docs/milestone-1-plan.md).

## Decisions so far

- **Language:** Rust, for both the interpreter and the native layer.
- **Kernel:** OpenCascade (OCCT) 7.9, linked from the system package rather than built from source. The bindings
  are thin `cxx` wrappers; [`opencascade-sys`](https://github.com/bschwind/opencascade-rs) is the reference for how
  to write them.
- **std:** MIT licence, copyright PTC. Keep the notice with any copy in this repo. The local copy must be the same
  version as the Onshape it is tested against, because std branches on its version in hundreds of places.
- **Oracle:** Onshape, used from a Chrome on the dev machine that the user has logged in to.

## Rules for using Onshape as the oracle

- Never create Onshape API keys and never call Onshape's REST API directly. The only API use allowed is the API
  Explorer page in the logged-in browser.
- Keep API Explorer calls to a minimum; we do not want to load Onshape's API. Prefer the plain UI wherever it
  works; std itself is copied by reading each Feature Studio tab in the UI.
- Put every test case of a run into one test feature in a Feature Studio. Each case gets its own id namespace,
  catches its own errors and records its measurements. All results come back in one read: from the page if
  possible, otherwise from a single `evalFeatureScript` call in the API Explorer.
- Editing Feature Studios and Part Studios in the browser is not an API call; prefer it where it does the job.
- Cache every oracle result in the repo, so a case is never asked twice.

## Milestone 1

Run std's own `extrude.fs` unmodified on the local interpreter, writing only the `@` built-ins the extrude path
needs, and check about 100 sketch-and-extrude cases against one Onshape batch.
