// Generates fixture batch 160: probes of how an undimensioned circle settles between two lines held tangent to it and
// to a dimensioned circle, at other angles between the lines (batch 156 found the circle closing four fifths of the gap
// by its radius and a fifth by moving along the lines' bisector, at one angle); and boundary surfaces from a level
// sheet's edge, matching its tangency at magnitudes 2 and 0.5, to an arc (batch 159 c005 at magnitude 1).
//
// Usage: node tools/fixtures/gen-batch-160.mjs
//
// Writes fixtures/batch-160/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, line, planeAt, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
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

// The surface's area, its point and normal at the middle of its parameters, and its heights (along z) over points
// across it, rounded to 1e-6.
function surfaceProbe(context is Context, faces is Query) returns array
{
    var out = [];
    const r = function(x) { return round(x * 1e6) / 1e6; };
    for (var f in evaluateQuery(context, faces))
    {
        const p = evFaceTangentPlane(context, { "face" : f, "parameter" : vector(0.5, 0.5) });
        var heights = [];
        for (var x in [2.5, 5, 7.5])
        {
            for (var y in [5, 10])
            {
                const d = evDistance(context, { "side0" : f, "side1" : line(vector(x, y, 0) * millimeter, vector(0, 0, 1)) });
                heights = append(heights, r(d.sides[0].point[2] / millimeter));
            }
        }
        out = append(out, [r(evArea(context, { "entities" : f }) / millimeter ^ 2),
                [r(p.origin[0] / millimeter), r(p.origin[1] / millimeter), r(p.origin[2] / millimeter)],
                [r(p.normal[0]), r(p.normal[1]), r(p.normal[2])], heights]);
    }
    return out;
}
`;
const batch = new Batch({ number: 160, title: 'probes: undimensioned circles between lines at other angles; boundary surfaces to an arc at other magnitudes', helpers });

// Two circles on the x axis, the first (radius r1, at the origin) dimensioned, the second (radius r2, at x = d) not,
// and their outer tangent lines drawn as batch 156's were: from 0.2 right of and 0.1 nearer the axis than where they
// touch the first circle, to 0.2 left of and 0.1 further from it than where they touch the second (so turned a little,
// and the second circle must change its radius to fit between them); the lower line (starting first by x, then y) is
// the one Onshape holds.
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const tangent = (name, a, b) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.TANGENT, "localFirst" : "${a}", "localSecond" : "${b}" });`;
const radius = (name, c, r) => `skConstraint(s, "${name}", { "constraintType" : ConstraintType.RADIUS, "localFirst" : "${c}", "length" : ${r} * millimeter });`;
const round6 = (x) => Math.round(x * 1e6) / 1e6;
const wedge = (r1, r2, d) => {
  const s = (r1 - r2) / d;
  const c = Math.sqrt(1 - s * s);
  const side = (sign) => {
    const n = [s, sign * c];
    const p = [r1 * n[0], r1 * n[1]];
    const q = [d + r2 * n[0], r2 * n[1]];
    return [[round6(p[0] + 0.2), round6(p[1] - 0.1 * sign)], [round6(q[0] - 0.2), round6(q[1] + 0.1 * sign)]];
  };
  const [u0, u1] = side(1);
  const [l0, l1] = side(-1);
  return [
    sketch('s', top), circle('s', 'c1', [0, 0], r1), circle('s', 'c2', [d, 0], r2), line('s', 'l1', u0, u1), line('s', 'l2', l0, l1),
    tangent('t1', 'l1', 'c1'), tangent('t2', 'l1', 'c2'), tangent('t3', 'l2', 'c1'), tangent('t4', 'l2', 'c2'),
    radius('r1', 'c1', r1), solve('s'), `return { "curves" : sketchProbe(context, id + "s") };`,
  ];
};
batch.add('sketch', 'circles of radius 15 (dimensioned) and 5 (not), 30 apart: the lines\' half-angle\'s sine 1/3', wedge(15, 5, 30));
batch.add('sketch', 'circles of radius 15 and 10, 40 apart: sine 0.125', wedge(15, 10, 40));
batch.add('sketch', 'circles of radius 15 and 12, 20 apart: sine 0.15', wedge(15, 12, 20));
batch.add('sketch', 'circles of radius 20 and 8, 30 apart: sine 0.4', wedge(20, 8, 30));
batch.add('sketch', 'circles of radius 15 and 8, 30 apart (batch 156 c001\'s, its lines drawn otherwise): sine 0.2333', wedge(15, 8, 30));

// From a level sheet's edge (x = 0, z = 0, y from 0 to 20), matching its tangency, to an arc over x = 10 from z = 5
// at its ends to 9 at its middle.
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetEdge = (body, p) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "${body}", EntityType.BODY), BodyType.SHEET), EntityType.EDGE), ${vec3(...p)})`;
const sheets = `qBodyType(qCreatedBy(id + "surface1", EntityType.BODY), BodyType.SHEET)`;
const profile = (entities, condition, magnitude = 1) =>
  `{ "uProfileEntities" : ${entities}, "uCondition" : BSurfEndDerivativeType.${condition}, "uMagnitude" : ${magnitude}, "uReference" : qNothing(), "uDirection" : qNothing() }`;
const surface = (first, second) => call('boundarySurface', 'surface1', { uProfilesArray: `[${first}, ${second}]`, vProfilesArray: '[]' });
const toArc = (magnitude) => [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), arc('t', 'a1', [0, 5], [10, 9], [20, 5]), solve('t'),
  surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT', magnitude), profile(wires('t'), 'DEFAULT')),
  `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheets}, EntityType.FACE)) };`,
];
batch.add('boundary-surface', 'from a level sheet\'s edge, matching its tangency with magnitude 2, to an arc rising 4 at its middle', toArc(2));
batch.add('boundary-surface', 'the same with magnitude 0.5', toArc(0.5));

batch.write('tools/fixtures/gen-batch-160.mjs');
