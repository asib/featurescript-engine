# Onshape local engine: discussion log

Date: 2026-11-07. A conversation with Claude (Opus 5.5) about the `onshape-fsgen` project's local builder, and
about building our own FeatureScript engine that uses Onshape as a test oracle.

## Context for picking this up elsewhere

- **Repos looked at** (read only; none of their code was run):
  - `https://github.com/Lucasfrit/onshape-fsgen`, commit `eabcc02` ("Release 0.2.0")
  - `https://github.com/javawizard/onshape-std-library-mirror`, commit `a2a7b13` ("Version 2960.0", 2026-05-15)
- The clones were in a scratch folder on the old sprite and are **not** included here. To look again:

  ```bash
  git clone https://github.com/Lucasfrit/onshape-fsgen.git
  ```

  ```bash
  git clone --depth 1 https://github.com/javawizard/onshape-std-library-mirror.git
  ```

- **Open question at the end:** whether to plan a first milestone: std's own `extrude.fs` running locally,
  checked by an Onshape test feature.

---

## 1. First question

> Can you investigate this project: https://github.com/Lucasfrit/onshape-fsgen; I am particularly interested in
> the local builder, since it could potentially help to speed up design iterations if we were rebuilding the model
> locally rather than relying on onshape's servers to do that. I would not want to use the code directly, since I
> am worried about vulnerabilities, so I am just interested in how they developed a local engine that works like
> onshape's engine works

### Answer

The local builder is about 4,000 lines of Python on the open-source OpenCascade kernel. It doesn't clone Onshape's
engine: it re-creates a small part of its behaviour, and they confirmed it by comparing volumes with Onshape. It
only works because fsgen owns the model as a script, rather than reading an existing Onshape document. I read the
code (release 0.2.0) without running any of it.

#### How it's built

```
Part Studio script (FeatureScript-like, written by the AI or by hand)
  ├─ pass 1  Tracer (native/script.py): runs the script with each feature call replaced by a recorder
  │          → list of features {type, name, params}; queries kept as FeatureScript text
  ├─ pass 2a Local builder (native/local.py): replays that list as OpenCascade operations
  └─ pass 2b Push (native/build.py): the same list → Onshape's feature JSON, one API call per feature
```

**1. A small FeatureScript interpreter (`fsgen/fslite/`).** It's a hand-written lexer, parser and interpreter of
about 1,300 lines. It copies the FeatureScript rules that matter:

- arrays and maps are copied on assignment, not shared;
- units are tracked as length and angle exponents, stored in metres and radians;
- ids work like `id + "name"`;
- it has enums and custom-feature parameters, plus step and recursion limits.

`import(...)` is parsed and then ignored. All of Onshape's standard library they need (about 90 query, sketch and
modelling functions) is rewritten in Python in `stdlib.py`.

One clever detail: every number remembers the Onshape expression it came from. So `#width / 2 - #inset` gives a
number for the local build and stays a live, variable-driven expression when pushed to Onshape. One run produces
both.

**2. A model state that mimics Onshape's (`fslite/geom.py`).** This is the core of the "works like Onshape" part:

- **"Created by" tracking.** Every face, edge and vertex is tagged with the id of the feature that made it. After
  each operation, the tags are carried through OpenCascade's record of what each operation changed or deleted.
  Anything new and untagged gets the current feature's id. This is how `qCreatedBy` keeps working after booleans
  and fillets.
- **Queries are evaluated late.** A query like `qCreatedBy` or `qParallelEdges` is an object that only resolves
  when an operation uses it, as in Onshape.
- **Entity identity includes the owning body.** A sketch edge and the solid edge that reuses its shape count as
  different entities, as in Onshape. This came from a real bug they hit.
- **Onshape's id rules are enforced**, so the errors Onshape would raise also appear locally.
- **Sketches are not solved.** There's no constraint solver. Scripts give explicit coordinates. Solving a sketch
  just joins edges into closed loops, makes faces, and splits overlapping loops into separate regions like
  Onshape's sketch regions. Constraints and dimensions are only generated for the Onshape push.
