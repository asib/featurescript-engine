// Generates fixture batch 150: probes. The edges a rectangle projected normal to a cylinder's side splits it along
// (their ends, directions and curves: std's frame orients its segments from them, batch 138 c005); and how Onshape's
// solver settles under-constrained sketches drawn a little off (which entities move, which keep their lengths,
// directions and radii: batch 147 c001's cam).
//
// Usage: node tools/fixtures/gen-batch-150.mjs
//
// Writes fixtures/batch-150/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, line, mm, planeAt, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
// Each edge's curve type, its ends and its direction at the start, in millimetres, rounded to 1e-6; sorted, as the
// kernels list a split's edges in their own orders.
function edgeProbe(context is Context, edges is Query) returns array
{
    const r = function(v) { return [round(v[0] / millimeter * 1e6) / 1e6, round(v[1] / millimeter * 1e6) / 1e6, round(v[2] / millimeter * 1e6) / 1e6]; };
    var out = [];
    for (var e in evaluateQuery(context, edges))
    {
        const a = evEdgeTangentLine(context, { "edge" : e, "parameter" : 0 });
        const b = evEdgeTangentLine(context, { "edge" : e, "parameter" : 1 });
        const d = a.direction;
        const c = evCurveDefinition(context, { "edge" : e });
        const s = r(a.origin);
        out = append(out, [(c is Line ? 1e6 : 0) + s[2] * 100 + s[1], [c is Line ? "line" : (c is Circle ? "circle" : (c is Ellipse ? "ellipse" : "other")), s, r(b.origin), [round(d[0] * 1e6) / 1e6, round(d[1] * 1e6) / 1e6, round(d[2] * 1e6) / 1e6]]]);
    }
    return mapArray(sort(out, function(a, b) { return a[0] - b[0]; }), function(x) { return x[1]; });
}

