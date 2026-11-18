// Generates fixture batch 198: sketch constraints only the UI writes (std's features use none): circular patterns as a
// forum post quotes the UI's code (`angle`, `patterng`, `patternc1`, `openPattern`, `localPivot`, `sketchToolType`,
// and `localInstance<g>,<i>` naming each instance's entities), with the seed named as instance 0 and without, two
// entities to an instance, an open pattern; and a point pierced by a 3D line, named three ways. Each sketch is drawn
// off and returns its edges' ends and middles, and its points.
//
// Usage: node tools/fixtures/gen-batch-198.mjs
//
// Writes fixtures/batch-198/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 198, title: 'probes: sketch circular pattern and pierce constraints', helpers });
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

// A circle of radius 2 at (10, 0) fixed, three more drawn off, a point at the origin fixed as the pivot.
const pattern = (fields, instances, extra = []) => solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [1, 9], 1), circle('c2', [-9, 1], 3), circle('c3', [0, -8], 2),
  ...extra, fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 90 * degree, "patterng" : ${fields.groups ?? 1}, "patternc1" : ${fields.count ?? 4}, "openPattern" : ${fields.open ?? 'false'}, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN${instances.map(([k, v]) => `, "localInstance${k}" : "${v}"`).join('')} });`,
]);
batch.add('sketch', 'a circular pattern of four circles a quarter turn apart about a fixed point, the fixed seed named instance 0', [pattern({}, [['0,0', 'c0'], ['0,1', 'c1'], ['0,2', 'c2'], ['0,3', 'c3']])]);
batch.add('sketch', 'the pattern, only the three copies named, as instances 1 to 3', [pattern({}, [['0,1', 'c1'], ['0,2', 'c2'], ['0,3', 'c3']])]);
batch.add('sketch', 'the pattern open, its angle the whole spread (three copies over 270°), all four named', [solved([
  point('p', [0, 0]), circle('c0', [10, 0], 2), circle('c1', [1, 9], 1), circle('c2', [-9, 1], 3), circle('c3', [0, -8], 2),
  fix('p'), fix('c0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 270 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : true, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "c0", "localInstance0,1" : "c1", "localInstance0,2" : "c2", "localInstance0,3" : "c3" });`,
])]);
batch.add('sketch', 'a pattern of a line from (8, 0) to (12, 0), fixed, and three lines drawn off, a quarter turn apart', [solved([
  point('p', [0, 0]), seg('l0', [8, 0], [12, 0]), seg('l1', [1, 7], [2, 13]), seg('l2', [-7, 1], [-13, 0]), seg('l3', [1, -9], [0, -11]),
  fix('p'), fix('l0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 90 * degree, "patterng" : 1, "patternc1" : 4, "openPattern" : false, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "l0", "localInstance0,1" : "l1", "localInstance0,2" : "l2", "localInstance0,3" : "l3" });`,
])]);
batch.add('sketch', 'a pattern of two entities, a circle and a point, three times a third of a turn apart', [solved([
  point('p', [0, 0]), circle('a0', [10, 0], 2), point('b0', [14, 0]), circle('a1', [-4, 9], 1), point('b1', [-6, 11]), circle('a2', [-5, -8], 3), point('b2', [-8, -12]),
  fix('p'), fix('a0'), fix('b0'),
  `skConstraint(${S}, "pat", { "constraintType" : ConstraintType.CIRCULAR_PATTERN, "angle" : 120 * degree, "patterng" : 2, "patternc1" : 3, "openPattern" : false, "localPivot" : "p", "sketchToolType" : SketchToolType.PATTERN, "localInstance0,0" : "a0", "localInstance1,0" : "b0", "localInstance0,1" : "a1", "localInstance1,1" : "b1", "localInstance0,2" : "a2", "localInstance1,2" : "b2" });`,
])]);
// A line from (5, 3, −10) to (5, 3, 10) crossing the sketch plane at (5, 3), and a point drawn at (1, 1).
const pierce = (fields) => [
  `opFitSpline(context, id + "line", { "points" : [vector(5, 3, -10) * millimeter, vector(5, 3, 10) * millimeter] });`,
  ...solved([point('p', [1, 1]), `skConstraint(${S}, "pierce", { "constraintType" : ConstraintType.PIERCE${Object.entries(fields).map(([k, v]) => `, "${k}" : ${v}`).join('')} });`]),
];
const lineEdge = 'qCreatedBy(id + "line", EntityType.EDGE)';
batch.add('sketch', 'a point pierced by a 3D line, as `localFirst` and `externalSecond`', [pierce({ localFirst: q('p'), externalSecond: lineEdge })]);
batch.add('sketch', 'the point pierced by the line, as `localEntity` and `externalEdge` (a forum post\'s)', [pierce({ localEntity: q('p'), externalEdge: lineEdge })]);
batch.add('sketch', 'the point pierced by the line, as `externalFirst` and `localSecond`', [pierce({ externalFirst: lineEdge, localSecond: q('p') })]);

batch.write('tools/fixtures/gen-batch-198.mjs');