- **Onshape's quirks are copied on purpose.** Examples: a union ignores `targets` unless grouping is set,
  intersections take tools only, and revolves follow Onshape's measured `angleBack` direction. Fillets that fail in
  metres are retried at millimetre scale. The sweep tolerance was tightened because the default gave up to 5%
  volume error on threads.

**3. Standard features rewritten by hand (`native/local.py`).** In Onshape, Extrude, Fillet and Pattern are
FeatureScript code built on low-level operations. fsgen doesn't run that code; it rewrites 16 features in Python.
Feature patterns use "reapply features": each extrude's shape before the boolean is kept, moved, and the boolean
applied again. Patterns of patterns work because each pattern records what it produced. Query text from the tracer
is compiled on the fly into a tiny function and cached.

#### How they made it match Onshape: measurement

This is the most reusable part.

- **Golden tests.** 11 FeatureScript files were built both locally and in Onshape (`fsgen/compare.py`). Part
  count, volume, area and bounding box were compared, and the matches were saved to `examples/compat/golden.json`
  as offline tests. The native example parts carry Onshape's volumes in `tests/test_native_local.py`. All match to
  3 decimal places; the loft is off by 0.0001%.
- **A log of measured conventions** in `docs/FINDINGS.md`: default plane axes, how `plane()` picks its x-axis, the
  revolve direction, the sign of shell thickness, boolean quirks. Each entry says how it was found.
- **Exact math checks** where Onshape wasn't asked. For example, a profile swept along a helix must have
  volume = area × centroid radius × angle.
- **Error sorting.** Errors Onshape would also raise (an empty selection, a bad parameter) stop the build.
  Failures only OpenCascade has, and features not built locally, become warnings and mark the result as partial.
  This stops the AI from "fixing" code that was correct.

#### What this means for our goal

- **It can't rebuild an existing Onshape model.** fsgen's queries are text it wrote
  (`qCreatedBy(id + "Plate", EntityType.EDGE)`), sent to Onshape as such. A real Part Studio is different. Its
  sketches need a constraint solver, and as far as I know, selections made by clicking are stored as Onshape's
  internal ids. Those can't be resolved locally. fsgen avoids this problem rather than solving it.
- **The match is narrow.** It covers 16 features, with no "up to" bounds, draft, sheet metal or surfaces.
  "Matches" means the same volume on about 8 test parts, not the same topology. Onshape runs on Parasolid, and the
  two kernels fail differently on fillets and shells.
- **Some things can't be predicted locally.** The x-axis of a sketch on a part face depends on how Onshape made
  that face. So fsgen uses its own frame and spends one API call per face sketch to measure Onshape's when pushing.
- **It's fast:** 0.1–0.4 s per part.

**Ideas worth borrowing:**

- Treat code you own as the model, and Onshape as one build target.
- Trace once, then build twice (local geometry and Onshape feature JSON) from the same feature list.
- Use the kernel's change history for "created by" queries.
- Check parameters offline against Onshape's own feature definitions (from its `/featurespecs` endpoint).
- Keep golden volume tests against Onshape, plus a log of measured conventions.
- Hash each feature and re-push only what changed.

**An alternative they didn't take:** Onshape's standard features are themselves FeatureScript, and the standard
library source is viewable. An interpreter complete enough to run the real feature code, on top of local low-level
operations and queries, would get standard-feature behaviour almost for free. It's a much bigger interpreter,
though, and you'd need to check the library's licence.

#### Security notes (from reading only)

- **Good:** AI-written scripts run in their own interpreter, not Python's `eval`, with step and recursion limits.
- **Small flaw:** when following a redirect, it sends your Onshape API keys to any address that starts with
  `https://cad.onshape.com` (`fsgen/onshape.py:123`). So a host like `cad.onshape.com.attacker.example` would also
  get them.
