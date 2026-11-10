// Generates fixture batch 095: how FeatureScript can import an uploaded STEP file (batch 094's importForeign with
// the file's BLOB_DATA failed, "Execution error", and opImportForeign with it INVALID_INPUT): the errors caught in
// full, importForeign and opImportForeign given the file's data id as the foreign id, and opImportForeign given the
// BLOB_DATA with every field importForeign passes.
//
// Usage: node tools/fixtures/gen-batch-095.mjs
//
// Writes fixtures/batch-095/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, uploadImport } from './lib.mjs';

const helpers = `
// The names of the bodies a query finds.
function bodyNames(context is Context, bodies is Query) returns array
{
    return mapArray(evaluateQuery(context, bodies), function(b) { return getProperty(context, { "entity" : b, "propertyType" : PropertyType.NAME }); });
}
`;

const batch = new Batch({ number: 95, title: 'importing an uploaded STEP file', helpers, imports: [uploadImport('Parts', 'fixture-parts.step')] });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const names = 'bodyNames(context, qCreatedBy(id + "import1", EntityType.BODY))';
const caught = (statement) => [
  `try { ${statement} } catch (error) { ${returns({ error: 'error', status: 'getFeatureError(context, id + "import1")' })} }`,
  returns({ names }),
];

batch.add('import', 'importForeign with the BLOB_DATA (batch 094 c005), its error caught', [
  caught(`importForeign(context, id + "import1", { "dependsOnBlob" : true, "blobData" : Parts::BLOB_DATA });`),
]);
batch.add('import', 'opImportForeign with the BLOB_DATA (batch 094 c009), its error caught', [
  caught(`opImportForeign(context, id + "import1", { "blobData" : Parts::BLOB_DATA });`),
]);
batch.add('import', 'importForeign with the data id as the foreign id', [
  caught(`importForeign(context, id + "import1", { "foreignId" : Parts::BLOB_DATA.dataId });`),
]);
batch.add('import', 'opImportForeign with the data id as the foreign id', [
  caught(`opImportForeign(context, id + "import1", { "foreignId" : Parts::BLOB_DATA.dataId });`),
]);
batch.add('import', 'opImportForeign with the BLOB_DATA and every field importForeign passes', [
  caught(
    `opImportForeign(context, id + "import1", { "blobData" : Parts::BLOB_DATA, "flatten" : false, "yAxisIsUp" : false, "isModifiable" : true, "scale" : 1.0, "importFaceColors" : true, "importMaterialDensity" : false, "allowFaultyParts" : false, "maxAssembliesToCreate" : 10, "createComposite" : false, "isInContext" : false });`,
  ),
]);

batch.write('tools/fixtures/gen-batch-095.mjs');
