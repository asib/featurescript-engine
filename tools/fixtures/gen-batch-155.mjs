// Generates fixture batch 155: probes of which of two lines held tangent to two dimensioned circles (radii 15 and 8)
// Onshape keeps where it was (batch 154 kept the lower): the layout mirrored, shifted up, down and right, turned a
// quarter turn, and each line drawn further off than the other.
//
// Usage: node tools/fixtures/gen-batch-155.mjs
//
// Writes fixtures/batch-155/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 155, title: 'probes: which of two tangent lines holds', helpers });
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const tangent = (name, a, b) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.TANGENT, "localFirst" : "${a}", "localSecond" : "${b}" });`;
const radius = (name, c, r) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.RADIUS, "localFirst" : "${c}", "length" : ${r} * millimeter });`;
const probe = `return { "curves" : sketchProbe(context, id + "s") };`;

batch.add('sketch', 'mirrored: the first line below, the second above', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8),
  line('s', 'l1', [3.7, -14.486], [31.667, -7.879]), line('s', 'l2', [3.7, 14.486], [31.667, 7.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
  radius('r1', 'c1', 15), radius('r2', 'c2', 8), solve('s'), probe,
]);
batch.add('sketch', 'shifted 40 up (both lines above the origin)', [
  sketch('s', top), circle('s', 'c1', [0, 40], 15), circle('s', 'c2', [30, 40], 8),
  line('s', 'l1', [3.7, 54.486], [31.667, 47.879]), line('s', 'l2', [3.7, 25.514], [31.667, 32.121]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
  radius('r1', 'c1', 15), radius('r2', 'c2', 8), solve('s'), probe,
]);
batch.add('sketch', 'shifted 40 down (both lines below the origin)', [
  sketch('s', top), circle('s', 'c1', [0, -40], 15), circle('s', 'c2', [30, -40], 8),
  line('s', 'l1', [3.7, -25.514], [31.667, -32.121]), line('s', 'l2', [3.7, -54.486], [31.667, -47.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
  radius('r1', 'c1', 15), radius('r2', 'c2', 8), solve('s'), probe,
]);
batch.add('sketch', 'turned a quarter turn (lines running up)', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [0, 30], 8),
  line('s', 'l1', [-14.486, 3.7], [-7.879, 31.667]), line('s', 'l2', [14.486, 3.7], [7.879, 31.667]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
  radius('r1', 'c1', 15), radius('r2', 'c2', 8), solve('s'), probe,
]);
batch.add('sketch', 'the upper line drawn 0.2 further off than the lower', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8),
  line('s', 'l1', [3.7, 14.686], [31.667, 8.079]), line('s', 'l2', [3.7, -14.486], [31.667, -7.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
  radius('r1', 'c1', 15), radius('r2', 'c2', 8), solve('s'), probe,
]);
batch.add('sketch', 'the lower line drawn 0.2 further off than the upper', [
  sketch('s', top), circle('s', 'c1', [0, 0], 15), circle('s', 'c2', [30, 0], 8),
  line('s', 'l1', [3.7, 14.486], [31.667, 7.879]), line('s', 'l2', [3.7, -14.686], [31.667, -8.079]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
  radius('r1', 'c1', 15), radius('r2', 'c2', 8), solve('s'), probe,
]);
batch.add('sketch', 'shifted 40 right', [
  sketch('s', top), circle('s', 'c1', [40, 0], 15), circle('s', 'c2', [70, 0], 8),
  line('s', 'l1', [43.7, 14.486], [71.667, 7.879]), line('s', 'l2', [43.7, -14.486], [71.667, -7.879]),
  tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
  radius('r1', 'c1', 15), radius('r2', 'c2', 8), solve('s'), probe,
]);

batch.write('tools/fixtures/gen-batch-155.mjs');
