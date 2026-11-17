// Generates fixture batch 184: which parameters Onshape's sketch constraints take beyond those std's own features
// use (batch 135's ANGLE given an `angle` and MIDPOINT given a point and a line changed nothing): a line at an angle to
// a fixed one, the angle given as `angle` with `aligned`, as `length`, as `value`; a point at a line's midpoint given as
// the UI writes it (`localMidpoint`, `localEntity1`, `localEntity2`) to the line's ends and to the line; a point at a
// circle's quadrant; an ellipse's major and minor diameters; a conic's rho. Each sketch is drawn off and returns its
// edges' ends and middles, and its points.
//
// Usage: node tools/fixtures/gen-batch-184.mjs
//
// Writes fixtures/batch-184/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, planeAt, sketch, solve, vec2 } from './lib.mjs';

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

const batch = new Batch({ number: 184, title: 'probes: the parameters of sketch constraints std\'s features don\'t use', helpers });
const S = 's';
const seg = (name, a, b) => `skLineSegment(${S}, "${name}", { "start" : ${vec2(...a)}, "end" : ${vec2(...b)} });`;
const con = (name, type, fields) =>
  `skConstraint(${S}, "${name}", { "constraintType" : ConstraintType.${type}${Object.entries(fields).map(([k, v]) => `, "${k}" : ${v}`).join('')} });`;
const q = (s) => `"${s}"`;
const solved = (body) => [
  `const ${S} = newSketchOnPlane(context, id + "${S}", { "sketchPlane" : ${planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])} });`,
  ...body, solve(S), `return sketchProbe(context, id + "${S}");`,
];
// A fixed horizontal line and one from its start drawn at about 41°, 20 long.
const angled = (angleFields) => solved([
  seg('l1', [0, 0], [30, 0]), seg('l2', [0.4, 0.3], [15, 13]),
  con('fix', 'FIX', { localFirst: q('l1') }),
  con('co', 'COINCIDENT', { localFirst: q('l1.start'), localSecond: q('l2.start') }),
  con('a', 'ANGLE', { localFirst: q('l1'), localSecond: q('l2'), ...angleFields }),
  con('len', 'LENGTH', { localFirst: q('l2'), length: '20 * millimeter' }),
]);
const point = (name, at) => `skPoint(${S}, "${name}", { "position" : ${vec2(...at)} });`;

batch.add('sketch', 'a line at 30° to a fixed one, the angle given as `angle` with `aligned` false', [angled({ angle: '30 * degree', aligned: 'false' })]);
batch.add('sketch', 'the line at 30°, the angle given as `angle` with `aligned` true', [angled({ angle: '30 * degree', aligned: 'true' })]);
batch.add('sketch', 'the line at 30°, the angle given as `length`', [angled({ length: '30 * degree' })]);
batch.add('sketch', 'the line at 30°, the angle given as `value`', [angled({ value: '30 * degree' })]);
batch.add('sketch', 'a point drawn at (9, 7) made the midpoint of a fixed line from (0, 0) to (20, 10), given as `localMidpoint` to the line\'s ends', [solved([
  seg('l1', [0, 0], [20, 10]), point('p', [9, 7]),
  con('fix', 'FIX', { localFirst: q('l1') }),
  con('mid', 'MIDPOINT', { localMidpoint: q('p'), localEntity1: q('l1.start'), localEntity2: q('l1.end') }),
])]);
batch.add('sketch', 'the point made the line\'s midpoint, given as `localMidpoint` to the line', [solved([
  seg('l1', [0, 0], [20, 10]), point('p', [9, 7]),
  con('fix', 'FIX', { localFirst: q('l1') }),
  con('mid', 'MIDPOINT', { localMidpoint: q('p'), localEntity1: q('l1') }),
])]);
batch.add('sketch', 'a point drawn at (1, 9) put at the quadrant of a fixed circle of radius 10 at the origin', [solved([
  `skCircle(${S}, "c1", { "center" : ${vec2(0, 0)}, "radius" : 10 * millimeter });`, point('p', [1, 9]),
  con('fix', 'FIX', { localFirst: q('c1') }),
  con('qd', 'QUADRANT', { localFirst: q('p'), localSecond: q('c1') }),
])]);
batch.add('sketch', 'an ellipse (radii 10 and 5) with its centre fixed, its major diameter made 30 and its minor 8', [solved([
  `skEllipse(${S}, "e1", { "center" : ${vec2(0, 0)}, "majorRadius" : 10 * millimeter, "minorRadius" : 5 * millimeter });`,
  con('fix', 'FIX', { localFirst: q('e1.center') }),
  con('maj', 'MAJOR_DIAMETER', { localFirst: q('e1'), length: '30 * millimeter' }),
  con('min', 'MINOR_DIAMETER', { localFirst: q('e1'), length: '8 * millimeter' }),
])]);
batch.add('sketch', 'a conic from (0, 0) to (20, 0), its control point at (10, 10), rho 0.5, made rho 0.7, its ends and control fixed', [solved([
  `skConicSegment(${S}, "k1", { "start" : ${vec2(0, 0)}, "controlPoint" : ${vec2(10, 10)}, "end" : ${vec2(20, 0)}, "rho" : 0.5 });`,
  con('f1', 'FIX', { localFirst: q('k1.start') }),
  con('f2', 'FIX', { localFirst: q('k1.end') }),
  con('rho', 'RHO', { localFirst: q('k1'), value: '0.7' }),
])]);

batch.write('tools/fixtures/gen-batch-184.mjs');
