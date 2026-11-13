// Generates fixture batch 163: probes tabulating the extra term in a boundary surface's derivative at magnitudes other
// than 1 (batch 162: a linear operator on the rows of poles, scaled by (magnitude - 1) and the derivative, set by the
// knots across the profiles alone). From a level sheet's edge (or a sheet extruded from a straight spline, its edge
// carrying the knots), matching its tangency, to straight splines at x = 10, z = 5 of one to five even spans, of
// uneven ones, at magnitude 0.5, and to a rising sheet's edge matching it too. Each probe gives the surface's poles.
//
// Usage: node tools/fixtures/gen-batch-163.mjs
//
// Writes fixtures/batch-163/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, line, planeAt, sketch, solve, vec3 } from './lib.mjs';

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

const batch = new Batch({ number: 163, title: 'probes: the extra term in a boundary surface\'s derivative at magnitudes other than 1, by the knots across its profiles', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const probe = `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheetsOf('surface1')}, EntityType.FACE)) };`;
const edgeOf = (body, p) => `qContainsPoint(qOwnedByBody(${sheetsOf(body)}, EntityType.EDGE), ${vec3(...p)})`;
const curve = (name) => `qOwnedByBody(qCreatedBy(id + "${name}", EntityType.BODY), EntityType.EDGE)`;
// A level sheet (x from -20 to 0, y from 0 to 20) extruded from a line along x = 0.
const level = [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
];
// The same sheet extruded from a straight spline along x = 0 through y = 0, 10, 20 (two spans).
const levelSpline = [
  call('opFitSpline', 'aCurve', { points: `[${vec3(0, 0, 0)}, ${vec3(0, 10, 0)}, ${vec3(0, 20, 0)}]` }),
  call('opExtrude', 'a', { entities: curve('aCurve'), direction: 'vector(-1, 0, 0)', endBound: 'BoundingType.BLIND', endDepth: '20 * millimeter' }),
];
// A sheet rising from x = 10, z = 5 along (2, 0, 1) extruded from a straight spline through y = 0, 10, 20.
const rising = [
  call('opFitSpline', 'bCurve', { points: `[${vec3(10, 0, 5)}, ${vec3(10, 10, 5)}, ${vec3(10, 20, 5)}]` }),
  call('opExtrude', 'b', { entities: curve('bCurve'), direction: 'normalize(vector(2, 0, 1))', endBound: 'BoundingType.BLIND', endDepth: '20 * millimeter' }),
];
const condition = (entities, type, magnitude) =>
  `{ "uProfileEntities" : ${entities}, "uCondition" : BSurfEndDerivativeType.${type}, "uMagnitude" : ${magnitude}, "uReference" : qNothing(), "uDirection" : qNothing() }`;
const surface = (second, magnitude, secondType = 'DEFAULT') => call('boundarySurface', 'surface1', {
  uProfilesArray: `[${condition(edgeOf('a', [0, 10, 0]), 'MATCH_TANGENT', magnitude)}, ${condition(second, secondType, 1)}]`, vProfilesArray: '[]',
});
const spline = (ys) => call('opFitSpline', 't', { points: `[${ys.map((y) => vec3(10, y, 5)).join(', ')}]` });
const even = (spans) => Array.from({ length: spans + 1 }, (_, i) => (20 * i) / spans);

batch.add('boundary-surface', 'to a straight spline of one span (through y = 0, 20), magnitude 2', [...level, spline(even(1)), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to a straight spline of three even spans, magnitude 2', [...level, spline(even(3)), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to a straight spline of four even spans, magnitude 2', [...level, spline(even(4)), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to a straight spline of five even spans, magnitude 2', [...level, spline(even(5)), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to a straight spline through y = 0, 2, 10, 20 (three uneven spans), magnitude 2', [...level, spline([0, 2, 10, 20]), surface(curve('t'), 2), probe]);
batch.add('boundary-surface', 'to a straight spline of two even spans, magnitude 0.5', [...level, spline(even(2)), surface(curve('t'), 0.5), probe]);
batch.add('boundary-surface', 'from a sheet extruded from a two-span straight spline, magnitude 2, to a line', [
  ...levelSpline, sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), line('t', 'l1', [0, 5], [20, 5]), solve('t'),
  surface(wires('t'), 2), probe,
]);
batch.add('boundary-surface', 'from the two-span sheet at magnitude 2 to a rising two-span sheet\'s edge, matching both', [
  ...levelSpline, ...rising, surface(edgeOf('b', [10, 10, 5]), 2, 'MATCH_TANGENT'), probe,
]);
batch.add('boundary-surface', 'from the two-span sheet at magnitude 1 to the rising sheet\'s edge, matching both', [
  ...levelSpline, ...rising, surface(edgeOf('b', [10, 10, 5]), 1, 'MATCH_TANGENT'), probe,
]);

batch.write('tools/fixtures/gen-batch-163.mjs');
