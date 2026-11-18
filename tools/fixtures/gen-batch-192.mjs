// Generates fixture batch 192: sketch constraints between pairs of entities batches 135, 136 and 184 didn't try:
// two points made horizontal or vertical; distances from a point to a circle (outside it and inside) and to an arc,
// from a line to a circle, between circles; lines made collinear and circles made one; a point on an ellipse and on a
// fit spline; a line tangent to an ellipse; a circle concentric with an ellipse; an ellipse made horizontal; a point
// made an arc's midpoint as the UI writes it. Each sketch is drawn off and returns its edges' ends and middles, and its
// points.
//
// Usage: node tools/fixtures/gen-batch-192.mjs
//
// Writes fixtures/batch-192/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, planeAt, solve, vec2 } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// The sketch's edges' ends and middles, and its points.
function sketchProbe(context is Context, sketchId is Id) returns map
{
    var curves = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(qCreatedBy(sketchId, EntityType.BODY), EntityType.EDGE)))
    {
        const ends = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1] });
        curves = append(curves, [round6(ends[0].origin), round6(ends[1].origin), round6(ends[2].origin)]);
    }
    var points = [];
    for (var v in evaluateQuery(context, qBodyType(qCreatedBy(sketchId, EntityType.BODY), BodyType.POINT)))
        points = append(points, round6(evVertexPoint(context, { "vertex" : qOwnedByBody(v, EntityType.VERTEX) })));
    return { "curves" : curves, "points" : points };
}
`;

const batch = new Batch({ number: 192, title: 'probes: sketch constraints between other pairs of entities', helpers });
const S = 's';
const seg = (name, a, b) => `skLineSegment(${S}, "${name}", { "start" : ${vec2(...a)}, "end" : ${vec2(...b)} });`;
const point = (name, at) => `skPoint(${S}, "${name}", { "position" : ${vec2(...at)} });`;
const circle = (name, c, r) => `skCircle(${S}, "${name}", { "center" : ${vec2(...c)}, "radius" : ${r} * millimeter });`;
const arc = (name, a, m, b) => `skArc(${S}, "${name}", { "start" : ${vec2(...a)}, "mid" : ${vec2(...m)}, "end" : ${vec2(...b)} });`;
const ellipse = (name, c, r1, r2, axis = null) =>
  `skEllipse(${S}, "${name}", { "center" : ${vec2(...c)}, "majorRadius" : ${r1} * millimeter, "minorRadius" : ${r2} * millimeter${axis ? `, "majorAxis" : vector(${axis[0]}, ${axis[1]})` : ''} });`;
const con = (name, type, fields) =>
  `skConstraint(${S}, "${name}", { "constraintType" : ConstraintType.${type}${Object.entries(fields).map(([k, v]) => `, "${k}" : ${v}`).join('')} });`;
const q = (s) => `"${s}"`;
const fix = (name) => con(`fix_${name.replace('.', '_')}`, 'FIX', { localFirst: q(name) });
const pair = (type, a, b, more = {}) => con('c', type, { localFirst: q(a), localSecond: q(b), ...more });
const solved = (body) => [
  `const ${S} = newSketchOnPlane(context, id + "${S}", { "sketchPlane" : ${planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])} });`,
  ...body, solve(S), `return sketchProbe(context, id + "${S}");`,
];
const mm = (v) => `${v} * millimeter`;

batch.add('sketch', 'a point drawn at (26, 9) made horizontal with the end of a fixed line from (0, 0) to (20, 5)', [solved([
  seg('l1', [0, 0], [20, 5]), point('p', [26, 9]), fix('l1'), pair('HORIZONTAL', 'l1.end', 'p'),
])]);
batch.add('sketch', 'a point drawn at (3, 12) made vertical with the start of that fixed line', [solved([
  seg('l1', [0, 0], [20, 5]), point('p', [3, 12]), fix('l1'), pair('VERTICAL', 'l1.start', 'p'),
])]);
batch.add('sketch', 'two free points, at (0, 0) and (10, 3), made horizontal', [solved([
  point('p1', [0, 0]), point('p2', [10, 3]), pair('HORIZONTAL', 'p1', 'p2'),
])]);
batch.add('sketch', 'a point drawn at (12, 5) put 5 from a fixed circle of radius 10 at the origin', [solved([
  circle('c1', [0, 0], 10), point('p', [12, 5]), fix('c1'), pair('DISTANCE', 'p', 'c1', { length: mm(5) }),
])]);
batch.add('sketch', 'a point drawn at (3, 4), inside that circle, put 2 from it', [solved([
  circle('c1', [0, 0], 10), point('p', [3, 4]), fix('c1'), pair('DISTANCE', 'p', 'c1', { length: mm(2) }),
])]);
batch.add('sketch', 'a line from (−10, 14) to (10, 15) put 6 from that circle', [solved([
  circle('c1', [0, 0], 10), seg('l1', [-10, 14], [10, 15]), fix('c1'), pair('DISTANCE', 'l1', 'c1', { length: mm(6) }),
])]);
batch.add('sketch', 'a circle of radius 4 at (20, 3) put 3 from that circle', [solved([
  circle('c1', [0, 0], 10), circle('c2', [20, 3], 4), fix('c1'), pair('DISTANCE', 'c1', 'c2', { length: mm(3) }),
])]);
batch.add('sketch', 'a line from (25, 2) to (40, 5) made collinear (coincident) with a fixed one from (0, 0) to (20, 0)', [solved([
  seg('l1', [0, 0], [20, 0]), seg('l2', [25, 2], [40, 5]), fix('l1'), pair('COINCIDENT', 'l1', 'l2'),
])]);
batch.add('sketch', 'a circle of radius 8 at (1, 1) made coincident with a fixed one of radius 10 at the origin', [solved([
  circle('c1', [0, 0], 10), circle('c2', [1, 1], 8), fix('c1'), pair('COINCIDENT', 'c1', 'c2'),
])]);
batch.add('sketch', 'a point drawn at (6, 6) put on a fixed ellipse of radii 10 and 5 at the origin', [solved([
  ellipse('e1', [0, 0], 10, 5), point('p', [6, 6]), fix('e1'), pair('COINCIDENT', 'p', 'e1'),
])]);
batch.add('sketch', 'a line from (−15, 7) to (15, 8) made tangent to that ellipse', [solved([
  ellipse('e1', [0, 0], 10, 5), seg('l1', [-15, 7], [15, 8]), fix('e1'), pair('TANGENT', 'e1', 'l1'),
])]);
batch.add('sketch', 'a circle of radius 3 at (2, 1) made concentric with that ellipse', [solved([
  ellipse('e1', [0, 0], 10, 5), circle('c2', [2, 1], 3), fix('e1'), pair('CONCENTRIC', 'e1', 'c2'),
])]);
batch.add('sketch', 'an ellipse of radii 10 and 5, its major axis drawn at 20° and its centre fixed, made horizontal', [solved([
  ellipse('e1', [0, 0], 10, 5, [0.9396926207859084, 0.3420201433256687]), fix('e1.center'),
  con('h', 'HORIZONTAL', { localFirst: q('e1') }),
])]);
batch.add('sketch', 'a point drawn at (11, 8) put on a fixed fit spline through (0, 0), (10, 5), (20, 0) and (30, 5)', [solved([
  `skFitSpline(${S}, "sp", { "points" : [${[[0, 0], [10, 5], [20, 0], [30, 5]].map((p) => vec2(...p)).join(', ')}] });`,
  point('p', [11, 8]), fix('sp'), pair('COINCIDENT', 'p', 'sp'),
])]);
batch.add('sketch', 'a point drawn at (1, 7) made the midpoint of a fixed arc from (10, 0) through (0, 10) to (−10, 0), as the UI writes it', [solved([
  arc('a1', [10, 0], [0, 10], [-10, 0]), point('p', [1, 7]), fix('a1'),
  con('mid', 'MIDPOINT', { localMidpoint: q('p'), localEntity1: q('a1') }),
])]);
batch.add('sketch', 'a point drawn at (2, 14) put 3 from that arc', [solved([
  arc('a1', [10, 0], [0, 10], [-10, 0]), point('p', [2, 14]), fix('a1'), pair('DISTANCE', 'p', 'a1', { length: mm(3) }),
])]);

batch.write('tools/fixtures/gen-batch-192.mjs');
