// Generates fixture batch 136: how Onshape's sketch solver moves an underdetermined sketch (batch 135 c018: a line's
// length set, its start held where a point was coincident), and constraints batch 135 didn't try. Each sketch is
// drawn off and pulled into place by its constraints; each case returns its sketch's curves.
//
// Underdetermined: a free line made 15 long; with its start fixed; with its end fixed; with its start, then its
// end, coincident with a free point; a free point put on a free line; a free circle given a radius; two free lines
// drawn apart, joined; a free line made horizontal; a free line made parallel to a fixed one.
//
// Other constraints, fully determined: a circle tangent to a fixed one outside it; a point on a fixed circle,
// fixed in x by a distance; a point a distance from a fixed line along a fixed vertical; circles made equal; a line
// normal to a fixed circle; points mirrored; a distance between parallel lines.
//
// Usage: node tools/fixtures/gen-batch-136.mjs
//
// Writes fixtures/batch-136/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, mm, planeAt, sketch, solve, vec2 } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// A sketch's curves: each edge's start, middle and end, in the order the sketch made them; then its points.
function sketchProbe(context is Context, sketchId is Id) returns map
{
    var curves = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(qBodyType(qCreatedBy(sketchId, EntityType.BODY), BodyType.WIRE), EntityType.EDGE)))
    {
        const ends = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1] });
        curves = append(curves, [round6(ends[0].origin), round6(ends[1].origin), round6(ends[2].origin)]);
    }
    var points = [];
    for (var vertex in evaluateQuery(context, qOwnedByBody(qBodyType(qCreatedBy(sketchId, EntityType.BODY), BodyType.POINT), EntityType.VERTEX)))
        points = append(points, round6(evVertexPoint(context, { "vertex" : vertex })));
    return { "curves" : curves, "points" : points };
}
`;

const batch = new Batch({ number: 136, title: 'sketch solving: underdetermined sketches, more constraints', helpers });
const S = 's';
const seg = (name, a, b, extra = '') => `skLineSegment(${S}, "${name}", { "start" : ${vec2(...a)}, "end" : ${vec2(...b)}${extra} });`;
const pt = (name, p) => `skPoint(${S}, "${name}", { "position" : ${vec2(...p)} });`;
const circ = (name, c, r) => `skCircle(${S}, "${name}", { "center" : ${vec2(...c)}, "radius" : ${mm(r)} });`;
const con = (name, type, fields) =>
  `skConstraint(${S}, "${name}", { "constraintType" : ConstraintType.${type}${Object.entries(fields).map(([k, v]) => `, "${k}" : ${v}`).join('')} });`;
const q = (s) => `"${s}"`;
const solved = (body) => [
  sketch(S, planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), body, solve(S),
  `return sketchProbe(context, id + "${S}");`,
];
const len = (name, of, l) => con(name, 'LENGTH', { localFirst: q(of), length: mm(l) });
const fix = (name, of) => con(name, 'FIX', { localFirst: q(of) });
const coincident = (name, a, b) => con(name, 'COINCIDENT', { localFirst: q(a), localSecond: q(b) });

// ----- underdetermined -----

batch.add('sketch', 'a free line from (0, 0) to (10, 0) made 15 long', solved([seg('l1', [0, 0], [10, 0]), len('len', 'l1', 15)]));
batch.add('sketch', 'a line from (0, 0) to (10, 0), its start fixed, made 15 long', solved([seg('l1', [0, 0], [10, 0]), fix('f', 'l1.start'), len('len', 'l1', 15)]));
batch.add('sketch', 'a line from (0, 0) to (10, 0), its end fixed, made 15 long', solved([seg('l1', [0, 0], [10, 0]), fix('f', 'l1.end'), len('len', 'l1', 15)]));
batch.add('sketch', 'a line from (0, 0) to (10, 0), its start coincident with a free point there, made 15 long', solved([
  seg('l1', [0, 0], [10, 0]), pt('p', [0, 0]), coincident('c', 'p', 'l1.start'), len('len', 'l1', 15),
]));
batch.add('sketch', 'a line from (0, 0) to (10, 0), its end coincident with a free point there, made 15 long', solved([
  seg('l1', [0, 0], [10, 0]), pt('p', [10, 0]), coincident('c', 'p', 'l1.end'), len('len', 'l1', 15),
]));
batch.add('sketch', 'a free line from (0, 0) to (10, 0) and a free point at (5, 2) put on it', solved([
  seg('l1', [0, 0], [10, 0]), pt('p', [5, 2]), coincident('c', 'p', 'l1'),
]));
batch.add('sketch', 'a free circle at (5, 5) of radius 3 given radius 4', solved([circ('c1', [5, 5], 3), con('r', 'RADIUS', { localFirst: q('c1'), length: mm(4) })]));
batch.add('sketch', 'two free lines, (0, 0) to (10, 0) and (11, 1) to (20, 5), the first\'s end joined to the second\'s start', solved([
  seg('l1', [0, 0], [10, 0]), seg('l2', [11, 1], [20, 5]), coincident('c', 'l1.end', 'l2.start'),
]));
batch.add('sketch', 'a free line from (0, 0) to (10, 2) made horizontal', solved([seg('l1', [0, 0], [10, 2]), con('h', 'HORIZONTAL', { localFirst: q('l1') })]));
batch.add('sketch', 'a free line from (0, 5) to (10, 7) made parallel to a fixed line from (0, 0) to (10, 0)', solved([
  seg('l1', [0, 0], [10, 0]), seg('l2', [0, 5], [10, 7]), fix('f', 'l1'), con('p', 'PARALLEL', { localFirst: q('l1'), localSecond: q('l2') }),
]));

// ----- other constraints, fully determined -----

batch.add('sketch', 'a fixed circle at (0, 0) of radius 5 and a circle at (9, 1) of radius 3, its centre fixed in y at 0 and its radius 3, made tangent outside', solved([
  circ('c1', [0, 0], 5), circ('c2', [9, 1], 3), fix('f', 'c1'),
  con('r', 'RADIUS', { localFirst: q('c2'), length: mm(3) }),
  pt('origin', [0, 0]), fix('fo', 'origin'),
  con('dy', 'DISTANCE', { localFirst: q('origin'), localSecond: q('c2.center'), length: mm(0), direction: 'DimensionDirection.VERTICAL' }),
  con('t', 'TANGENT', { localFirst: q('c1'), localSecond: q('c2') }),
]));
batch.add('sketch', 'a point at (4, 4) put on a fixed circle at (0, 0) of radius 5, 3 to the right of a fixed point at the origin', solved([
  circ('c1', [0, 0], 5), fix('f', 'c1'), pt('origin', [0, 0]), fix('fo', 'origin'), pt('p', [4, 4]),
  coincident('c', 'p', 'c1'), con('dx', 'DISTANCE', { localFirst: q('origin'), localSecond: q('p'), length: mm(3), direction: 'DimensionDirection.HORIZONTAL' }),
]));
batch.add('sketch', 'a point at (3, 4) on a fixed vertical line at x = 3, 6 from a fixed horizontal line along y = 0', solved([
  seg('base', [0, 0], [20, 0]), seg('wall', [3, -2], [3, 20]), fix('f1', 'base'), fix('f2', 'wall'), pt('p', [3, 4]),
  coincident('c', 'p', 'wall'), con('d', 'DISTANCE', { localFirst: q('p'), localSecond: q('base'), length: mm(6) }),
]));
batch.add('sketch', 'a fixed circle of radius 5 at (0, 0) and a circle of radius 3 at (12, 0), its centre fixed, made equal', solved([
  circ('c1', [0, 0], 5), circ('c2', [12, 0], 3), fix('f', 'c1'), fix('fc', 'c2.center'), con('eq', 'EQUAL', { localFirst: q('c1'), localSecond: q('c2') }),
]));
batch.add('sketch', 'a fixed circle of radius 5 at (0, 0) and a line from (6, 1) to (12, 3), its start fixed, made normal to the circle', solved([
  circ('c1', [0, 0], 5), seg('l1', [6, 1], [12, 3]), fix('f', 'c1'), fix('fs', 'l1.start'), len('len', 'l1', 6),
  con('n', 'NORMAL', { localFirst: q('l1'), localSecond: q('c1') }),
]));
batch.add('sketch', 'a fixed point at (2, 3) and a point at (17, 4) made its mirror image across a fixed vertical construction line at x = 10', solved([
  seg('axis', [10, -5], [10, 25], ', "construction" : true'), pt('p1', [2, 3]), pt('p2', [17, 4]), fix('fa', 'axis'), fix('fp', 'p1'),
  con('m', 'MIRROR', { localMirror: q('axis'), localFirst: q('p1'), localSecond: q('p2') }),
]));
batch.add('sketch', 'a fixed horizontal line and a parallel line drawn 4 above it, made 6 from it, its start fixed in x by a vertical line', solved([
  seg('l1', [0, 0], [20, 0]), seg('l2', [0, 4], [20, 4]), seg('wall', [0, -5], [0, 20], ', "construction" : true'),
  fix('f1', 'l1'), fix('fw', 'wall'), coincident('c', 'l2.start', 'wall'), len('len', 'l2', 20),
  con('p', 'PARALLEL', { localFirst: q('l1'), localSecond: q('l2') }), con('d', 'DISTANCE', { localFirst: q('l1'), localSecond: q('l2'), length: mm(6) }),
]));

batch.write('tools/fixtures/gen-batch-136.mjs');
