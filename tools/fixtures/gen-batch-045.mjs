// Generates fixture batch 045: Part Studios with composite parts and sheet metal derived (`importDerived`): an open
// composite of a sheet metal plate and a block, a closed one of the same, and a closed composite of two plain
// blocks beside a sheet metal plate. The sources are given as build functions here, as in batch 023.
//
// Usage: node tools/fixtures/gen-batch-045.mjs
//
// Writes fixtures/batch-045/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call } from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)),
             "composites" : evaluateQueryCount(context, qBodyType(all, BodyType.COMPOSITE)) };
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

// A 10 mm block from (x0, y0, 0) to (x1, y1, 10), made by feature "name".
function block(context is Context, name is string, x0 is number, y0 is number, x1 is number, y1 is number)
{
    fCuboid(context, makeId(name), { "corner1" : vector(x0, y0, 0) * millimeter, "corner2" : vector(x1, y1, 10) * millimeter });
}

// A sheet metal plate and a block, made a composite part (closed or open).
function plateAndBlock(closed is boolean) returns Context
{
    const context = newContextWithDefaults();
    smPlate(context, "sm1", 0, 0, 40, 30);
    block(context, "block1", 50, 0, 60, 10);
    opCreateCompositePart(context, makeId("composite1"), { "bodies" : qBodyType(qEverything(EntityType.BODY), BodyType.SOLID), "closed" : closed });
    return context;
}

const openComposite = function(configuration is map) returns Context { return plateAndBlock(false); };
const closedComposite = function(configuration is map) returns Context { return plateAndBlock(true); };

// Two blocks made a closed composite, beside a sheet metal plate.
const blocksAndPlate = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    block(context, "block1", 50, 0, 60, 10);
    block(context, "block2", 50, 20, 60, 30);
    opCreateCompositePart(context, makeId("composite1"), { "bodies" : qUnion([qCreatedBy(makeId("block1"), EntityType.BODY), qCreatedBy(makeId("block2"), EntityType.BODY)]), "closed" : true });
    smPlate(context, "sm1", 0, 0, 40, 30);
    return context;
};

function studio(build is function, parts is Query) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : parts } as PartStudioData;
}
`;

const batch = new Batch({ number: 45, title: 'composite parts with sheet metal derived', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const counts = returns({ counts: 'bodyCounts(context)' });
const derived = (build, parts) =>
  call('importDerived', 'derived1', {
    newUI: 'true', partStudio: `studio(${build}, ${parts})`, preserveActiveSheetMetal: 'true', location: 'qNothing()',
    placement: 'DerivedPlacementType.AT_ORIGIN', includeMateConnectors: 'false', includeProperties: 'true',
  });
const composites = 'qBodyType(qEverything(EntityType.BODY), BodyType.COMPOSITE)';
const allParts = 'qUnion([qBodyType(qEverything(EntityType.BODY), BodyType.SOLID), qBodyType(qEverything(EntityType.BODY), BodyType.COMPOSITE)])';

batch.add('derived', 'an open composite of a sheet metal plate and a block derived, the plate\'s model kept', [derived('openComposite', composites), counts]);
batch.add('derived', 'a closed composite of a sheet metal plate and a block derived', [derived('closedComposite', composites), counts]);
batch.add('derived', 'a closed composite of two blocks and a separate sheet metal plate derived', [derived('blocksAndPlate', allParts), counts]);

batch.write('tools/fixtures/gen-batch-045.mjs');
