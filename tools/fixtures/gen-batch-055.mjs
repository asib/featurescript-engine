// Generates fixture batch 055: what batches 039, 053 and 054 left open, and loft end conditions and guides. Face
// blends: how far an untrimmed blend runs on (longer sheets, a smaller radius, sheets of unequal extent), short and
// long trims, sheets apart with flips std gets right, uneven radii, width and conics at 60° and 120°. A sphere's
// band extended round it by other distances, with points along its edges; curvature fillets of magnitude 0.86 to
// 0.99; lofts between squares and circles with conditions at their ends, and one with a guide. Cases return their
// faces' surfaces (a B-spline's degrees, knots and poles) or points along their edges, to show how Onshape builds
// them.
//
// Usage: node tools/fixtures/gen-batch-055.mjs
//
// Writes fixtures/batch-055/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, edgeAt, edgeOf, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each face of a body: its area, its surface's type, and for a B-spline its degrees, knots and poles. (Batch 055's
// first build: taking a B-spline's knots with mapArray failed in Onshape with "Execution error"; indexed loops,
// each part caught on its own.)
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            const s = evSurfaceDefinition(context, { "face" : face });
            entry["type"] = s.surfaceType;
            if (s is BSplineSurface)
                entry.bspline = bsplineProbe(s);
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}

// Knot arrays and control point matrices are typed: indexed, as std does, rather than iterated.
function knotsProbe(knots) returns array
{
    var out = [];
    for (var i = 0; i < size(knots); i += 1)
        out = append(out, roundToPrecision(knots[i], 9));
    return out;
}

function bsplineProbe(s) returns map
{
    var b = { "uDegree" : s.uDegree, "vDegree" : s.vDegree, "rational" : s.isRational };
    try
    {
        b.uKnots = knotsProbe(s.uKnots);
        b.vKnots = knotsProbe(s.vKnots);
    }
    catch (error)
    {
        b.knots = "failed";
    }
    try
    {
        const matrix = s.controlPoints;
        var rows = [];
        for (var i = 0; i < size(matrix); i += 1)
        {
            var points = [];
            for (var j = 0; j < size(matrix[i]); j += 1)
                points = append(points, round6(matrix[i][j]));
            rows = append(rows, points);
        }
        b.poles = rows;
    }
    catch (error)
    {
        b.poles = "failed";
    }
    return b;
}

// Nine points along each edge of a body, evenly spaced in its own parameter.
function edgeProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1], "arcLengthParameterization" : false });
        var points = [];
        for (var l in lines)
            points = append(points, round6(l.origin));
        out = append(out, points);
    }
    return out;
}
`;

const batch = new Batch({ number: 55, title: 'face blends, sphere bands extended round, curvature fillets past 0.85, loft end conditions and guides', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
// The case's sheets: the sides (the blended sheet keeps the first's identity) and a detached blend.
const allSheets = `qBodyType(qUnion([qCreatedBy(id + "floor", EntityType.BODY), qCreatedBy(id + "wall", EntityType.BODY), qCreatedBy(id + "blend1", EntityType.BODY)]), BodyType.SHEET)`;

// ----- face blends -----

// Batch 054's sheets: "floor" upright, y = 0, x from x1 to x2, z from z0 up 20 (or leaning along d); "wall" level,
// z = 0, y 0..20, x from x1 to x2.
const upright = (z0, d = [0, 0, 1], [x1, x2] = [0, 30]) => [
  sketch('s1', planeAt([0, 0, z0], d, [1, 0, 0])), line('s1', 'l1', [x1, 0], [x2, 0]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
];
const level = ([x1, x2] = [0, 30]) => [
  sketch('s2', PLANES.xz), line('s2', 'l1', [x1, 0], [x2, 0]), solve('s2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const blend = (fields) =>
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('floor')}, EntityType.FACE)`, flipSide1: 'true', side2: `qOwnedByBody(${sheetsOf('wall')}, EntityType.FACE)`,
    flipSide2: 'false', propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS',
    radius: mm(5), asymmetric: 'false', crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false',
    conicHoldLines: 'false', hasCliffEdges: 'false', hasCaps: 'false', hasLimits: 'false', hasHelpPoint: 'false',
    trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false', showIsocurves: 'false', ...fields,
  });
