// Generates fixture batch 159: boundary surfaces matching the tangency of a flat face next to a profile: from a level
// sheet's edge up to a line above it, from the edge of a sheet rising at 26.6°, from a level sheet's edge to an arc,
// both ends matching two sheets, and with a magnitude of 2. Then probes of which way a flat fill faces when its edges
// run inconsistently round its loop (batch 157 c004 faced up where the engine faced down): batch 157's gap between a
// level sheet and a rising one filled with its edges listed in other orders, a rectangle of lines drawn half each way
// listed from different sides, and which way the sheets' edges run. Last, two of the boundary surfaces given as v
// profiles only.
//
// Usage: node tools/fixtures/gen-batch-159.mjs
//
// Writes fixtures/batch-159/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, planeAt, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
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

// The fill's area and its normal at the middle of its parameters, rounded to 1e-6.
function facing(context is Context, faces is Query) returns array
{
    var out = [];
    const r = function(x) { return round(x * 1e6) / 1e6; };
    for (var f in evaluateQuery(context, faces))
    {
        const p = evFaceTangentPlane(context, { "face" : f, "parameter" : vector(0.5, 0.5) });
        out = append(out, [r(evArea(context, { "entities" : f }) / millimeter ^ 2), [r(p.normal[0]), r(p.normal[1]), r(p.normal[2])]]);
    }
    return out;
}

// For each point, the start and direction of the edge through it, rounded to 1e-6.
function runs(context is Context, edges is Query, points is array) returns array
{
    var out = [];
    const r = function(x) { return round(x * 1e6) / 1e6; };
    for (var p in points)
    {
        const t = evEdgeTangentLine(context, { "edge" : qContainsPoint(edges, p * millimeter), "parameter" : 0 });
        out = append(out, [[r(t.origin[0] / millimeter), r(t.origin[1] / millimeter), r(t.origin[2] / millimeter)],
                [r(t.direction[0]), r(t.direction[1]), r(t.direction[2])]]);
    }
    return out;
}
`;
const batch = new Batch({ number: 159, title: 'boundary surfaces matching the tangency of flat faces next to their profiles', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetEdge = (body, p) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "${body}", EntityType.BODY), BodyType.SHEET), EntityType.EDGE), ${vec3(...p)})`;
const sheets = `qBodyType(qCreatedBy(id + "surface1", EntityType.BODY), BodyType.SHEET)`;
const profile = (entities, condition, magnitude = 1) =>
  `{ "uProfileEntities" : ${entities}, "uCondition" : BSurfEndDerivativeType.${condition}, "uMagnitude" : ${magnitude}, "uReference" : qNothing(), "uDirection" : qNothing() }`;
const surface = (first, second) => call('boundarySurface', 'surface1', { uProfilesArray: `[${first}, ${second}]`, vProfilesArray: '[]' });
const probe = `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheets}, EntityType.FACE)) };`;
// A level sheet (x from -20 to 0, y from 0 to 20); a sheet rising from x = 10, z = 5 along (2, 0, 1).
const level = [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
];
const rising = [
  sketch('bSketch', planeAt([10, 0, 5], [2 / Math.sqrt(5), 0, 1 / Math.sqrt(5)], [0, 1, 0])), line('bSketch', 'l1', [0, 0], [20, 0]), solve('bSketch'),
  call('extrude', 'b', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('bSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
];
// A line along y at x = 10, z = 5, drawn in the upright plane x = 10.
const target = [sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), line('t', 'l1', [0, 5], [20, 5]), solve('t')];

batch.add('boundary-surface', 'from a level sheet\'s edge, matching its tangency, to a line 10 across and 5 up', [
  ...level, ...target, surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), profile(wires('t'), 'DEFAULT')), probe,
]);
batch.add('boundary-surface', 'from a level sheet\'s edge, matching its tangency with magnitude 2, to the line', [
  ...level, ...target, surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT', 2), profile(wires('t'), 'DEFAULT')), probe,
]);
batch.add('boundary-surface', 'from a level sheet\'s edge to a rising sheet\'s edge, matching both', [
  ...level, ...rising, surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), profile(sheetEdge('b', [10, 10, 5]), 'MATCH_TANGENT')), probe,
]);
batch.add('boundary-surface', 'from the line to a rising sheet\'s edge, matching its tangency', [
  sketch('t', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), line('t', 'l1', [0, 0], [20, 0]), solve('t'), ...rising,
  surface(profile(wires('t'), 'DEFAULT'), profile(sheetEdge('b', [10, 10, 5]), 'MATCH_TANGENT')), probe,
]);
batch.add('boundary-surface', 'from a level sheet\'s edge, matching its tangency, to an arc over x = 10 rising 4 at its middle', [
  ...level, sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), arc('t', 'a1', [0, 5], [10, 9], [20, 5]), solve('t'),
  surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), profile(wires('t'), 'DEFAULT')), probe,
]);

