// Generates fixture batch 032: face patterns of hole-feature holes in a sheet metal plate, which carry the holes'
// tool bodies along.
//
// Usage: node tools/fixtures/gen-batch-032.mjs
//
// Writes fixtures/batch-032/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, call, deg, edgeOf, mm, planeAt, rect, sketch, solve, vec2 } from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({ number: 32, title: 'sheet metal hole patterns', helpers });

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
const holeAt = (x, y, fields = {}) => [
  sketch('pts1', planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])), `skPoint(pts1, "p1", { "position" : ${vec2(x, y)} });`, solve('pts1'),
  call('hole', 'hole1', {
    locations: `qOwnedByBody(qBodyType(qCreatedBy(id + "pts1", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`, scope: part,
    style: 'HoleStyle.SIMPLE', holeDiameter: mm(4), endStyle: 'HoleEndStyle.THROUGH', ...fields,
  }),
];
const holeFaces = `qCreatedBy(id + "hole1", EntityType.FACE)`;

batch.add('sheet-metal-pattern', 'a hole-feature hole\'s faces patterned 3 times, 10 mm apart', [
  plate, holeAt(8, 15), axisLine('dirX', [0, 0, 0], [1, 0, 0]),
  call('linearPattern', 'pattern1', {
    patternType: 'PatternType.FACE', faces: holeFaces, directionOne: edgeOf('dirX'), distance: mm(10), instanceCount: '3', oppositeDirection: 'false',
  }),
  counts,
]);
batch.add('sheet-metal-pattern', 'a countersunk hole\'s faces patterned 4 times round the plate\'s middle', [
  plate, holeAt(28, 15, { style: 'HoleStyle.C_SINK', cSinkDiameter: mm(7), cSinkAngle: deg(90) }), axisLine('axis1', [20, 15, 0], [20, 15, 10]),
  call('circularPattern', 'pattern1', {
    patternType: 'PatternType.FACE', faces: holeFaces, axis: edgeOf('axis1'), angle: deg(360), instanceCount: '4', equalSpace: 'true',
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-032.mjs');
