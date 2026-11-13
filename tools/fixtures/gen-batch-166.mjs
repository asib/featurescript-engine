// Generates fixture batch 166: boundary surfaces with an end condition where a profile is more than one edge (which
// the engine refused): from a level sheet's edge in two pieces, matching it, to a line and to an arc; from two sheets'
// edges in a row whose faces lie in different planes, matching both; and from one edge to a line in two pieces. Each
// probe gives the surface's faces with their poles.
//
// Usage: node tools/fixtures/gen-batch-166.mjs
//
// Writes fixtures/batch-166/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 166, title: 'boundary surfaces with end conditions on profiles of more than one edge', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const probe = `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheetsOf('surface1')}, EntityType.FACE)) };`;
const edgesOf = (body, points) => `qUnion([${points.map((p) => `qContainsPoint(qOwnedByBody(${sheetsOf(body)}, EntityType.EDGE), ${vec3(...p)})`).join(', ')}])`;
const condition = (entities, type) =>
  `{ "uProfileEntities" : ${entities}, "uCondition" : BSurfEndDerivativeType.${type}, "uMagnitude" : 1, "uReference" : qNothing(), "uDirection" : qNothing() }`;
const surface = (first, second) => call('boundarySurface', 'surface1', { uProfilesArray: `[${first}, ${second}]`, vProfilesArray: '[]' });
// A level sheet (x from -20 to 0, y from 0 to 20) extruded from two lines along x = 0 meeting at y = 10.
const level = [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [10, 0]), line('aSketch', 'l2', [10, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
];
const levelEdges = edgesOf('a', [[0, 5, 0], [0, 15, 0]]);
const target = (sk) => [sketch(sk, planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), line(sk, 'l1', [0, 5], [20, 5]), solve(sk)];

batch.add('boundary-surface', 'from a level sheet\'s edge in two pieces, matching it, to a line 10 across and 5 up', [
  ...level, ...target('t'), surface(condition(levelEdges, 'MATCH_TANGENT'), condition(wires('t'), 'DEFAULT')), probe,
]);
batch.add('boundary-surface', 'from the edge in two pieces, matching it, to an arc rising 4 at its middle', [
  ...level, sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), arc('t', 'a1', [0, 5], [10, 9], [20, 5]), solve('t'),
  surface(condition(levelEdges, 'MATCH_TANGENT'), condition(wires('t'), 'DEFAULT')), probe,
]);
// Two sheets with edges in a row along x = 0: a level one for y from 0 to 10, and one dipping back from y = 10 to 20
// along (-2, 0, -1).
batch.add('boundary-surface', 'from two sheets\' edges in a row, one level and one dipping, matching both, to the line', [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [10, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('bSketch', planeAt([0, 0, 0], [-2 / Math.sqrt(5), 0, -1 / Math.sqrt(5)], [0, 1, 0])), line('bSketch', 'l1', [10, 0], [20, 0]), solve('bSketch'),
  call('extrude', 'b', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('bSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  ...target('t'),
  surface(condition(`qUnion([${edgesOf('a', [[0, 5, 0]])}, ${edgesOf('b', [[0, 15, 0]])}])`, 'MATCH_TANGENT'), condition(wires('t'), 'DEFAULT')), probe,
]);
batch.add('boundary-surface', 'from a level sheet\'s single edge, matching it, to a line in two pieces', [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), line('t', 'l1', [0, 5], [8, 5]), line('t', 'l2', [8, 5], [20, 5]), solve('t'),
  surface(condition(edgesOf('a', [[0, 10, 0]]), 'MATCH_TANGENT'), condition(wires('t'), 'DEFAULT')), probe,
]);

batch.write('tools/fixtures/gen-batch-166.mjs');
