// Generates fixture batch 162: probes isolating the extra term in a boundary surface's derivative at magnitudes other
// than 1 (batch 161 c008, c009: from a level sheet's edge to an arc, the interior rows of poles shift by a term that
// comes from the sheet's edge, not the arc). Each case changes one thing: the far profile straight but a spline of two
// spans, with uneven knots, with uneven speed along it; the gap twice as wide; magnitude 3; the sheet's edge running the
// other way; the arc drawn the other way; the profiles listed the other way round; and the straight spline at
// magnitude 1. Each probe gives the surface's poles.
//
// Usage: node tools/fixtures/gen-batch-162.mjs
//
// Writes fixtures/batch-162/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, planeAt, sketch, solve, vec3 } from './lib.mjs';

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

const batch = new Batch({ number: 162, title: 'probes: what the extra term in a boundary surface\'s derivative at magnitudes other than 1 depends on', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const probe = `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheetsOf('surface1')}, EntityType.FACE)) };`;
const sheetEdge = `qContainsPoint(qOwnedByBody(${sheetsOf('a')}, EntityType.EDGE), ${vec3(0, 10, 0)})`;
// A level sheet (x from -20 to 0, y from 0 to 20), its edge along x = 0 drawn from y = 0 to 20 (or back).
const level = (reversed = false) => [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), reversed ? line('aSketch', 'l1', [20, 0], [0, 0]) : line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
];
const condition = (entities, type, magnitude) =>
  `{ "uProfileEntities" : ${entities}, "uCondition" : BSurfEndDerivativeType.${type}, "uMagnitude" : ${magnitude}, "uReference" : qNothing(), "uDirection" : qNothing() }`;
const surface = (target, magnitude, swapped = false) => {
  const first = condition(sheetEdge, 'MATCH_TANGENT', magnitude);
  const second = condition(target, 'DEFAULT', 1);
  return call('boundarySurface', 'surface1', { uProfilesArray: swapped ? `[${second}, ${first}]` : `[${first}, ${second}]`, vProfilesArray: '[]' });
};
// An arc over x = gap from z = 5 at y = 0 and 20 rising 4 at its middle (drawn from y = 20 back, if reversed).
const arcAt = (gap, reversed = false) => [
  sketch('t', planeAt([gap, 0, 0], [1, 0, 0], [0, 1, 0])),
  reversed ? arc('t', 'a1', [20, 5], [10, 9], [0, 5]) : arc('t', 'a1', [0, 5], [10, 9], [20, 5]), solve('t'),
];
const spline = (ys, parameters) => call('opFitSpline', 't', {
  points: `[${ys.map((y) => vec3(10, y, 5)).join(', ')}]`, ...(parameters ? { parameters: `[${parameters.join(', ')}]` } : {}),
});
const curve = (name) => `qOwnedByBody(qCreatedBy(id + "${name}", EntityType.BODY), EntityType.EDGE)`;

batch.add('boundary-surface', 'to a straight spline through y = 0, 10, 20 at x = 10, z = 5, magnitude 2', [...level(), spline([0, 10, 20]), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to a straight spline through y = 0, 5, 20 (knots uneven), magnitude 2', [...level(), spline([0, 5, 20]), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to a straight spline through y = 0, 10, 20 at parameters 0, 0.3, 1 (speed uneven), magnitude 2', [...level(), spline([0, 10, 20], [0, 0.3, 1]), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to the straight spline through y = 0, 10, 20 at magnitude 1', [...level(), spline([0, 10, 20]), surface(curve('t'), 1), probe]);
batch.add('boundary-surface', 'to an arc rising 4 over x = 20 (the gap twice as wide), magnitude 2', [...level(), ...arcAt(20), surface(wires('t'), 2), probe]);
batch.add('boundary-surface', 'to the arc over x = 10, magnitude 3', [...level(), ...arcAt(10), surface(wires('t'), 3), probe]);
batch.add('boundary-surface', 'to the arc, magnitude 2, the sheet\'s edge drawn from y = 20 back', [...level(true), ...arcAt(10), surface(wires('t'), 2), probe]);
batch.add('boundary-surface', 'to the arc drawn from y = 20 back, magnitude 2', [...level(), ...arcAt(10, true), surface(wires('t'), 2), probe]);
batch.add('boundary-surface', 'the arc listed first, the sheet\'s edge (magnitude 2) second', [...level(), ...arcAt(10), surface(wires('t'), 2, true), probe]);

batch.write('tools/fixtures/gen-batch-162.mjs');