- **Worth knowing:** `fsgen generate` runs `claude -p` with web search and fetch on by default, or whatever command
  you set in `FSGEN_LLM_COMMAND`. It also pulls in large compiled dependencies (OpenCascade bindings, lib3mf).

---

## 2. Second question

> hmm so since the standard lib is open source, we could in theory download the entire stdlib and then write our
> own featurescript engine that takes featurescript and generates models? and we could use onshape via the api
> explorer with a logged-in session (to get unrestricted api calls) as an "oracle" i.e. to create models that we
> can use for testing our own engine; we can have as many models as we need to test all edge cases we might
> encounter

### Answer

Yes, both parts are sound. It's also a cleaner design than fsgen's: you'd run Onshape's real feature code instead
of rewriting each feature by hand. The FeatureScript side is very doable. Almost all of the effort and risk sits in
the native layer underneath it.

#### The standard library

I cloned a community mirror (`javawizard/onshape-std-library-mirror`) and measured it:

- **Licence:** MIT, copyright PTC, so you can use it if you keep the notice.
- **Size:** 265 files and about 146k lines. 101 of those files are generated enum lists, which leaves about 94k
  hand-written lines.
- **Where your engine starts:** std calls Onshape's server through `@`-prefixed built-in functions such as
  `@opExtrude(...)` and `@evaluateQuery(...)`. Those calls are exactly what you'd have to write; everything above
  them runs unchanged. std uses **266 distinct ones**:
  - 69 modelling operations (`op…`);
  - 49 measurement functions (`ev…`), including query evaluation;
  - 19 sketch functions, including `skConstraint` and `skSolve`;
  - the rest: maths, matrices, strings, attributes, variables, feature status and pattern bookkeeping.
- **Version lag:** the mirror is at version 2960, while fsgen's scripts target 3083. std contains 605 "is this
  document at version X?" checks, so your local copy must be the same version as the Onshape you test against.
  Take it from Onshape's official public "std" document.

#### Effort, part by part

1. **The interpreter: moderate.** You need the full language, not fsgen's subset:
   - user-defined types with check functions (134 of them);
   - function overloading by argument type (`qCreatedBy` alone has 4 versions);
   - operator overloading (units are themselves written in std, with 55 overloads);
   - `as` casts, `box`, lambdas, annotations, preconditions, and versioned imports.

   Speed matters: a simple Python interpreter re-reading 94k lines on every rebuild will be slow. Cache the parsed
   code, or use a faster language.
2. **Query evaluation and face/edge naming: the hardest part.** std builds queries as plain maps
   (`{queryType: CREATED_BY, featureId: …}`) and hands them to the native layer. There are 77 query types. Many
   depend on what each operation did: which faces are an extrude's end caps, which came from a split, historical
   queries. fsgen's "created by" tagging is a starting point, but you'd need much more of it.
3. **The sketch solver.** std builds constraints and calls the native `skSolve`. As far as I know, Onshape uses
   Siemens' D-Cubed solver. The realistic open option is FreeCAD's planegcs (LGPL). Sketch code stores starting
   positions for the geometry (`skSetInitialGuess`), so the solver should usually land on the same solution.
4. **OpenCascade vs Parasolid.** Fillets and shells fail in different cases. The two kernels also build topology
   differently: OpenCascade adds a seam edge to cylinders, and Parasolid doesn't need one. That changes edge
   counts, so a query can return different results and std may take a different branch.
5. **Feature machinery.** Sub-feature transactions (`startFeature` / `abortFeature`), feature patterns (std re-runs
   the patterned feature for each instance), attributes, and error and status reporting.

A practical way in: almost every file imports `common.fs`, which re-exports nearly everything, so even one feature
loads nearly all of std. Make every built-in throw "not implemented" at first, then write only the ones your tests
actually hit.

#### Using Onshape as the test oracle

