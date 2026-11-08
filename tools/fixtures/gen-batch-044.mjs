// Generates fixture batch 044: sheet metal lofts beyond batch 017's two parallel lines: L profiles (walls and bends),
// a skew line (a quad that isn't planar), unlike profiles (an L to a line), arcs (tessellated by the chordal
// tolerance), and a loft added to a plate.
//
// Usage: node tools/fixtures/gen-batch-044.mjs
//
// Writes fixtures/batch-044/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, mm, planeAt, rect, sketch, solve } from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({ number: 44, title: 'sheet metal lofts: L profiles, skew lines, unlike profiles, arcs, added to a plate', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const counts = returns({ counts: 'bodyCounts(context)' });
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const up = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const smFields = { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false' };
const loft = (p1, p2, fields = {}) =>
  call('sheetMetalLoft', 'sm1', {
    surfaceOperationType: 'NewSurfaceOperationType.NEW', profile1: p1, profile2: p2, matchConnections: 'false', chordalTolerance: mm(0.1),
    ...smFields, ...fields,
  });

batch.add('sheet-metal-loft', 'walls lofted between an L and the same L 20 mm up and 10 mm along y', [
  sketch('p1', xy), line('p1', 'l1', [0, 0], [30, 0]), line('p1', 'l2', [30, 0], [30, 20]), solve('p1'),
  sketch('p2', up(20)), line('p2', 'l1', [0, 10], [30, 10]), line('p2', 'l2', [30, 10], [30, 30]), solve('p2'),
  loft(wireEdges('p1'), wireEdges('p2')), counts,
]);
batch.add('sheet-metal-loft', 'a wall lofted between a line and a line 20 mm up turned 10 degrees', [
  sketch('p1', xy), line('p1', 'l1', [0, 0], [30, 0]), solve('p1'),
  sketch('p2', up(20)), line('p2', 'l1', [0, 10], [29.544232590366242, 15.209445330007912]), solve('p2'),
  loft(wireEdges('p1'), wireEdges('p2')), counts,
]);
batch.add('sheet-metal-loft', 'walls lofted between an L and a line 20 mm up', [
  sketch('p1', xy), line('p1', 'l1', [0, 0], [30, 0]), line('p1', 'l2', [30, 0], [30, 20]), solve('p1'),
  sketch('p2', up(20)), line('p2', 'l1', [0, 10], [40, 10]), solve('p2'),
  loft(wireEdges('p1'), wireEdges('p2')), counts,
]);
batch.add('sheet-metal-loft', 'a wall lofted between an arc of radius 20 and one of radius 10 above it', [
  sketch('p1', xy), arc('p1', 'a1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('p1'),
  sketch('p2', up(20)), arc('p2', 'a1', [10, 0], [7.0710678118654755, 7.0710678118654755], [0, 10]), solve('p2'),
  loft(wireEdges('p1'), wireEdges('p2')), counts,
]);
batch.add('sheet-metal-loft', 'a wall lofted from a 40 × 30 plate\'s far edge to a line 20 mm up, added to the plate', [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'base', {
    ...smFields, process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
  sketch('p2', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), line('p2', 'l1', [0, 40], [40, 40]), solve('p2'),
  loft(`qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SOLID), EntityType.EDGE), vector(20, 30, 2) * millimeter)`, wireEdges('p2'), {
    surfaceOperationType: 'NewSurfaceOperationType.ADD', booleanScope: `qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SOLID)`,
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-044.mjs');
