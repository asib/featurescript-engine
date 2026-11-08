// Generates fixture batch 033: face patterns of sheet metal forms, which carry the forms' parts along.
//
// Usage: node tools/fixtures/gen-batch-033.mjs
//
// Writes fixtures/batch-033/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, call, deg, edgeOf, mm, planeAt, rect, sketch, solve, vec2 } from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

// A form tool: a boss 5 mm in radius standing 2 mm proud of the sheet, its coordinate system on the sheet's face. The
// positive part runs down through the sheet (form tools must overlap it).
const bossTool = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const thickness = configuration.thickness == undefined ? 2 * millimeter : configuration.thickness;
    fCylinder(context, makeId("positive"), { "bottomCenter" : vector(0, 0, 0) * meter - vector(0, 0, 1) * thickness,
                                             "topCenter" : vector(0, 0, 2) * millimeter, "radius" : 5 * millimeter });
    opMateConnector(context, makeId("cSys"), { "coordSystem" : coordSystem(vector(0, 0, 0) * meter, vector(1, 0, 0), vector(0, 0, 1)) });
    tag(context, makeId("tag"), { "tagPurpose" : TagPurpose.FORM, "positivePart" : qCreatedBy(makeId("positive"), EntityType.BODY),
                                  "negativePart" : qNothing(), "flatFormSketch" : {} as FeatureList,
                                  "cSysMateConnector" : qCreatedBy(makeId("cSys"), EntityType.BODY) });
    return context;
};

// A form tool: a cup, the boss above with a 4 mm hole up into it from below the sheet.
const cupTool = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const thickness = configuration.thickness == undefined ? 2 * millimeter : configuration.thickness;
    fCylinder(context, makeId("positive"), { "bottomCenter" : vector(0, 0, 0) * meter - vector(0, 0, 1) * thickness,
                                             "topCenter" : vector(0, 0, 2) * millimeter, "radius" : 5 * millimeter });
    fCylinder(context, makeId("negative"), { "bottomCenter" : vector(0, 0, -1) * millimeter - vector(0, 0, 1) * thickness,
                                             "topCenter" : vector(0, 0, 1) * millimeter, "radius" : 4 * millimeter });
    opMateConnector(context, makeId("cSys"), { "coordSystem" : coordSystem(vector(0, 0, 0) * meter, vector(1, 0, 0), vector(0, 0, 1)) });
    tag(context, makeId("tag"), { "tagPurpose" : TagPurpose.FORM, "positivePart" : qCreatedBy(makeId("positive"), EntityType.BODY),
                                  "negativePart" : qCreatedBy(makeId("negative"), EntityType.BODY), "flatFormSketch" : {} as FeatureList,
                                  "cSysMateConnector" : qCreatedBy(makeId("cSys"), EntityType.BODY) });
    return context;
};

function formOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}
`;


const batch = new Batch({ number: 33, title: 'sheet metal form patterns', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const counts = returns({ counts: 'bodyCounts(context)' });
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [60, 40]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
const formAt = (tool, x, y) => [
  sketch('pts', planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])), `skPoint(pts, "p1", { "position" : ${vec2(x, y)} });`, solve('pts'),
  call('sheetMetalFormed', 'form1', {
    formPartStudio: `formOf(${tool})`, locations: `qOwnedByBody(qBodyType(qCreatedBy(id + "pts", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`,
    flipDirection: 'false', targetFaces: `qClosestTo(qOwnedByBody(${part}, EntityType.FACE), vector(30, 20, 2) * millimeter)`,
  }),
];
const formFaces = `qCreatedBy(id + "form1", EntityType.FACE)`;

batch.add('sheet-metal-pattern', 'a boss formed on a plate, its faces patterned 3 times, 15 mm apart', [
  plate, formAt('bossTool', 12, 20), axisLine('dirX', [0, 0, 0], [1, 0, 0]),
  call('linearPattern', 'pattern1', {
    patternType: 'PatternType.FACE', faces: formFaces, directionOne: edgeOf('dirX'), distance: mm(15), instanceCount: '3', oppositeDirection: 'false',
  }),
  counts,
]);
batch.add('sheet-metal-pattern', 'a cup formed on a plate, its faces patterned 3 times round the plate\'s middle', [
  plate, formAt('cupTool', 45, 20), axisLine('axis1', [30, 20, 0], [30, 20, 10]),
  call('circularPattern', 'pattern1', {
    patternType: 'PatternType.FACE', faces: formFaces, axis: edgeOf('axis1'), angle: deg(360), instanceCount: '3', equalSpace: 'true',
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-033.mjs');
