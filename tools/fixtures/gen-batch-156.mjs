// Generates fixture batch 156: probes of how Onshape settles circles without radius dimensions held tangent to
// lines drawn a little off (batch 154 c005 grew its second circle): one circle dimensioned, the other, the circles
// drawn in the other order, the lines drawn exactly tangent to the first circle, one circle between two lines, and
// two circles under one line.
//
// Usage: node tools/fixtures/gen-batch-156.mjs
//
// Writes fixtures/batch-156/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, circle, line, planeAt, sketch, solve } from './lib.mjs';

const helpers = `
// A sketch's solved curves: lines as their ends, circles as centre and radius; sorted (batch 150).
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

const batch = new Batch({ number: 156, title: 'probes: circles without radius dimensions settling against tangent lines', helpers });
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const tangent = (name, a, b) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.TANGENT, "localFirst" : "${a}", "localSecond" : "${b}" });`;
const radius = (name, c, r) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.RADIUS, "localFirst" : "${c}", "length" : ${r} * millimeter });`;
const probe = `return { "curves" : sketchProbe(context, id + "s") };`;

batch.add('sketch', 'only the first circle dimensioned', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8), line('s', 'l1', [3.7, 14.486], [31.667, 7.879]), line('s', 'l2', [3.7, -14.486], [31.667, -7.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), radius('r1', 'c1', 15), solve('s'), probe,
]);
batch.add('sketch', 'only the second circle dimensioned', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8), line('s', 'l1', [3.7, 14.486], [31.667, 7.879]), line('s', 'l2', [3.7, -14.486], [31.667, -7.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), radius('r2', 'c2', 8), solve('s'), probe,
]);
batch.add('sketch', 'the circles drawn second first, neither dimensioned', [
  sketch('s', top), circle('s', 'c2', [30, 0], 8), circle('s', 'c1', [0, 0], 15), line('s', 'l1', [3.7, 14.486], [31.667, 7.879]), line('s', 'l2', [3.7, -14.486], [31.667, -7.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), solve('s'), probe,
]);
batch.add('sketch', 'the lines drawn exactly tangent to the first circle, neither dimensioned', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8), line('s', 'l1', [3.688186, 14.435992], [31.655186, 7.828992]), line('s', 'l2', [3.688186, -14.435992], [31.655186, -7.828992]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), solve('s'), probe,
]);
batch.add('sketch', 'one circle (radius 15, not dimensioned) drawn after two lines held tangent to it', [
  sketch('s', top), line('s', 'l1', [3.7, 14.486], [31.667, 7.879]), line('s', 'l2', [3.7, -14.486], [31.667, -7.879]), circle('s', 'c1', [0, 0], 15),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l2', 'c1'), solve('s'), probe,
]);
batch.add('sketch', 'two circles, neither dimensioned, and one line held tangent to both', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8), line('s', 'l1', [3.7, 14.486], [31.667, 7.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), solve('s'), probe,
]);

batch.write('tools/fixtures/gen-batch-156.mjs');
