# Uploaded files

Files uploaded to the fixture document through Onshape's UI ("+" → "Import…"), for cases that import them into
the Feature Studio (`Namespace::import(path : "<element id>", version : "<version>")`) and use their `BLOB_DATA`:
`skImage` and `importForeign`. `uploads.txt` maps each tab's element id to its file here, the version the
Feature Studio's Import dialog gave and the BLOB_DATA importing it gives; the engine's harness resolves the imports
from it.

| file | what | tab in the document |
|---|---|---|
| fixture-image.png | an 8 × 4 RGB image: the top row green, the rest red on the left half and blue on the right | `fixture-image.png` (an image tab) |
| fixture-parts.step | a 40 × 30 × 10 mm block from the origin named "Block" and a ball of radius 8 mm at (60, 15, 8) mm named "Ball", AP242 in metres (`cargo run -p fs-fixtures --example make_blob_step`) | `fixture-parts.step` in the "CAD Imports" folder (Onshape's import also made a Part Studio `fixture-parts`) |

Uploaded on 2026-11-10 with the default import options (import to this document, appearances on), then twice
more: once with "Orient imported models with Y Axis Up" ticked, once with "Create a composite part" ticked. Each
upload made a Part Studio of the file's parts (`fixture-parts`, `fixture-parts (1)`, `fixture-parts (2)`), exported
through the UI (`node tools/browser/export-step.mjs fixtures/blobs <document> <tab> <file>`) as
fixture-parts-onshape.step, fixture-parts-y-up-onshape.step and fixture-parts-composite-onshape.step: Onshape's
importForeign, which tests/import_foreign.rs compares the engine's with. Their Import features' parameters were
read from the page's own websocket frames (the composite is named after the file, `fixture-parts`).

The Part Studios are tabs too: imported into the Feature Studio (batch 096), each is a module with a build
function. The engine stands in for each with a FeatureScript module replaying its one Import feature
(fixture-parts.studio.fs, fixture-parts-y-up.studio.fs, fixture-parts-composite.studio.fs), listed in uploads.txt.

A Part Studio drawn by hand in the UI is a tab too: `Part Studio 2` (batch 103), a sketch of splines, a Bézier, a
line, a circle, an arc and a point, and an extrude of two of its regions. ui-sketch.studio.fs is its FeatureScript
exactly as its "Show code" gives it (right-click its tab), which the engine loads for it unchanged; `Part Studio 3`
(batch 104), trimmed and used splines, is ui-sketch-2.studio.fs.
