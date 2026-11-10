// Generates fixture batch 088: which way edges run (batch 087's gap): the edges of extrudes up, down and symmetric,
// of an extruded circle and slot, a revolve, and of a block with an edge filleted, an edge chamfered and a hole cut;
// each edge's points in its own order and its tangent at its start.
//
// Usage: node tools/fixtures/gen-batch-088.mjs
//
// Writes fixtures/batch-088/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, edgeAt, edgeOf, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

function dir6(v) returns array
{
    return [roundToPrecision(v[0], 6), roundToPrecision(v[1], 6), roundToPrecision(v[2], 6)];
}

// Every edge of a body: three points along it in its own order and its direction at its start.
function edgeDirections(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6),
                            "points" : mapArray(lines, function(l) { return round6(l.origin); }), "start" : dir6(lines[0].direction) });
    }
    return out;
}
`;

const batch = new Batch({ number: 88, title: 'edge directions of extrudes, a revolve, a fillet, a chamfer and a hole', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const probe = returns({ edges: `edgeDirections(context, ${base})` });
const rectangle = [sketch('s1', xy), rect('s1', 'r1', [0, 0], [40, 30]), solve('s1')];
const extrude = (fields) => call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(10), oppositeDirection: 'false', ...fields });

batch.add('extrude', 'a 40 × 30 rectangle on the xy plane extruded 10 up', [rectangle, extrude({}), probe]);
batch.add('extrude', 'the same, extruded 10 down', [rectangle, extrude({ oppositeDirection: 'true' }), probe]);
batch.add('extrude', 'the same, extruded 10 symmetrically', [rectangle, extrude({ symmetric: 'true' }), probe]);
batch.add('extrude', 'a rectangle drawn as four lines running clockwise, extruded 10 up', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [0, 30]), line('s1', 'l2', [0, 30], [40, 30]), line('s1', 'l3', [40, 30], [40, 0]), line('s1', 'l4', [40, 0], [0, 0]), solve('s1'),
  extrude({}), probe,
]);
batch.add('extrude', 'a circle of radius 10 extruded 10 up', [sketch('s1', xy), circle('s1', 'c1', [0, 0], 10), solve('s1'), extrude({}), probe]);
batch.add('extrude', 'a slot (two lines and two half circles) extruded 10 up', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [20, 0]), arc('s1', 'a1', [20, 0], [25, 5], [20, 10]), line('s1', 'l2', [20, 10], [0, 10]), arc('s1', 'a2', [0, 10], [-5, 5], [0, 0]), solve('s1'),
  extrude({}), probe,
]);
batch.add('revolve', 'a 10 × 20 rectangle in the xz plane, 10 to 20 out from z, revolved 90° about z', [
  sketch('s1', PLANES.xz), rect('s1', 'r1', [10, 0], [20, 20]), solve('s1'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('s1'), axis: edgeOf('axis'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', angle: '90 * degree', oppositeDirection: 'false' }),
  probe,
]);
batch.add('fillet', 'a 40 × 30 × 20 block, its top front edge filleted 5 mm', [
  box('base', [0, 0, 0], [40, 30, 20]), call('fillet', 'fillet1', { entities: edgeAt(base, [20, 0, 20]), radius: mm(5) }), probe,
]);
batch.add('chamfer', 'a 40 × 30 × 20 block, its top front edge chamfered 5 mm', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('chamfer', 'chamfer1', { entities: edgeAt(base, [20, 0, 20]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(5) }), probe,
]);
batch.add('extrude', 'a 40 × 30 × 20 block with a hole of radius 5 cut down through it at (20, 15)', [
  box('base', [0, 0, 0], [40, 30, 20]),
  sketch('h1', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('h1', 'c1', [20, 15], 5), solve('h1'),
  call('extrude', 'hole', { entities: regions('h1'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base }),
  probe,
]);

batch.write('tools/fixtures/gen-batch-088.mjs');
