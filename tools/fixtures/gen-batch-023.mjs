// Generates fixture batch 023: derived sheet metal parts. The sources are Part Studios given as build functions here,
// as another Part Studio would be: a bracket (a plate with a flange) and two separate plates. They're derived with
// their sheet metal models kept, at the origin, at a mate connector (so `sheetMetalTransform` moves the model), at
// the source's own mate connector, then edited with a flange, and once as plain parts.
//
// Usage: node tools/fixtures/gen-batch-023.mjs
//
// Writes fixtures/batch-023/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, createdBy, deg, mm, vec3 } from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

// A sheet metal plate from (x0, y0) to (x1, y1) on the xy plane, 2 mm thick with 1 mm bends, made by feature "name"
// from sketch "<name>Sketch".
function smPlate(context is Context, name is string, x0 is number, y0 is number, x1 is number, y1 is number)
{
    const sketch = newSketchOnPlane(context, makeId(name ~ "Sketch"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(sketch, "rect1", { "firstCorner" : vector(x0, y0) * millimeter, "secondCorner" : vector(x1, y1) * millimeter });
    skSolve(sketch);
    sheetMetalStart(context, makeId(name), { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(makeId(name ~ "Sketch")),
                                   "thickness" : 2 * millimeter, "oppositeDirection" : false, "radius" : 1 * millimeter,
                                   "flipDirectionUp" : false, "tangentPropagation" : false, "bends" : qNothing() });
}

// The bracket: a 40 by 30 plate with a 15 mm flange up from its x = 40 edge, and a mate connector on the plate's
// top at (10, 15), its x axis along y.
const bracketStudio = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    smPlate(context, "sm1", 0, 0, 40, 30);
    const part = qBodyType(qCreatedBy(makeId("sm1"), EntityType.BODY), BodyType.SOLID);
    sheetMetalFlange(context, makeId("flange1"), {
            "edges" : qClosestTo(qOwnedByBody(part, EntityType.EDGE), vector(40, 15, 2) * millimeter),
            "flangeAlignment" : SMFlangeAlignment.INNER, "limitType" : SMFlangeBoundingType.BLIND, "distance" : 15 * millimeter,
            "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE, "oppositeDirection" : false, "bendAngle" : 90 * degree,
            "autoMiter" : true, "useDefaultRadius" : true });
    opMateConnector(context, makeId("mc"), { "coordSystem" : coordSystem(vector(10, 15, 2) * millimeter, vector(0, 1, 0), vector(0, 0, 1)),
                                             "owner" : part });
    return context;
};

// Two plates, separate sheet metal models.
const twoPlatesStudio = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    smPlate(context, "smA", 0, 0, 20, 20);
    smPlate(context, "smB", 30, 0, 60, 10);
    return context;
};

// Onshape won't derive one source twice in a Part Studio, so each case has its own (wrapping) source.
const bracketA = function(configuration is map) returns Context { return bracketStudio(configuration); };
const bracketB = function(configuration is map) returns Context { return bracketStudio(configuration); };
const bracketC = function(configuration is map) returns Context { return bracketStudio(configuration); };
const bracketD = function(configuration is map) returns Context { return bracketStudio(configuration); };
const bracketE = function(configuration is map) returns Context { return bracketStudio(configuration); };

function studio(build is function, parts is Query) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : parts } as PartStudioData;
}
`;

const batch = new Batch({ number: 23, title: 'derived sheet metal parts', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const counts = returns({ counts: 'bodyCounts(context)' });
const solidsQ = 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)';
const derived = (name, build, parts, fields = {}) =>
  call('importDerived', name, {
    newUI: 'true', partStudio: `studio(${build}, ${parts})`, preserveActiveSheetMetal: 'true', location: 'qNothing()',
    placement: 'DerivedPlacementType.AT_ORIGIN', includeMateConnectors: 'false', includeProperties: 'true', ...fields,
  });
const connector = (name, origin, x, z) =>
  call('opMateConnector', name, { coordSystem: `coordSystem(${vec3(...origin)}, vector(${x.join(', ')}), vector(${z.join(', ')}))` });

batch.add('derived', 'a sheet metal bracket derived at the origin, its model kept', [derived('derived1', 'bracketA', solidsQ), counts]);
batch.add('derived', 'the bracket derived at a mate connector, turned a quarter about z', [
  connector('m1', [60, 10, 5], [0, 1, 0], [0, 0, 1]),
  derived('derived1', 'bracketB', solidsQ, { location: createdBy('m1') }), counts,
]);
batch.add('derived', 'the bracket derived at a tilted mate connector, then a flange added to its far edge', [
  connector('m1', [0, 0, 10], [1, 0, 0], [0, -1, 1]),
  derived('derived1', 'bracketC', solidsQ, { location: createdBy('m1') }),
  call('sheetMetalFlange', 'flange2', {
    edges: `qClosestTo(qOwnedByBody(qBodyType(qCreatedBy(id + "derived1", EntityType.BODY), BodyType.SOLID), EntityType.EDGE), ${vec3(0, 9.1923882, 22.0208153)})`,
    flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(10),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true',
  }),
  counts,
]);
batch.add('derived', 'the bracket derived with its own mate connector at the origin', [
  derived('derived1', 'bracketD', solidsQ, {
    placement: 'DerivedPlacementType.AT_MATE_CONNECTOR', mateConnectorIndex: '0', mateConnectorId: 'makeId("mc")',
    mateConnectorIndexInFeature: '0',
  }),
  counts,
]);
batch.add('derived', 'the bracket derived at a mate connector as plain parts', [
  connector('m1', [60, 10, 5], [0, 1, 0], [0, 0, 1]),
  derived('derived1', 'bracketE', solidsQ, { location: createdBy('m1'), preserveActiveSheetMetal: 'false' }), counts,
]);
batch.add('derived', 'one of two sheet metal plates derived', [
  derived('derived1', 'twoPlatesStudio', `qBodyType(qCreatedBy(makeId("smB"), EntityType.BODY), BodyType.SOLID)`), counts,
]);

batch.write('tools/fixtures/gen-batch-023.mjs');
