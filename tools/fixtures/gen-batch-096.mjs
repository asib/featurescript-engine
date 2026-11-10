// Generates fixture batch 096: Part Studio tabs imported into the Feature Studio (fixture-parts and its Y-up and
// composite twins, Onshape's imports of fixtures/blobs/fixture-parts.step): one's build function run, the parts
// of two derived (one's in a composite) and the third's mirrored with derivedMirror, which Onshape refused with a
// build function from the Feature Studio (batch 060 c001; a first build mirrored the composite's, which failed,
// IMPORT_DERIVED_NO_PARTS). Each source once: a Part Studio won't derive one twice.
// Nothing reads the parts' names from the batch's own context (Onshape refuses that while a feature runs).
//
// Usage: node tools/fixtures/gen-batch-096.mjs
//
// Writes fixtures/batch-096/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, uploadImport } from './lib.mjs';

const helpers = `
// The names of the bodies a query finds (in a context no feature is running in).
function bodyNames(context is Context, bodies is Query) returns array
{
    return mapArray(evaluateQuery(context, bodies), function(b) { return getProperty(context, { "entity" : b, "propertyType" : PropertyType.NAME }); });
}

// The parts a query finds in a Part Studio, to derive.
function partsOf(build is function, parts is Query) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : parts } as PartStudioData;
}
`;

const batch = new Batch({
  number: 96,
  title: 'Part Studio tabs imported: built, derived and mirrored',
  helpers,
  imports: [
    uploadImport('Fp', 'fixture-parts.studio.fs'),
    uploadImport('Fy', 'fixture-parts-y-up.studio.fs'),
    uploadImport('Fc', 'fixture-parts-composite.studio.fs'),
  ],
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const solids = 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)';
const derived = (parts) =>
  `importDerived(context, id + "derived1", { "newUI" : true, "partStudio" : ${parts}, "preserveActiveSheetMetal" : false, "location" : qNothing(), "placement" : DerivedPlacementType.AT_ORIGIN, "includeMateConnectors" : false, "includeProperties" : true });`;
const counts = (name) => returns({
  solids: `evaluateQueryCount(context, qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID))`,
  composites: `evaluateQueryCount(context, qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.COMPOSITE))`,
});

batch.add('studio', 'fixture-parts built: its solids\' names and volumes', [
  `const other = Fp::build({});`,
  returns({
    names: `bodyNames(other, ${solids})`,
    volumes: `mapArray(evaluateQuery(other, ${solids}), function(b) { return roundToPrecision(evVolume(other, { "entities" : b }) / millimeter ^ 3, 3); })`,
  }),
]);
batch.add('derived', 'every solid of fixture-parts derived at the origin', [derived(`partsOf(Fp::build, ${solids})`), counts('derived1')]);
batch.add('derived', 'every solid of fixture-parts (2) (its parts in a composite) derived', [derived(`partsOf(Fc::build, ${solids})`), counts('derived1')]);
batch.add('mirror', 'the solids of fixture-parts (1) (Y up) mirrored with derivedMirror', [
  `derivedMirror(context, id + "mirror1", { "partStudio" : partsOf(Fy::build, ${solids}) });`,
  counts('mirror1'),
]);

batch.add('studio', 'fixture-parts (2) built: its composites, open and closed, its solids and their names', [
  `const other = Fc::build({});`,
  `const composites = qBodyType(qEverything(EntityType.BODY), BodyType.COMPOSITE);`,
  returns({
    open: `evaluateQueryCount(other, qCompositePartTypeFilter(composites, CompositePartType.OPEN))`,
    closed: `evaluateQueryCount(other, qCompositePartTypeFilter(composites, CompositePartType.CLOSED))`,
    composites: `bodyNames(other, composites)`,
    solids: `bodyNames(other, ${solids})`,
  }),
]);

batch.write('tools/fixtures/gen-batch-096.mjs');
