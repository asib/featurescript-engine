// Generates fixture batch 107: a sheet metal plate drawn in Onshape's UI ("Part Studio 4", its Show code in
// fixtures/blobs/ui-sheet-metal.studio.fs: a rectangle on Top thickened into sheet metal, its region found by a
// zlib-compressed query), imported and built, its sheet metal mirrored with derivedMirror (std's
// sheetMetalTransform, which batch 023 reached only through importDerived), and its compressed region query decoded.
// (A first plate had a flange, whose edge query names Onshape's own history operations; left out.)
//
// Usage: node tools/fixtures/gen-batch-107.mjs
//
// Writes fixtures/batch-107/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { readFileSync } from 'node:fs';
import { Batch, uploadImport } from './lib.mjs';

const studio = readFileSync(new URL('../../fixtures/blobs/ui-sheet-metal.studio.fs', import.meta.url), 'utf8');
const region = studio.match(/qCompressed\(1\.0,"(&[^"]*)",id\)/)[1];

const helpers = `
// The solids in a context, each as its volume (mm³) and bounding box (mm), sorted.
function solids(context is Context, q is Query) returns array
{
    return sort(mapArray(evaluateQuery(context, qBodyType(q, BodyType.SOLID)), function(b) {
            const bounds = evBox3d(context, { "topology" : b, "tight" : true });
            return { "volume" : roundToPrecision(evVolume(context, { "entities" : b }) / millimeter ^ 3, 3),
                     "min" : [roundToPrecision(bounds.minCorner[0] / millimeter, 4), roundToPrecision(bounds.minCorner[1] / millimeter, 4), roundToPrecision(bounds.minCorner[2] / millimeter, 4)],
                     "max" : [roundToPrecision(bounds.maxCorner[0] / millimeter, 4), roundToPrecision(bounds.maxCorner[1] / millimeter, 4), roundToPrecision(bounds.maxCorner[2] / millimeter, 4)] };
        }), function(a, b) { return a.volume - b.volume; });
}
`;

const batch = new Batch({
  number: 107,
  title: "a sheet metal plate drawn in Onshape's UI: built, mirrored, its compressed query decoded",
  helpers,
  imports: [uploadImport('Sm', 'ui-sheet-metal.studio.fs')],
});

batch.add('studio', "the UI-drawn sheet metal plate's tab built: its solids and the sheet metal models' count", [
  'const built = Sm::build({});',
  `return { "solids" : solids(built, qEverything(EntityType.BODY)), "sheets" : size(evaluateQuery(built, qBodyType(qEverything(EntityType.BODY), BodyType.SHEET))) };`,
]);
batch.add('mirror', "the plate's solids mirrored with derivedMirror", [
  `derivedMirror(context, id + "mirror1", { "partStudio" : { "buildFunction" : Sm::build, "configuration" : {}, "partQuery" : qBodyType(qEverything(EntityType.BODY), BodyType.SOLID) } as PartStudioData });`,
  `return { "solids" : solids(context, qCreatedBy(id + "mirror1", EntityType.BODY)) };`,
]);
batch.add('compressed', "the plate's zlib-compressed region query decoded", [
  `return { "region" : qCompressed(1, "${region}", newId()) };`,
]);

batch.write('tools/fixtures/gen-batch-107.mjs');
