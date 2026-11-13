// Generates fixture batch 161: the poles of Onshape's own fits where the engine differs (a three-sided fill of fit
// splines, batch 153 c004; a three-sided fill of two lines and a rising arc; fills tangent to both sheets across batch
// 157's gap and to the level one only; boundary surfaces from a level sheet's edge to an arc at magnitude 2, rising 4
// and 2), and fills whose curvature condition or guide point the plane already meets.
//
// Usage: node tools/fixtures/gen-batch-161.mjs
//
// Writes fixtures/batch-161/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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
const batch = new Batch({ number: 161, title: 'probes: the poles of Onshape\'s three-sided and tangent fills and of boundary surfaces at magnitude 2; fills whose curvature condition or guide the plane meets', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const probe = (name) => `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheetsOf(name)}, EntityType.FACE)) };`;
const edgeOf = (body, p) => `qContainsPoint(qOwnedByBody(${sheetsOf(body)}, EntityType.EDGE), ${vec3(...p)})`;
const fill = (entries, extra = {}) => call('fill', 'fill1', {
  edges: `[${entries.map(([q, c]) => `{ "entities" : ${q}, "continuity" : GeometricContinuity.${c} }`).join(', ')}]`,
  surfaceOperationType: 'NewSurfaceOperationType.NEW', ...extra,
});

// Batch 153 c004: three fit splines through (0, 0, 0), (30, 0, 0), (15, 25, 0), each bulging 4 up at its middle.
batch.add('fill', 'a triangle of fit splines bulging 4 up (batch 153 c004): its poles', [
  call('opFitSpline', 'e1', { points: `[${vec3(0, 0, 0)}, ${vec3(15, 0, 4)}, ${vec3(30, 0, 0)}]` }),
  call('opFitSpline', 'e2', { points: `[${vec3(30, 0, 0)}, ${vec3(22.5, 12.5, 4)}, ${vec3(15, 25, 0)}]` }),
  call('opFitSpline', 'e3', { points: `[${vec3(15, 25, 0)}, ${vec3(7.5, 12.5, 4)}, ${vec3(0, 0, 0)}]` }),
  fill([[`qUnion([${wires('e1')}, ${wires('e2')}, ${wires('e3')}])`, 'G0']]), probe('fill1'),
]);
// Two lines in the top plane, (0, 0) to (30, 0) to (15, 25), and an arc from (15, 25) back to (0, 0) rising 4 above
// the plane at its middle, in the upright plane through them.
const slant = [-15 / Math.hypot(15, 25), -25 / Math.hypot(15, 25), 0];
batch.add('fill', 'a three-sided loop of two lines in the top plane and an arc rising 4 over the third side: its poles', [
  sketch('e1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), line('e1', 'l1', [0, 0], [30, 0]), line('e1', 'l2', [30, 0], [15, 25]), solve('e1'),
  sketch('e2', planeAt([15, 25, 0], [slant[1], -slant[0], 0], slant)), arc('e2', 'a1', [0, 0], [Math.hypot(15, 25) / 2, 4], [Math.hypot(15, 25), 0]), solve('e2'),
  fill([[`qUnion([${wires('e1')}, ${wires('e2')}])`, 'G0']]), probe('fill1'),
]);
// Batch 157's gap: a level sheet (x from -20 to 0) and one rising from x = 10, z = 5 along (2, 0, 1), lines along
// y = 0 and y = 20 between their edges.
const gap = [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('bSketch', planeAt([10, 0, 5], [2 / Math.sqrt(5), 0, 1 / Math.sqrt(5)], [0, 1, 0])), line('bSketch', 'l1', [0, 0], [20, 0]), solve('bSketch'),
  call('extrude', 'b', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('bSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('s1', 'l1', [0, 0], [10, 5]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), line('s2', 'l1', [0, 0], [10, 5]), solve('s2'),
];
const gapFill = (ca, cb) => fill([[edgeOf('a', [0, 10, 0]), ca], [edgeOf('b', [10, 10, 5]), cb], [`qUnion([${wires('s1')}, ${wires('s2')}])`, 'G0']]);
batch.add('fill', 'batch 157\'s gap filled tangent to both sheets (157 c001): its poles', [...gap, gapFill('G1', 'G1'), probe('fill1')]);
batch.add('fill', 'the gap filled tangent to the level sheet only (157 c002): its poles', [...gap, gapFill('G1', 'G0'), probe('fill1')]);
batch.add('fill', 'the gap filled with curvature continuity to the rising sheet, which the plane continues', [...gap, gapFill('G0', 'G2'), probe('fill1')]);
// A 30 × 20 rectangle of lines in the top plane with a sketch point at (10, 5) as a guide.
batch.add('fill', 'a rectangle of lines filled with a guide point in its plane', [
  sketch('r', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])),
  line('r', 'l1', [0, 0], [30, 0]), line('r', 'l2', [30, 0], [30, 20]), line('r', 'l3', [30, 20], [0, 20]), line('r', 'l4', [0, 20], [0, 0]), solve('r'),
  sketch('g', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), `skPoint(g, "p1", { "position" : vector(10, 5) * millimeter });`, solve('g'),
  fill([[wires('r'), 'G0']], { addGuides: 'true', guideEntities: `qCreatedBy(id + "g", EntityType.VERTEX)`, constraintMode: 'FillConstraintMode.PRECISE' }),
  probe('fill1'),
]);
// Boundary surfaces from the level sheet's edge, matching its tangency at magnitude 2, to an arc over x = 10 from
// z = 5 at its ends rising 4 (batch 160 c006) and 2 at its middle.
const sheetEdge = (p) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "a", EntityType.BODY), BodyType.SHEET), EntityType.EDGE), ${vec3(...p)})`;
const toArc = (rise, magnitude) => [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), arc('t', 'a1', [0, 5], [10, 5 + rise], [20, 5]), solve('t'),
  call('boundarySurface', 'surface1', {
    uProfilesArray: `[{ "uProfileEntities" : ${sheetEdge([0, 10, 0])}, "uCondition" : BSurfEndDerivativeType.MATCH_TANGENT, "uMagnitude" : ${magnitude}, "uReference" : qNothing(), "uDirection" : qNothing() }, { "uProfileEntities" : ${wires('t')}, "uCondition" : BSurfEndDerivativeType.DEFAULT, "uMagnitude" : 1, "uReference" : qNothing(), "uDirection" : qNothing() }]`,
    vProfilesArray: '[]',
  }),
  probe('surface1'),
];
batch.add('boundary-surface', 'from a level sheet\'s edge at magnitude 1 to an arc rising 4 (batch 159 c005): its poles', toArc(4, 1));
batch.add('boundary-surface', 'the same at magnitude 2 (batch 160 c006): its poles', toArc(4, 2));
batch.add('boundary-surface', 'at magnitude 2 to an arc rising 2: its poles', toArc(2, 2));

batch.write('tools/fixtures/gen-batch-161.mjs');
