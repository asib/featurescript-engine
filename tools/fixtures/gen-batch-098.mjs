// Generates fixture batch 098: why Onshape fails derivedMirror of a build function from the Feature Studio ("Execution
// error", batch 060 c001) but not of a Part Studio tab's (batch 096 c004), nor importDerived building one twice
// (batch 045): build functions instantiated with no build before, after building one directly, two instances of
// one, importDerived of one, and derivedMirror of one making a box by a feature and of one making a ball by an
// operation. Each case has its own build function (a Part Studio won't derive one source twice).
//
// Usage: node tools/fixtures/gen-batch-098.mjs
//
// Writes fixtures/batch-098/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch } from './lib.mjs';

const studios = ['A', 'B', 'C', 'D', 'F']
  .map((name, i) => `
const studio${name} = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    fCuboid(context, makeId("box"), { "corner1" : vector(0, 0, 5) * millimeter, "corner2" : vector(${20 + i}, 10, 15) * millimeter });
    return context;
};`)
  .join('\n');

const helpers = `${studios}

const studioE = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    opSphere(context, makeId("ball"), { "center" : vector(10, 5, 10) * millimeter, "radius" : 4 * millimeter });
    return context;
};

// A build function's solids, to derive.
function partsOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qBodyType(qEverything(EntityType.BODY), BodyType.SOLID) } as PartStudioData;
}

// How many solids an id made.
function solidCount(context is Context, id is Id) returns number
{
    return evaluateQueryCount(context, qBodyType(qCreatedBy(id, EntityType.BODY), BodyType.SOLID));
}
`;

const batch = new Batch({ number: 98, title: 'build functions from the Feature Studio built, instantiated, derived and mirrored', helpers });

const instantiate = (build, count = 1) => [
  `const instantiator = newInstantiator(id + "inst");`,
  ...Array.from({ length: count }, () => `addInstance(instantiator, partsOf(${build}), {});`),
  `instantiate(context, instantiator);`,
  `return { "solids" : solidCount(context, id + "inst") };`,
];

batch.add('instantiate', 'a box\'s build function instantiated, not built before', [instantiate('studioA')]);
batch.add('instantiate', 'a box\'s build function built directly, then instantiated', [`const other = studioB({});`, instantiate('studioB')]);
batch.add('derived', 'a box\'s build function derived with importDerived', [
  `importDerived(context, id + "derived1", { "newUI" : true, "partStudio" : partsOf(studioC), "preserveActiveSheetMetal" : false, "location" : qNothing(), "placement" : DerivedPlacementType.AT_ORIGIN, "includeMateConnectors" : false, "includeProperties" : true });`,
  `return { "solids" : solidCount(context, id + "derived1") };`,
]);
batch.add('mirror', 'a box\'s build function (fCuboid, a feature) mirrored with derivedMirror', [
  `derivedMirror(context, id + "mirror1", { "partStudio" : partsOf(studioD) });`,
  `return { "solids" : solidCount(context, id + "mirror1") };`,
]);
batch.add('mirror', 'a ball\'s build function (opSphere, an operation) mirrored with derivedMirror', [
  `derivedMirror(context, id + "mirror1", { "partStudio" : partsOf(studioE) });`,
  `return { "solids" : solidCount(context, id + "mirror1") };`,
]);
batch.add('instantiate', 'a box\'s build function instantiated twice in one instantiator', [instantiate('studioF', 2)]);

batch.write('tools/fixtures/gen-batch-098.mjs');
