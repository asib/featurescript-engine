// Generates fixture batch 171: probes of a boundary surface leaving an arc normal to its plane (batch 117: its
// derivative's interior poles follow no rule found), on arcs short enough to leave one interior row: a 45° arc
// (radius 20, in the plane y = 0) to a line 5 above it at y = 20, to a 45° arc of radius 15 at y = 20, a 90° arc to a
// line, and the first at magnitude 2. Each probe gives the surface's poles.
//
// Usage: node tools/fixtures/gen-batch-171.mjs
//
// Writes fixtures/batch-171/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, planeAt, sketch, solve } from './lib.mjs';

const helpers = `
function numbersProbe(numbers) returns array
{
    var out = [];
    for (var i = 0; i < size(numbers); i += 1)
        out = append(out, roundToPrecision(numbers[i], 9));
    return out;
}

// Each face's area, its point and normal at the middle of its parameters, and, for a B-spline surface, its degrees,
// knots, weights and poles (rounded to 1e-6 mm).
function surfaceProbe(context is Context, faces is Query) returns array
{
    var out = [];
    const r = function(x) { return roundToPrecision(x, 6); };
    for (var f in evaluateQuery(context, faces))
    {
        const p = evFaceTangentPlane(context, { "face" : f, "parameter" : vector(0.5, 0.5) });
        var entry = { "area" : r(evArea(context, { "entities" : f }) / millimeter ^ 2),
                "middle" : [r(p.origin[0] / millimeter), r(p.origin[1] / millimeter), r(p.origin[2] / millimeter)],
                "normal" : [r(p.normal[0]), r(p.normal[1]), r(p.normal[2])] };
        const s = evSurfaceDefinition(context, { "face" : f });
        entry["type"] = s.surfaceType;
        if (s is BSplineSurface)
        {
            entry.degrees = [s.uDegree, s.vDegree];
            entry.uKnots = numbersProbe(s.uKnots);
            entry.vKnots = numbersProbe(s.vKnots);
            if (s.isRational)
                entry.weights = s.weights;
            var rows = [];
            for (var i = 0; i < size(s.controlPoints); i += 1)
            {
                var points = [];
                for (var j = 0; j < size(s.controlPoints[i]); j += 1)
                {
                    const q = s.controlPoints[i][j];
                    points = append(points, [r(q[0] / millimeter), r(q[1] / millimeter), r(q[2] / millimeter)]);
                }
                rows = append(rows, points);
            }
            entry.poles = rows;
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 171, title: 'probes: boundary surfaces leaving short arcs normal to their planes', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const probe = `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheetsOf('surface1')}, EntityType.FACE)) };`;
const front = (y) => planeAt([0, y, 0], [0, -1, 0], [1, 0, 0]);
const pt = (r, deg) => [Math.round(r * Math.cos(deg * Math.PI / 180) * 1e6) / 1e6, Math.round(r * Math.sin(deg * Math.PI / 180) * 1e6) / 1e6];
// An arc of radius r about the origin of the sketch plane from a0 to a1 degrees (sketch y is world z).
const arcOf = (s, y, r, a0, a1) => [sketch(s, front(y)), arc(s, 'a1', pt(r, a0), pt(r, (a0 + a1) / 2), pt(r, a1)), solve(s)];
const condition = (s, type, magnitude) =>
  `{ "uProfileEntities" : ${wires(s)}, "uCondition" : BSurfEndDerivativeType.${type}, "uMagnitude" : ${magnitude}, "uReference" : qNothing(), "uDirection" : qNothing() }`;
const surface = (magnitude) => call('boundarySurface', 'surface1', { uProfilesArray: `[${condition('p1', 'NORMAL_TO_PROFILE', magnitude)}, ${condition('p2', 'DEFAULT', 1)}]`, vProfilesArray: '[]' });

batch.add('boundary-surface', 'a 45° arc (radius 20) normal to its plane, to a line 5 above it at y = 20', [
  ...arcOf('p1', 0, 20, 67.5, 112.5), sketch('p2', front(20)), line('p2', 'l1', [8, 25], [-8, 25]), solve('p2'), surface(1), probe,
]);
batch.add('boundary-surface', 'the 45° arc normal to its plane, to a 45° arc of radius 15 at y = 20', [
  ...arcOf('p1', 0, 20, 67.5, 112.5), ...arcOf('p2', 20, 15, 67.5, 112.5), surface(1), probe,
]);
batch.add('boundary-surface', 'a 90° arc (radius 20) normal to its plane, to a line at y = 20', [
  ...arcOf('p1', 0, 20, 45, 135), sketch('p2', front(20)), line('p2', 'l1', [15, 25], [-15, 25]), solve('p2'), surface(1), probe,
]);
batch.add('boundary-surface', 'the 45° arc to the line, at magnitude 2', [
  ...arcOf('p1', 0, 20, 67.5, 112.5), sketch('p2', front(20)), line('p2', 'l1', [8, 25], [-8, 25]), solve('p2'), surface(2), probe,
]);

batch.write('tools/fixtures/gen-batch-171.mjs');
