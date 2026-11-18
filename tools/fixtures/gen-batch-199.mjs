// Generates fixture batch 199: open circular patterns (batch 198 c003 found Onshape spacing the instances evenly by a
// step of its own, not the angle given): the seed, a circle of radius 2 at (10, 0), fixed, and the copies drawn at
// distance 10 at given angles, with radius 2 unless said; the angle given varied. Each sketch returns its edges'
// ends and middles, and its points.
//
// Usage: node tools/fixtures/gen-batch-199.mjs
//
// Writes fixtures/batch-199/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 199, title: 'probes: open sketch circular patterns', helpers });
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

batch.add('sketch', "an open pattern: two copies drawn at 80° and 170°, the angle given 180°", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [1.736481777, 9.84807753], 2), circle('c2', [-9.84807753, 1.736481777], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 180 * degree, "patterng" : 1, "patternc1" : 3, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2" });`,
])]);
batch.add('sketch', "an open pattern: three copies drawn at 90°, 180° and 270°, the angle given 270°", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [0.0, 10.0], 2), circle('c2', [-10.0, 0.0], 2), circle('c3', [-0.0, -10.0], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 270 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3" });`,
])]);
batch.add('sketch', "an open pattern: three copies drawn at 80°, 180° and 270°, the angle given 270°", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [1.736481777, 9.84807753], 2), circle('c2', [-10.0, 0.0], 2), circle('c3', [-0.0, -10.0], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 270 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3" });`,
])]);
batch.add('sketch', "an open pattern: three copies drawn at 90°, 170° and 270°, the angle given 270°", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [0.0, 10.0], 2), circle('c2', [-9.84807753, 1.736481777], 2), circle('c3', [-0.0, -10.0], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 270 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3" });`,
])]);
batch.add('sketch', "an open pattern: three copies drawn at 90°, 180° and 260°, the angle given 270°", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [0.0, 10.0], 2), circle('c2', [-10.0, 0.0], 2), circle('c3', [-1.736481777, -9.84807753], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 270 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3" });`,
])]);
batch.add('sketch', "an open pattern: three copies drawn at 90°, 180° and 270°, the angle given 300°", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [0.0, 10.0], 2), circle('c2', [-10.0, 0.0], 2), circle('c3', [-0.0, -10.0], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 300 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3" });`,
])]);
batch.add('sketch', "an open pattern: batch 198 c003's copies, (1, 9), (−9, 1) and (0, −8), their radii drawn 2", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [1, 9], 2), circle('c2', [-9, 1], 2), circle('c3', [0, -8], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 270 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3" });`,
])]);
batch.add('sketch', "an open pattern: four copies drawn at 70°, 140°, 210° and 280°, the angle given 360°", [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [3.420201433, 9.396926208], 2), circle('c2', [-7.660444431, 6.427876097], 2), circle('c3', [-8.660254038, -5.0], 2), circle('c4', [1.736481777, -9.84807753], 2), fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 360 * degree, "patterng" : 1, "patternc1" : 5, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3", "localInstance0,4" : "c4" });`,
])]);

batch.write('tools/fixtures/gen-batch-199.mjs');
