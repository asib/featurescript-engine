// Generates fixture batch 043: sheet metal unfolded beyond batch 015's single bend: rolled walls (sheet metal
// extruded along arcs) unfolded alone and beside a flat wall, and every bend of a U channel unfolded at once.
//
// Usage: node tools/fixtures/gen-batch-043.mjs
//
// Writes fixtures/batch-043/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, mm, planeAt, sketch, solve } from './lib.mjs';

const helpers = `
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

function atFraction(context is Context, part is Query, entityType is EntityType, fx is number, fy is number, fz is number) returns Query
{
    return qContainsPoint(qOwnedByBody(part, entityType), pointAtFraction(context, part, fx, fy, fz));
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({ number: 43, title: 'sheet metal unfolded: rolled walls, several bends at once', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const counts = returns({ counts: 'bodyCounts(context)' });
const extruded = (s) =>
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges(s), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  });
const unfoldAll = (hold) => call('sheetMetalUnfold', 'unfold1', { unfoldAll: 'true', partToUnfold: part, entitiesToUnfold: 'qNothing()', holdEntity: hold });
const nearestEdge = (x, y, z) => `qClosestTo(qOwnedByBody(${part}, EntityType.EDGE), vector(${x}, ${y}, ${z}) * millimeter)`;

// A half circle of radius 20 about the origin, from (20, 0) round through y > 0 to (−20, 0).
batch.add('sheet-metal-unfold', 'a rolled wall (half a tube) unfolded, holding its straight edge at (20, 0)', [
  sketch('s1', xy), arc('s1', 'a1', [20, 0], [0, 20], [-20, 0]), solve('s1'), extruded('s1'), unfoldAll(nearestEdge(20, 0, 12.5)), counts,
]);
// A flat wall from (−30, 0) to (0, 0) running on into a quarter circle of radius 20 about (0, 20), up to (20, 20).
batch.add('sheet-metal-unfold', 'a flat wall running on into a rolled wall (a rip between them makes two parts), the rolled wall\'s part unfolded, holding its far edge', [
  sketch('s1', xy), line('s1', 'l1', [-30, 0], [0, 0]), arc('s1', 'a1', [0, 0], [14.142135623730951, 5.857864376269049], [20, 20]), solve('s1'),
  extruded('s1'),
  call('sheetMetalUnfold', 'unfold1', {
    unfoldAll: 'true', partToUnfold: `qOwnerBody(qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0))`,
    entitiesToUnfold: 'qNothing()', holdEntity: nearestEdge(20, 20, 12.5),
  }),
  counts,
]);
// A U channel: a base 30 mm wide with 20 mm sides.
batch.add('sheet-metal-unfold', 'a U channel with every bend unfolded at once, holding its base', [
  sketch('s1', xy), line('s1', 'l1', [0, 20], [0, 0]), line('s1', 'l2', [0, 0], [30, 0]), line('s1', 'l3', [30, 0], [30, 20]), solve('s1'),
  extruded('s1'), unfoldAll(at('FACE', 0.5, 0, 0.5)), counts,
]);

batch.write('tools/fixtures/gen-batch-043.mjs');