**The free Explorer calls:** this is officially true. Onshape's
[API limits page](https://onshape-public.github.io/docs/auth/limits) says Explorer calls made with an Onshape
session don't count. The same page says they *do* count when the Explorer uses API keys or OAuth. Two caveats:

- **Driving it from scripts is a grey area.** Using the Explorer by hand is clearly allowed. Running scripts on a
  copied session cookie is not what the Explorer is for, and that page doesn't address it. I'd ask
  api-support@onshape.com before building on it.
- **Short-term rate limits still apply.** fsgen's author got one endpoint locked for about 23 hours.

**You probably won't need it if you batch test cases.** std's features are ordinary functions, so a custom test
feature can call `extrude(context, id + "case17" + "f1", {...})` directly. One feature in a Feature Studio can:

- run hundreds of test cases, each in its own id namespace;
- catch each case's error so one failure doesn't stop the rest;
- measure each result: part count, volume, area, face and edge counts by surface type, per-face area and centroid,
  query result counts, and the error code;
- delete each case's bodies before the next case;
- store all results in a variable, which one `evalFeatureScript` call then returns as JSON.

Pasting code into a Feature Studio in the browser is free, so each batch costs about 1–3 calls. Even the
2,500-call yearly allowance then covers tens of thousands of cases. For exact geometry checks, also export a STEP
of the whole batch, with cases spaced apart, and compare shapes locally by the volume of their difference.
Generate cases at random where you can: shapes near the point where a fillet or shell fails, tangent and
coincident faces, overlapping patterns, cuts that remove nothing.

**One gap:** this tests feature code that uses written-out queries, which is exactly "FeatureScript in, model
out". It doesn't cover selections made by clicking in the Onshape UI. Those are stored as Onshape's internal ids,
which a local engine can't resolve. That only matters if you later want to rebuild Part Studios made in the UI.

#### Sources

- [Onshape API limits](https://onshape-public.github.io/docs/auth/limits)
- [FeatureScript guide (std is open source, public "std" document)](https://cad.onshape.com/FsDoc)
- [Forum thread on the GitHub std mirror](https://forum.onshape.com/discussion/23623/github-standard-library-mirror)
- [MCADCafe (2016) on the MIT licence](https://www10.mcadcafe.com/blogs/jeffrowe/2016/08/04/is-featurescript-one-of-onshapes-best-features)

---

## 3. Next step (not started)

Proposed first milestone, not yet planned in detail: get std's own `extrude.fs` running unmodified on a local
interpreter. Write only the `@` built-ins it needs on OpenCascade, and check the results against an Onshape test
feature that runs about 100 sketch-and-extrude cases in one batch.

---

## 4. Decisions in the follow-up session (2026-11-07)

- The project lives in a private GitHub repo. Commit directly to `main` until told to switch to pull requests.
- **Rust** for the engine, because it can be optimised further than Python. OpenCascade has usable Rust bindings:
  - [`bschwind/opencascade-rs`](https://github.com/bschwind/opencascade-rs) (crates `opencascade-sys` and
    `opencascade` 0.3.0, LGPL-2.1, 272 stars, last commit 2026-11-03). Hand-written `cxx` wrappers over about 40
    OCCT packages, including booleans, fillets, offsets, `BRepFeat`, topology exploration and mass properties. It
    can build OCCT from a bundled source tree or link an installed one.
  - [`lzpel/cadrum`](https://github.com/lzpel/cadrum) (0.9.1, October 2026): minimal bindings for OCCT 7.9.3.
  - [`occt-rs/occt-rs`](https://github.com/occt-rs/occt-rs): AGPL, young, 4 stars.

  Ubuntu 26.04 packages OCCT 7.9.2 (`libocct-*-dev`), so OCCT doesn't need to be compiled. The engine needs OCCT
  APIs none of these crates cover fully, such as the full operation history (`Generated`, `Modified`, `IsDeleted`)
  for "created by" tracking, so it will have its own thin `cxx` layer, with `opencascade-sys` as the reference.
- **Keep API Explorer calls to a minimum** so as not to load Onshape's API: put as much as possible into one call.
- The oracle uses a Chrome running on the dev sprite, which the user logs in to through a browser-based remote
  desktop (noVNC).
