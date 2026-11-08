// Generates fixture batch 022: sheet metal forms. Form tools are Part Studios given as build functions here: a boss
// (a positive part only) and a cup (positive and negative parts), each with its coordinate system's mate connector,
// placed on a plate at sketch points, one flipped.
//
// Usage: node tools/fixtures/gen-batch-022.mjs
//
// Writes fixtures/batch-022/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm, planeAt, rect, sketch, solve, vec2 } from './lib.mjs';

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

const batch = new Batch({ number: 22, title: 'sheet metal forms', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const counts = returns({ counts: 'bodyCounts(context)' });
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
const points = (s, z, list) => [
  sketch(s, planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])),
  list.map(([x, y], i) => `skPoint(${s}, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve(s),
];
const pointsOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
const top = `qClosestTo(qOwnedByBody(${part}, EntityType.FACE), vector(20, 15, 2) * millimeter)`;
const form = (tool, fields = {}) =>
  call('sheetMetalFormed', 'form1', { formPartStudio: `formOf(${tool})`, locations: pointsOf('pts'), flipDirection: 'false', targetFaces: top, ...fields });

batch.add('sheet-metal-formed', 'a boss formed on a plate', [plate, points('pts', 2, [[20, 15]]), form('bossTool'), counts]);
batch.add('sheet-metal-formed', 'cups formed at two places on a plate', [plate, points('pts', 2, [[12, 15], [28, 15]]), form('cupTool'), counts]);
batch.add('sheet-metal-formed', 'a cup formed on a plate, flipped', [
  plate, points('pts', 2, [[20, 15]]), form('cupTool', { flipDirection: 'true' }), counts,
]);

batch.write('tools/fixtures/gen-batch-022.mjs');
