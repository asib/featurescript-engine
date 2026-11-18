// Generates fixture batch 194: more sketch constraints: a circle tangent to an ellipse; a line tangent to a fit spline;
// distances from a point to an ellipse and to a spline; an ellipse made equal to and coincident with another; a
// distance between lines that aren't parallel; points put on an arc and a line beyond their ends; a line normal to
// an ellipse; an ellipse mirrored across a line. Each sketch is drawn off and returns its edges' ends and middles,
// and its points.
//
// Usage: node tools/fixtures/gen-batch-194.mjs
//
// Writes fixtures/batch-194/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 194, title: 'probes: sketch constraints with ellipses, splines and curves\' extensions', helpers });
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


const plain = (body) => [
  `const ${S} = newSketchOnPlane(context, id + "${S}", { "sketchPlane" : ${planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])} });`,
  ...body, solve(S), `return sketchProbe(context, id + "${S}");`,
];
const fit = (name, pts, more = '') => `skFitSpline(${S}, "${name}", { "points" : [${pts.map((p) => vec2(...p)).join(', ')}]${more} });`;
const SPLINE = [[0, 0], [10, 5], [20, 0], [30, 5]];

batch.add('sketch', 'a circle of radius 3 at (14, 4) made tangent to a fixed ellipse of radii 10 and 5 at the origin', [plain([
  ellipse('e1', [0, 0], 10, 5), circle('c2', [14, 4], 3), fix('e1'), pair('TANGENT', 'e1', 'c2'),
])]);
batch.add('sketch', 'a line from (5, 9) to (15, 10) made tangent to a fixed fit spline through (0, 0), (10, 5), (20, 0) and (30, 5)', [plain([
  fit('sp', SPLINE), seg('l1', [5, 9], [15, 10]), fix('sp'), pair('TANGENT', 'sp', 'l1'),
])]);
batch.add('sketch', 'a point drawn at (14, 3) put 2 from that ellipse', [plain([
  ellipse('e1', [0, 0], 10, 5), point('p', [14, 3]), fix('e1'), pair('DISTANCE', 'p', 'e1', { length: mm(2) }),
])]);
batch.add('sketch', 'a point drawn at (11, 9) put 2 from that spline', [plain([
  fit('sp', SPLINE), point('p', [11, 9]), fix('sp'), pair('DISTANCE', 'p', 'sp', { length: mm(2) }),
])]);
batch.add('sketch', 'an ellipse of radii 6 and 3 at (25, 0) made equal to that ellipse', [plain([
  ellipse('e1', [0, 0], 10, 5), ellipse('e2', [25, 0], 6, 3), fix('e1'), pair('EQUAL', 'e1', 'e2'),
])]);
batch.add('sketch', 'an ellipse of radii 8 and 4 at (1, 1), its axis at 10°, made coincident with that ellipse', [plain([
  ellipse('e1', [0, 0], 10, 5), ellipse('e2', [1, 1], 8, 4, [0.984807753012208, 0.17364817766693]), fix('e1'), pair('COINCIDENT', 'e1', 'e2'),
])]);
batch.add('sketch', 'a line from (0, 5) to (20, 8) put 10 from a fixed one from (0, 0) to (20, 0)', [plain([
  seg('l1', [0, 0], [20, 0]), seg('l2', [0, 5], [20, 8]), fix('l1'), pair('DISTANCE', 'l1', 'l2', { length: mm(10) }),
])]);
batch.add('sketch', 'a point drawn at (−8, −7) put on a fixed arc from (10, 0) through (0, 10) to (−10, 0), beyond its ends', [plain([
  arc('a1', [10, 0], [0, 10], [-10, 0]), point('p', [-8, -7]), fix('a1'), pair('COINCIDENT', 'p', 'a1'),
])]);
batch.add('sketch', 'a point drawn at (30, 3) put on a fixed line from (0, 0) to (20, 0), beyond its end', [plain([
  seg('l1', [0, 0], [20, 0]), point('p', [30, 3]), fix('l1'), pair('COINCIDENT', 'p', 'l1'),
])]);
batch.add('sketch', 'a line from (12, 3) to (20, 9) made normal to a fixed ellipse of radii 10 and 5 at the origin', [plain([
  ellipse('e1', [0, 0], 10, 5), seg('l1', [12, 3], [20, 9]), fix('e1'), pair('NORMAL', 'e1', 'l1'),
])]);
batch.add('sketch', 'an ellipse of radii 6 and 3 at (5, 12) mirrored onto one of radii 4 and 2 at (−3, 10) across a fixed vertical line', [plain([
  seg('m', [0, -5], [0, 20]), ellipse('e1', [5, 12], 6, 3), ellipse('e2', [-3, 10], 4, 2), fix('m'), fix('e1'),
  con('mir', 'MIRROR', { localFirst: q('e1'), localSecond: q('e2'), localMirror: q('m') }),
])]);
batch.add('sketch', 'a circle of radius 3 at (14, 4) put 2 from that ellipse', [plain([
  ellipse('e1', [0, 0], 10, 5), circle('c2', [14, 4], 3), fix('e1'), pair('DISTANCE', 'e1', 'c2', { length: mm(2) }),
])]);

batch.write('tools/fixtures/gen-batch-194.mjs');