// Batch 157's gap filled flat (lines along y = 0 and y = 20 from the level sheet's edge to the rising one's), its edges
// listed in other orders. Batch 157 listed the level sheet's edge, the rising one's, then the lines.
const sideLines = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('s1', 'l1', [0, 0], [10, 5]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), line('s2', 'l1', [0, 0], [10, 5]), solve('s2'),
];
const fills = `qBodyType(qCreatedBy(id + "fill1", EntityType.BODY), BodyType.SHEET)`;
const fill = (entities) => call('fill', 'fill1', {
  edges: `[${entities.map((e) => `{ "entities" : ${e}, "continuity" : GeometricContinuity.G0 }`).join(', ')}]`,
  surfaceOperationType: 'NewSurfaceOperationType.NEW',
});
const fillProbe = `return { "faces" : facing(context, qOwnedByBody(${fills}, EntityType.FACE)) };`;
const gap = { a: sheetEdge('a', [0, 10, 0]), b: sheetEdge('b', [10, 10, 5]), s1: wires('s1'), s2: wires('s2') };
batch.add('fill', 'batch 157\'s gap filled flat, the lines listed first', [
  ...level, ...rising, ...sideLines, fill([gap.s1, gap.s2, gap.a, gap.b]), fillProbe,
]);
batch.add('fill', 'the gap filled flat, the rising sheet\'s edge listed first', [
  ...level, ...rising, ...sideLines, fill([gap.b, gap.a, gap.s1, gap.s2]), fillProbe,
]);
batch.add('fill', 'the gap filled flat, listed round the loop from the level sheet\'s edge', [
  ...level, ...rising, ...sideLines, fill([gap.a, gap.s2, gap.b, gap.s1]), fillProbe,
]);
// A 30 × 20 rectangle in the top plane, each side its own sketch, the bottom and top drawn anticlockwise round it, the
// right and left clockwise.
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const segments = (list) => list.map(([p, q], i) => [sketch(`e${i}`, top), line(`e${i}`, 'l1', p, q), solve(`e${i}`)]).flat();
const half = segments([[[0, 0], [30, 0]], [[30, 20], [30, 0]], [[30, 20], [0, 20]], [[0, 0], [0, 20]]]);
const sides = (order) => order.map((i) => wires(`e${i}`));
batch.add('fill', 'a rectangle of lines drawn half each way round it, listed from the bottom (drawn anticlockwise)', [
  ...half, fill(sides([0, 1, 2, 3])), fillProbe,
]);
batch.add('fill', 'the rectangle drawn half each way, listed from the right side (drawn clockwise)', [
  ...half, fill(sides([1, 2, 3, 0])), fillProbe,
]);
// Three sides drawn clockwise, the bottom anticlockwise, listed from the bottom.
batch.add('fill', 'a rectangle with three sides drawn clockwise and the bottom anticlockwise, listed from the bottom', [
  ...segments([[[0, 0], [30, 0]], [[30, 20], [30, 0]], [[0, 20], [30, 20]], [[0, 0], [0, 20]]]), fill(sides([0, 1, 2, 3])), fillProbe,
]);
const sheetEdges = (body) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${body}", EntityType.BODY), BodyType.SHEET), EntityType.EDGE)`;
const u = [2 / Math.sqrt(5), 0, 1 / Math.sqrt(5)];
const along = (t, y) => vec3(10 + 20 * u[0] * t, y, 5 + 20 * u[2] * t).replace(/ \* millimeter/g, '');
batch.add('extrude', 'which way the edges of a sheet extruded from a line run: a level sheet and a rising one', [
  ...level, ...rising,
  `return { "level" : runs(context, ${sheetEdges('a')}, [vector(0, 10, 0), vector(-20, 10, 0), vector(-10, 0, 0), vector(-10, 20, 0)]), "rising" : runs(context, ${sheetEdges('b')}, [${along(0, 10)}, ${along(1, 10)}, ${along(0.5, 0)}, ${along(0.5, 20)}]) };`,
]);

// Boundary surfaces given as v profiles only, with end conditions on them: the first and last cases again.
const vProfile = (entities, condition) =>
  `{ "vProfileEntities" : ${entities}, "vCondition" : BSurfEndDerivativeType.${condition}, "vMagnitude" : 1, "vReference" : qNothing(), "vDirection" : qNothing() }`;
const vSurface = (first, second) => call('boundarySurface', 'surface1', { uProfilesArray: '[]', vProfilesArray: `[${first}, ${second}]` });
batch.add('boundary-surface', 'v profiles only: from a level sheet\'s edge, matching its tangency, to a line 10 across and 5 up', [
  ...level, ...target, vSurface(vProfile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), vProfile(wires('t'), 'DEFAULT')), probe,
]);
batch.add('boundary-surface', 'v profiles only: from a level sheet\'s edge, matching its tangency, to the arc', [
  ...level, sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), arc('t', 'a1', [0, 5], [10, 9], [20, 5]), solve('t'),
  vSurface(vProfile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), vProfile(wires('t'), 'DEFAULT')), probe,
]);

batch.write('tools/fixtures/gen-batch-159.mjs');
