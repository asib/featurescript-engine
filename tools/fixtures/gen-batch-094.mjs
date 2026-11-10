// Generates fixture batch 094: uploaded files (fixtures/blobs/README.md), which no batch has used: what `BLOB_DATA`
// holds for an image and a STEP file; `skImage` placing the image in boxes of its own shape, wider, and with the
// corners swapped; `importForeign` bringing in the STEP file as it is, Y axis up, as a composite, with its units
// overridden, and `opImportForeign` with no more than the file.
//
// Usage: node tools/fixtures/gen-batch-094.mjs
//
// Writes fixtures/batch-094/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, vec2, PLANES, uploadImport } from './lib.mjs';

const helpers = `
// The names of the bodies a query finds.
function bodyNames(context is Context, bodies is Query) returns array
{
    return mapArray(evaluateQuery(context, bodies), function(b) { return getProperty(context, { "entity" : b, "propertyType" : PropertyType.NAME }); });
}

// How many bodies of each type a query finds.
function bodyTypes(context is Context, bodies is Query) returns map
{
    var out = {};
    for (var bodyType in [BodyType.SOLID, BodyType.SHEET, BodyType.WIRE, BodyType.POINT, BodyType.COMPOSITE])
    {
        out[toString(bodyType)] = evaluateQueryCount(context, qBodyType(bodies, bodyType));
    }
    return out;
}

// The points of some vertices (mm, rounded), sorted by x and then y.
function pointList(context is Context, vertices is Query) returns array
{
    const points = mapArray(evaluateQuery(context, vertices), function(v) {
            const p = evVertexPoint(context, { "vertex" : v }) / millimeter;
            return [roundToPrecision(p[0], 6), roundToPrecision(p[1], 6), roundToPrecision(p[2], 6)];
        });
    return sort(points, function(a, b) { return a[0] != b[0] ? a[0] - b[0] : a[1] - b[1]; });
}

// What a sketch made: its bodies by type, their vertices' points and how many edges they have.
function sketchProbe(context is Context, sketchId is Id) returns map
{
    const bodies = qCreatedBy(sketchId, EntityType.BODY);
    return { "bodies" : bodyTypes(context, bodies),
             "points" : pointList(context, qOwnedByBody(bodies, EntityType.VERTEX)),
             "edges" : evaluateQueryCount(context, qOwnedByBody(bodies, EntityType.EDGE)) };
}
`;

const batch = new Batch({
  number: 94,
  title: 'uploaded files: an image in a sketch, a STEP file imported',
  helpers,
  imports: [uploadImport('Image', 'fixture-image.png'), uploadImport('Parts', 'fixture-parts.step')],
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;

// ----- what the uploads are -----

batch.add('blob', 'the BLOB_DATA of the image and of the STEP file', [returns({ image: 'Image::BLOB_DATA', parts: 'Parts::BLOB_DATA' })]);

// ----- the image in a sketch (8 × 4 pixels) -----

const image = (first, second) => [
  `const s1 = newSketchOnPlane(context, id + "s1", { "sketchPlane" : ${PLANES.xy} });`,
  `const placed = skImage(s1, "image1", { "blobInfo" : Image::BLOB_DATA, "firstCorner" : ${vec2(...first)}, "secondCorner" : ${vec2(...second)} });`,
  `skSolve(s1);`,
  returns({ placed: 'placed', sketch: 'sketchProbe(context, id + "s1")' }),
];
batch.add('sketch', 'the image in a 40 × 20 box from the origin, its own shape', [image([0, 0], [40, 20])]);
batch.add('sketch', 'the image in a 40 × 10 box, wider than it', [image([0, 0], [40, 10])]);
batch.add('sketch', 'the image in the 40 × 20 box, its corners given the other way round', [image([40, 20], [0, 0])]);

// ----- the STEP file imported -----

const imported = (fields) => [
  `importForeign(context, id + "import1", { "dependsOnBlob" : true, "blobData" : Parts::BLOB_DATA${fields} });`,
  returns({ names: 'bodyNames(context, qCreatedBy(id + "import1", EntityType.BODY))', types: 'bodyTypes(context, qCreatedBy(id + "import1", EntityType.BODY))' }),
];
batch.add('import', 'the STEP file (a block "Block" and a ball "Ball") imported as it is', [imported('')]);
batch.add('import', 'the same with its Y axis up', [imported(', "yAxisIsUp" : true')]);
batch.add('import', 'the same as a composite part named "Imported"', [imported(', "createComposite" : true, "compositeName" : "Imported"')]);
batch.add('import', 'the same with its units given as inches, from millimetres', [
  imported(', "specifyUnits" : true, "unit" : LengthUnitNames.Inch, "originalUnit" : LengthUnitNames.Millimeter'),
]);
batch.add('import', 'opImportForeign given only the file', [
  `opImportForeign(context, id + "import1", { "blobData" : Parts::BLOB_DATA });`,
  returns({ names: 'bodyNames(context, qCreatedBy(id + "import1", EntityType.BODY))', types: 'bodyTypes(context, qCreatedBy(id + "import1", EntityType.BODY))' }),
]);

batch.write('tools/fixtures/gen-batch-094.mjs');