const blendFaces = returns({ faces: `faceProbe(context, ${allSheets})` });
const untrimmed = { trim: 'FaceBlendTrimType.NO_TRIM' };
const lean = (deg) => [0, Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180)];

batch.add('face-blend', 'two 60 mm sheets at right angles, circular, 5 mm, untrimmed', [upright(0, undefined, [0, 60]), level([0, 60]), blend(untrimmed), blendFaces]);
batch.add('face-blend', 'two 30 mm sheets at right angles, circular, 2 mm, untrimmed', [upright(0), level(), blend({ ...untrimmed, radius: mm(2) }), blendFaces]);
// The level sheet along x 10..50, the upright one 0..30: they overlap along 10..30.
const offset = [upright(0), level([10, 50])];
batch.add('face-blend', 'an upright sheet along x 0..30 and a level one along 10..50, circular, 5 mm, untrimmed', [offset, blend(untrimmed), blendFaces]);
batch.add('face-blend', 'the same, trimmed to the walls', [offset, blend({}), returns({ faces: `faceProbe(context, ${allSheets})`, edges: `edgeProbe(context, ${allSheets})` })]);
batch.add('face-blend', 'the same, short trim', [offset, blend({ trim: 'FaceBlendTrimType.SHORT' }), blendFaces]);
batch.add('face-blend', 'the same, long trim', [offset, blend({ trim: 'FaceBlendTrimType.LONG' }), blendFaces]);
// Batch 054's sheets apart (c009, c010), the level sheet flipped to make up for std's flip.
batch.add('face-blend', 'an upright sheet 2 mm above a level one, circular, 5 mm, the level sheet flipped', [upright(2), level(), blend({ flipSide2: 'true' }), blendFaces]);
batch.add('face-blend', 'the same, conic, rho 0.7', [upright(2), level(), blend({ flipSide2: 'true', crossSectionShape: 'FaceBlendCrossSectionShape.CONIC', rho: '0.7' }), blendFaces]);
batch.add('face-blend', 'an upright sheet at 60° to a level one, circular, radii 5 and 3 mm', [
  upright(0, lean(60)), level(), blend({ asymmetric: 'true', secondRadius: mm(3), switchAsymmetric: 'false' }), blendFaces,
]);
batch.add('face-blend', 'an upright sheet at 120° to a level one, circular, 6 mm wide, ratio 2', [
  upright(0, lean(120)), level(), blend({ blendControlType: 'BlendControlType.WIDTH', width: mm(6), asymmetric: 'true', widthRatio: '2', switchAsymmetric: 'false' }), blendFaces,
]);
batch.add('face-blend', 'an upright sheet at 60° to a level one, conic, 5 mm, rho 0.7', [
  upright(0, lean(60)), level(), blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CONIC', rho: '0.7' }), blendFaces,
]);

// ----- a sphere's band extended round it -----

// Batch 039's: the arc of radius 10 from 0° to 60° in the xz plane swept 90° about z, extended at its edge in the
// xz plane.
const sheet = `qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SHEET)`;
const sphere = [
  sketch('profile', PLANES.xz), arc('profile', 'arc1', [10, 0], [8.660254037844387, 5], [5, 8.660254037844386]), solve('profile'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '90 * degree',
  }),
];
const extend = (edge, distance) =>
  call('extendSurface', 'extend1', {
    entities: edge, endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(distance), oppositeDirection: 'false',
    maintainCurvature: 'false', tangentPropagation: 'false',
  });
