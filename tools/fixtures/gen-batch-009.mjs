// Generates fixture batch 009: derived parts (from build functions in the cases, as another Part Studio would be),
// section parts, and feature patterns and mirrors. (`derivedMirror` fails in a Part Studio feature without a message,
// so it isn't here.)
//
// Usage: node tools/fixtures/gen-batch-009.mjs
//
// Writes fixtures/batch-009/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, createdBy, cylinder, deg, edgeOf, faceAt, mm, planeAt, planeBody, solidsOf, vec2, vec3 } from './lib.mjs';

const helpers = `
// Part Studios to derive from: each builds its own context. They're lambdas, since a named function can't be used
// as a value.
const blockStudio = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    fCuboid(context, makeId("block"), { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(20, 10, 5) * millimeter });
    return context;
};

const twoPartStudio = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    fCuboid(context, makeId("block"), { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(20, 10, 5) * millimeter });
    fCylinder(context, makeId("pin"), { "bottomCenter" : vector(30, 5, 0) * millimeter, "topCenter" : vector(30, 5, 12) * millimeter, "radius" : 3 * millimeter });
    opMateConnector(context, makeId("mate"), { "coordSystem" : coordSystem(vector(10, 5, 5) * millimeter, vector(1, 0, 0), vector(0, 0, 1)), "owner" : qCreatedBy(makeId("block"), EntityType.BODY) });
    return context;
};

// Onshape won't derive one source twice in a Part Studio, so the later cases get their own (wrapping) sources.
const twoPartStudioB = function(configuration is map) returns Context { return twoPartStudio(configuration); };

function studio(build is function, parts is Query) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : parts } as PartStudioData;
}
`;

const batch = new Batch({ number: 9, title: 'derived parts, section parts, and feature patterns', helpers });

const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const base = solidsOf('base');
const derived = (name, build, parts, fields = {}) =>
  call('importDerived', name, {
    newUI: 'true', partStudio: `studio(${build}, ${parts})`, preserveActiveSheetMetal: 'false', location: 'qNothing()',
    placement: 'DerivedPlacementType.AT_ORIGIN', includeMateConnectors: 'false', includeProperties: 'true', ...fields,
  });
const solidsQ = 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)';

// ----- derived parts -----

batch.add('derived', 'a block derived at the origin', [derived('derived1', 'blockStudio', solidsQ)]);
batch.add('derived', 'two parts derived at two mate connectors', [
  call('opMateConnector', 'm1', { coordSystem: `coordSystem(${vec3(0, 0, 0)}, vector(1, 0, 0), vector(0, 0, 1))` }),
  call('opMateConnector', 'm2', { coordSystem: `coordSystem(${vec3(50, 20, 10)}, vector(0, 1, 0), vector(1, 0, 0))` }),
  derived('derived1', 'twoPartStudio', solidsQ, { location: union(createdBy('m1'), createdBy('m2')) }),
]);
batch.add('derived', 'the block alone, with its mate connector', [
  derived('derived1', 'twoPartStudioB', `qCreatedBy(makeId("block"), EntityType.BODY)`, { includeMateConnectors: 'true' }),
]);

// ----- section parts (used by drawings) -----

const sectionTarget = [box('base', [0, 0, 0], [40, 30, 20]), cylinder('pin', [50, 15], 5, 0, 30)];
batch.add('section', 'section part: everything behind a plane through the block', [
  sectionTarget,
  call('sectionPart', 'section1', { targets: union(base, solidsOf('pin')), plane: `plane(${vec3(20, 15, 10)}, vector(1, 0, 0))` }),
]);
batch.add('section', 'plane section part of the block and pin', [
  sectionTarget,
  call('planeSectionPart', 'section1', { target: union(base, solidsOf('pin')), plane: `plane(${vec3(0, 15, 0)}, vector(0, 1, 0), vector(1, 0, 0))` }),
]);

// ----- feature patterns and mirrors -----

const boss = (name) => [
  `const ${name}Sketch = newSketchOnPlane(context, id + "${name}Sketch", { "sketchPlane" : plane(${vec3(0, 0, 12)}, vector(0, 0, 1), vector(1, 0, 0)) });`,
  `skCircle(${name}Sketch, "circle1", { "center" : ${vec2(8, 8)}, "radius" : ${mm(3)} });`,
  `skSolve(${name}Sketch);`,
];
const bossFeature = (name) =>
  `extrude(context, ${name}, { "entities" : qSketchRegion(id + "bossSketch"), "endBound" : BoundingType.BLIND, "depth" : ${mm(6)}, "operationType" : NewBodyOperationType.ADD, "defaultScope" : false, "booleanScope" : ${base} });`;
const featureList = `{ (id + "boss") : function(instanceId) { ${bossFeature('instanceId')} } } as FeatureList`;
const withBoss = [box('base', [0, 0, 0], [40, 30, 12]), boss('boss'), bossFeature('id + "boss"')];
batch.add('feature-pattern', 'an extruded boss patterned along a block', [
  withBoss,
  call('linearPattern', 'pattern1', {
    patternType: 'PatternType.FEATURE', instanceFunction: featureList, directionOne: `qContainsPoint(qOwnedByBody(${base}, EntityType.EDGE), ${vec3(20, 0, 0)})`,
    distance: mm(10), instanceCount: '3', fullFeaturePattern: 'false',
  }),
]);
batch.add('feature-pattern', 'an extruded boss patterned round the block\'s middle', [
  withBoss, axisLine('axis1', [20, 15, 0], [20, 15, 30]),
  call('circularPattern', 'pattern1', {
    patternType: 'PatternType.FEATURE', instanceFunction: featureList, axis: edgeOf('axis1'), angle: deg(360), instanceCount: '4',
    equalSpace: 'true', fullFeaturePattern: 'false',
  }),
]);
batch.add('feature-pattern', 'an extruded boss mirrored across the block\'s middle', [
  withBoss, planeBody('mid', [20, 15, 6], [1, 0, 0], [0, 1, 0]),
  call('mirror', 'mirror1', { patternType: 'MirrorType.FEATURE', instanceFunction: featureList, mirrorPlane: createdBy('mid', 'FACE'), fullFeaturePattern: 'false' }),
]);

batch.write('tools/fixtures/gen-batch-009.mjs');