// A sketch's solved curves: lines as their ends, circles as centre and radius; sorted, as Onshape lists curves held
// to each other in the order the sketch made them and the engine doesn't (batch 136 c015).
function sketchProbe(context is Context, sketchId is Id) returns array
{
    var out = [];
    for (var e in evaluateQuery(context, qOwnedByBody(qBodyType(qCreatedBy(sketchId, EntityType.BODY), BodyType.WIRE), EntityType.EDGE)))
    {
        const c = evCurveDefinition(context, { "edge" : e });
        const a = evEdgeTangentLine(context, { "edge" : e, "parameter" : 0 }).origin;
        const b = evEdgeTangentLine(context, { "edge" : e, "parameter" : 1 }).origin;
        const r = function(x) { return round(x / millimeter * 1e6) / 1e6; };
        if (c is Line)
            out = append(out, [1e9 + r(a[0]) * 1000 + r(a[1]), ["line", r(a[0]), r(a[1]), r(b[0]), r(b[1])]]);
        else if (c is Circle)
            out = append(out, [r(c.coordSystem.origin[0]) * 1000 + r(c.coordSystem.origin[1]), ["circle", r(c.coordSystem.origin[0]), r(c.coordSystem.origin[1]), r(c.radius)]]);
    }
    return mapArray(sort(out, function(a, b) { return a[0] - b[0]; }), function(x) { return x[1]; });
}
`;
const batch = new Batch({ number: 150, title: 'probes: a split cylinder\'s edges, under-constrained sketches settling', helpers });
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const constraint = (s, name, type, a, b) => `skConstraint(${s}, "${name}", { "constraintType" : ConstraintType.${type}, "localFirst" : "${a}", "localSecond" : "${b}" });`;

batch.add('split', 'a cylinder (radius 10, 30 tall) with its side split by an 8 × 10 rectangle projected from beside it: the split\'s edges\' ends and directions', [
  sketch('baseSketch', top(0)), circle('baseSketch', 'circle1', [0, 0], 10), solve('baseSketch'),
  call('extrude', 'base', { entities: `qSketchRegion(id + "baseSketch")`, endBound: 'BoundingType.BLIND', depth: mm(30) }),
  sketch('cut', planeAt([15, 0, 0], [1, 0, 0], [0, 1, 0])), `skRectangle(cut, "rect1", { "firstCorner" : vector(-4, 10) * millimeter, "secondCorner" : vector(4, 20) * millimeter });`, solve('cut'),
  `splitPart(context, id + "split1", { "splitType" : SplitType.FACE, "faceTargets" : qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SOLID), EntityType.FACE), ${vec3(10, 0, 15)}), "faceTools" : qOwnedByBody(qBodyType(qCreatedBy(id + "cut", EntityType.BODY), BodyType.WIRE), EntityType.EDGE), "projectionType" : ProjectionType.NORMAL_TO_TARGET });`,
  `return { "edges" : edgeProbe(context, qCreatedBy(id + "split1", EntityType.EDGE)) };`,
]);
batch.add('sketch', 'a circle (radius 10) and then a line drawn 0.5 above its top, held tangent', [
  sketch('s', top(0)), circle('s', 'c1', [0, 0], 10), line('s', 'l1', [-15, 10.5], [15, 10.5]), constraint('s', 't1', 'TANGENT', 'l1', 'c1'), solve('s'),
  `return { "curves" : sketchProbe(context, id + "s") };`,
]);
batch.add('sketch', 'a line drawn 0.5 above where a circle (radius 10) drawn after it has its top, held tangent', [
  sketch('s', top(0)), line('s', 'l1', [-15, 10.5], [15, 10.5]), circle('s', 'c1', [0, 0], 10), constraint('s', 't1', 'TANGENT', 'l1', 'c1'), solve('s'),
  `return { "curves" : sketchProbe(context, id + "s") };`,
]);
batch.add('sketch', 'a circle (radius 10) and then a line from 0.5 outside it, its start held on the circle', [
  sketch('s', top(0)), circle('s', 'c1', [0, 0], 10), line('s', 'l1', [10.5, 0], [20, 5]), `skConstraint(s, "k1", { "constraintType" : ConstraintType.COINCIDENT, "localFirst" : "l1.start", "localSecond" : "c1" });`, solve('s'),
  `return { "curves" : sketchProbe(context, id + "s") };`,
]);
batch.add('sketch', 'a circle (radius 10) and then a circle (radius 5) 15.5 to its right, held tangent', [
  sketch('s', top(0)), circle('s', 'c1', [0, 0], 10), circle('s', 'c2', [15.5, 0], 5), constraint('s', 't1', 'TANGENT', 'c1', 'c2'), solve('s'),
  `return { "curves" : sketchProbe(context, id + "s") };`,
]);
batch.add('sketch', 'a circle (radius 10) and then a line drawn 0.5 above its top and tilted 1°, held tangent', [
  sketch('s', top(0)), circle('s', 'c1', [0, 0], 10), line('s', 'l1', [-15, 10.5 - 15 * Math.tan(Math.PI / 180)], [15, 10.5 + 15 * Math.tan(Math.PI / 180)]), constraint('s', 't1', 'TANGENT', 'l1', 'c1'), solve('s'),
  `return { "curves" : sketchProbe(context, id + "s") };`,
]);
batch.add('sketch', 'a line and then a line from 0.5 past its end, their ends held coincident', [
  sketch('s', top(0)), line('s', 'l1', [0, 0], [10, 0]), line('s', 'l2', [10.5, 0.3], [15, 8]), `skConstraint(s, "k1", { "constraintType" : ConstraintType.COINCIDENT, "localFirst" : "l1.end", "localSecond" : "l2.start" });`, solve('s'),
  `return { "curves" : sketchProbe(context, id + "s") };`,
]);
batch.add('sketch', 'two circles (radii 15 and 8, 30 apart) with radius dimensions, joined by two lines drawn a little off, held tangent and with ends on them', [
  sketch('s', top(0)), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8),
  line('s', 'l1', [3.7, 14.486], [31.667, 7.879]), line('s', 'l2', [3.7, -14.486], [31.667, -7.879]),
  constraint('s', 't1', 'TANGENT', 'l1', 'c1'), constraint('s', 't2', 'TANGENT', 'l1', 'c2'), constraint('s', 't3', 'TANGENT', 'l2', 'c1'), constraint('s', 't4', 'TANGENT', 'l2', 'c2'),
  `skConstraint(s, "r1", { "constraintType" : ConstraintType.RADIUS, "localFirst" : "c1", "length" : 15 * millimeter });`,
  `skConstraint(s, "r2", { "constraintType" : ConstraintType.RADIUS, "localFirst" : "c2", "length" : 8 * millimeter });`,
  solve('s'),
  `return { "curves" : sketchProbe(context, id + "s") };`,
]);

batch.write('tools/fixtures/gen-batch-150.mjs');
