// Generates fixture batch 193: the order of a sketch's curves when nothing is constrained (batch 192 found a circle
// drawn first listed first where the engine listed closed curves last), which spline and Bézier points are point
// bodies, and more constraints: a free line's end made vertical with a fixed circle's centre; two free points made
// horizontal, the newer named first; a free point made vertical with a fixed arc's centre; a point put on a fixed
// elliptical arc, conic and Bézier; a line's end put a distance from a fixed circle. Each sketch is drawn off and
// returns its edges' ends and middles, and its points.
//
// Usage: node tools/fixtures/gen-batch-193.mjs
//
// Writes fixtures/batch-193/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 193, title: 'probes: sketch curves\' order, spline points, more constraints', helpers });
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

batch.add('sketch', 'a circle of radius 5 at (0, 0), then a line from (10, 0) to (20, 5), nothing constrained', [plain([
  circle('c1', [0, 0], 5), seg('l1', [10, 0], [20, 5]),
])]);
batch.add('sketch', 'that line, then that circle, nothing constrained', [plain([
  seg('l1', [10, 0], [20, 5]), circle('c1', [0, 0], 5),
])]);
batch.add('sketch', 'a circle, a line, an ellipse and an arc, nothing constrained', [plain([
  circle('c1', [0, 0], 5), seg('l1', [10, 0], [20, 5]), ellipse('e1', [0, 20], 8, 4), arc('a1', [30, 0], [35, 5], [40, 0]),
])]);
batch.add('sketch', 'a circle and then a line, the line fixed', [plain([
  circle('c1', [0, 0], 5), seg('l1', [10, 0], [20, 5]), fix('l1'),
])]);
batch.add('sketch', 'an open fit spline through (0, 0), (10, 5), (20, 0) and (30, 5), nothing constrained', [plain([
  fit('sp', SPLINE),
])]);
batch.add('sketch', 'a closed fit spline through (0, 0), (20, 0), (20, 10) and (0, 10), nothing constrained', [plain([
  fit('sp', [[0, 0], [20, 0], [20, 10], [0, 10], [0, 0]]),
])]);
batch.add('sketch', 'the open fit spline with end derivatives (10, 10) and (10, −10)', [plain([
  fit('sp', SPLINE, `, "startDerivative" : ${vec2(10, 10)}, "endDerivative" : ${vec2(10, -10)}`),
])]);
batch.add('sketch', 'a Bézier with control points (0, 0), (5, 10), (15, 10) and (20, 0), nothing constrained', [plain([
  `skBezier(${S}, "bz", { "points" : [${[[0, 0], [5, 10], [15, 10], [20, 0]].map((p) => vec2(...p)).join(', ')}] });`,
])]);
batch.add('sketch', 'a free line from (3, 8) to (13, 12) its start made vertical with a fixed circle\'s centre at the origin', [plain([
  circle('c1', [0, 0], 5), seg('l1', [3, 8], [13, 12]), fix('c1'), pair('VERTICAL', 'c1.center', 'l1.start'),
])]);
batch.add('sketch', 'two free points, at (0, 0) and then (10, 3), made horizontal, the newer named first', [plain([
  point('p1', [0, 0]), point('p2', [10, 3]), pair('HORIZONTAL', 'p2', 'p1'),
])]);
batch.add('sketch', 'a point drawn at (4, 15) made vertical with a fixed arc\'s centre (the arc from (10, 0) through (0, 10) to (−10, 0))', [plain([
  arc('a1', [10, 0], [0, 10], [-10, 0]), point('p', [4, 15]), fix('a1'), pair('VERTICAL', 'a1.center', 'p'),
])]);
batch.add('sketch', 'a point drawn at (6, 6) put on a fixed elliptical arc (radii 10 and 5, from parameter 0 to 0.4)', [plain([
  `skEllipticalArc(${S}, "ea", { "center" : ${vec2(0, 0)}, "majorAxis" : vector(1, 0), "majorRadius" : 10 * millimeter, "minorRadius" : 5 * millimeter, "startParameter" : 0, "endParameter" : 0.4 });`,
  point('p', [6, 6]), fix('ea'), pair('COINCIDENT', 'p', 'ea'),
])]);
batch.add('sketch', 'a point drawn at (10, 8) put on a fixed conic from (0, 0) to (20, 0), its control point (10, 10), rho 0.5', [plain([
  `skConicSegment(${S}, "k1", { "start" : ${vec2(0, 0)}, "controlPoint" : ${vec2(10, 10)}, "end" : ${vec2(20, 0)}, "rho" : 0.5 });`,
  point('p', [10, 8]), fix('k1'), pair('COINCIDENT', 'p', 'k1'),
])]);
batch.add('sketch', 'a point drawn at (9, 9) put on a fixed Bézier with control points (0, 0), (5, 10), (15, 10) and (20, 0)', [plain([
  `skBezier(${S}, "bz", { "points" : [${[[0, 0], [5, 10], [15, 10], [20, 0]].map((p) => vec2(...p)).join(', ')}] });`,
  point('p', [9, 9]), fix('bz'), pair('COINCIDENT', 'p', 'bz'),
])]);
batch.add('sketch', 'a free line from (8, 6) to (18, 10), its start put 3 from a fixed circle of radius 5 at the origin', [plain([
  circle('c1', [0, 0], 5), seg('l1', [8, 6], [18, 10]), fix('c1'), pair('DISTANCE', 'l1.start', 'c1', { length: mm(3) }),
])]);

batch.write('tools/fixtures/gen-batch-193.mjs');
