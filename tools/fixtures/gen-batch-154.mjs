// Generates fixture batch 154: probes of how Onshape's solver settles two dimensioned circles (radii 15 and 8, 30
// apart) and lines drawn a little off held tangent to them (batch 150 c008 kept its second line and turned its first):
// one line drawn after the circles, one drawn before them, two lines with their constraints given second line first,
// two lines drawn second line first, two lines with no radius dimensions, and a circle drawn after two lines it's held
// tangent to.
//
// Usage: node tools/fixtures/gen-batch-154.mjs
//
// Writes fixtures/batch-154/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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
const batch = new Batch({ number: 154, title: 'probes: dimensioned circles and tangent lines settling', helpers });
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const tangent = (name, a, b) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.TANGENT, "localFirst" : "${a}", "localSecond" : "${b}" });`;
const radius = (name, c, r) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.RADIUS, "localFirst" : "${c}", "length" : ${r} * millimeter });`;
const c1 = circle('s', 'c1', [0, 0], 15);
const c2 = circle('s', 'c2', [30, 0], 8);
const l1 = line('s', 'l1', [3.7, 14.486], [31.667, 7.879]);
const l2 = line('s', 'l2', [3.7, -14.486], [31.667, -7.879]);
const dims = [radius('r1', 'c1', 15), radius('r2', 'c2', 8)];
const probe = `return { "curves" : sketchProbe(context, id + "s") };`;

batch.add('sketch', 'two dimensioned circles, then one line drawn a little off held tangent to both', [
  sketch('s', top), c1, c2, l1, tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), ...dims, solve('s'), probe,
]);
batch.add('sketch', 'a line drawn a little off, then two dimensioned circles it is held tangent to', [
  sketch('s', top), l1, c1, c2, tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), ...dims, solve('s'), probe,
]);
batch.add('sketch', 'two dimensioned circles and two lines held tangent to both, the second line\'s constraints given first', [
  sketch('s', top), c1, c2, l1, l2, tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), ...dims, solve('s'), probe,
]);
batch.add('sketch', 'two dimensioned circles and two lines held tangent to both, the lower line drawn first', [
  sketch('s', top), c1, c2, l2, l1, tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), ...dims, solve('s'), probe,
]);
batch.add('sketch', 'two circles without dimensions and two lines held tangent to both (batch 147\'s cam)', [
  sketch('s', top), c1, c2, l1, l2, tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), solve('s'), probe,
]);
batch.add('sketch', 'two lines drawn a little off, then a dimensioned circle held tangent to both', [
  sketch('s', top), l1, l2, circle('s', 'c1', [0, 0], 15), tangent('t1', 'l1', 'c1'), tangent('t2', 'l2', 'c1'), radius('r1', 'c1', 15), solve('s'), probe,
]);
batch.add('sketch', 'two dimensioned circles and two lines held tangent to both, the radius dimensions given first', [
  sketch('s', top), c1, c2, l1, l2, ...dims, tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'), solve('s'), probe,
]);

batch.write('tools/fixtures/gen-batch-154.mjs');