for (const d of [1, 2, 5, 10]) {
  batch.add('extend-surface', `a band of a sphere extended ${d} mm round it, at its edge in the xz plane`, [
    sphere, extend(edgeAt(sheet, [8.660254037844387, 0, 5]), d), returns({ faces: `faceProbe(context, ${sheet})`, edges: `edgeProbe(context, ${sheet})` }),
  ]);
}

// ----- curvature fillets past magnitude 0.85 -----

const base = solidsOf('base');
const curvature = (size, radius, magnitude) => [
  box('base', [0, 0, 0], size),
  call('fillet', 'fillet1', { entities: edgeAt(base, [0, 0, 5]), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(radius), magnitude: String(magnitude) }),
  returns({ faces: `faceProbe(context, ${base})` }),
];
for (const magnitude of [0.86, 0.88, 0.9, 0.99]) {
  batch.add('fillet', `a block's upright edge, curvature, 4 mm, magnitude ${magnitude}`, curvature([30, 20, 10], 4, magnitude));
}
batch.add('fillet', 'a 60 × 40 × 10 block\'s upright edge, curvature, 10 mm, magnitude 0.95', curvature([60, 40, 10], 10, 0.95));

// ----- loft end conditions and a guide -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const loft = (profiles, fields) => [
  profiles.map(([name, z, body]) => [sketch(name, at(z)), body(name), solve(name)]),
  call('loft', 'loft1', { sheetProfilesArray: `[${profiles.map(([name]) => `{ "sheetProfileEntities" : ${regions(name)} }`).join(', ')}]`, ...fields }),
  returns({ faces: `faceProbe(context, ${loftBody})` }),
];
const squares = [['s1', 0, (s) => rect(s, 'rect1', [-10, -10], [10, 10])], ['s2', 20, (s) => rect(s, 'rect1', [-5, -5], [5, 5])]];
const circles = [['s1', 0, (s) => circle(s, 'circle1', [0, 0], 12)], ['s2', 30, (s) => circle(s, 'circle1', [0, 0], 6)]];
const start = (condition, magnitude) => ({ startCondition: `LoftEndDerivativeType.${condition}`, startMagnitude: String(magnitude) });
const end = (condition, magnitude) => ({ endCondition: `LoftEndDerivativeType.${condition}`, endMagnitude: String(magnitude) });

batch.add('loft', 'a square to a smaller square, normal to the start profile, magnitude 1', loft(squares, start('NORMAL_TO_PROFILE', 1)));
batch.add('loft', 'the same, magnitude 2', loft(squares, start('NORMAL_TO_PROFILE', 2)));
batch.add('loft', 'the same, normal to both profiles, magnitude 1', loft(squares, { ...start('NORMAL_TO_PROFILE', 1), ...end('NORMAL_TO_PROFILE', 1) }));
batch.add('loft', 'a circle to a smaller circle, normal to the start profile, magnitude 1', loft(circles, start('NORMAL_TO_PROFILE', 1)));
batch.add('loft', 'the same, normal to both profiles, magnitude 1', loft(circles, { ...start('NORMAL_TO_PROFILE', 1), ...end('NORMAL_TO_PROFILE', 1) }));
batch.add('loft', 'the same, tangent to the start profile, magnitude 1', loft(circles, start('TANGENT_TO_PROFILE', 1)));
// Two circles of radius 10, 30 apart, with an arc in the xz plane from (10, 0) through (13, 15) to (10, 30) as a
// guide.
batch.add('loft', 'a circle to an equal circle, guided by an arc bulging out between them', [
  sketch('g', PLANES.xz), arc('g', 'arc1', [10, 0], [13, 15], [10, 30]), solve('g'),
  loft([['s1', 0, (s) => circle(s, 'circle1', [0, 0], 10)], ['s2', 30, (s) => circle(s, 'circle1', [0, 0], 10)]], {
    guidesArray: `[{ "guideEntities" : qUnion([${wireEdges('g')}]) }]`,
  }),
]);

batch.write('tools/fixtures/gen-batch-055.mjs');
