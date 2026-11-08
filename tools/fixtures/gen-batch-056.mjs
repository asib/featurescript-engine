// Generates fixture batch 056: what batch 055 left open. Lofts: profiles off each other's axis (which corners'
// distance sets an end derivative's length), tangent to the profile on a square, along a direction given; face
// blends with conic and curvature sections trimmed to walls of unequal extent, two radii at 120°, width at 60°
// and 120°; curvature fillets of magnitude 0.72 to 0.84; spheres' bands away from the equator extended round.
// Cases return their faces' surfaces (a B-spline's degrees, knots and poles) or points along their edges.
//
// Usage: node tools/fixtures/gen-batch-056.mjs
//
// Writes fixtures/batch-056/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 56, title: 'loft end conditions off axis and along directions, face blends, curvature fillets 0.72 to 0.84, sphere bands', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;

// ----- lofts -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const loft = (profiles, fields, extra = []) => [
  extra,
  profiles.map(([name, z, body]) => [sketch(name, at(z)), body(name), solve(name)]),
  call('loft', 'loft1', { sheetProfilesArray: `[${profiles.map(([name]) => `{ "sheetProfileEntities" : ${regions(name)} }`).join(', ')}]`, ...fields }),
  returns({ faces: `faceProbe(context, ${loftBody})` }),
];
const start = (condition, magnitude) => ({ startCondition: `LoftEndDerivativeType.${condition}`, startMagnitude: String(magnitude) });
const square = ['s1', 0, (s) => rect(s, 'rect1', [-10, -10], [10, 10])];
const circles = [['s1', 0, (s) => circle(s, 'circle1', [0, 0], 12)], ['s2', 30, (s) => circle(s, 'circle1', [0, 0], 6)]];

batch.add('loft', 'a 20 mm square to a 10 mm one 20 mm above, centred 5 mm along x, normal to the start profile', loft([square, ['s2', 20, (s) => rect(s, 'rect1', [0, -5], [10, 5])]], start('NORMAL_TO_PROFILE', 1)));
batch.add('loft', 'a circle of radius 12 to one of 6, 30 mm above, centred 8 mm along x, normal to the start profile', loft([circles[0], ['s2', 30, (s) => circle(s, 'circle1', [8, 0], 6)]], start('NORMAL_TO_PROFILE', 1)));
batch.add('loft', 'a 20 mm square to a 10 mm one 20 mm above, tangent to the start profile', loft([square, ['s2', 20, (s) => rect(s, 'rect1', [-5, -5], [5, 5])]], start('TANGENT_TO_PROFILE', 1)));
batch.add('loft', 'a circle of radius 12 to one of 6, tangent to the start profile, magnitude 0.5', loft(circles, start('TANGENT_TO_PROFILE', 0.5)));
// A direction: a line from the origin towards (1, 0, 1).
batch.add('loft', 'a circle of radius 12 to one of 6, along a direction at 45° to the start profile', loft(circles, { ...start('TANGENT_DIRECTION', 1), startDirection: edgeOf('dir') }, [axisLine('dir', [0, 0, 0], [10, 0, 10])]));
batch.add('loft', 'a circle of radius 12 to one of 6, normal to a direction at 45° to the start profile', loft(circles, { ...start('NORMAL_DIRECTION', 1), startDirection: edgeOf('dir') }, [axisLine('dir', [0, 0, 0], [10, 0, 10])]));

// ----- face blends -----

const upright = (z0, d = [0, 0, 1], [x1, x2] = [0, 30]) => [
  sketch('f1', planeAt([0, 0, z0], d, [1, 0, 0])), line('f1', 'l1', [x1, 0], [x2, 0]), solve('f1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('f1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
];
const level = ([x1, x2] = [0, 30]) => [
  sketch('f2', PLANES.xz), line('f2', 'l1', [x1, 0], [x2, 0]), solve('f2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('f2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const blend = (fields) =>
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('floor')}, EntityType.FACE)`, flipSide1: 'true', side2: `qOwnedByBody(${sheetsOf('wall')}, EntityType.FACE)`,
    flipSide2: 'false', propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS',
    radius: mm(5), asymmetric: 'false', crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false',
    conicHoldLines: 'false', hasCliffEdges: 'false', hasCaps: 'false', hasLimits: 'false', hasHelpPoint: 'false',
    trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false', showIsocurves: 'false', ...fields,
  });
const allSheets = `qBodyType(qUnion([qCreatedBy(id + "floor", EntityType.BODY), qCreatedBy(id + "wall", EntityType.BODY), qCreatedBy(id + "blend1", EntityType.BODY)]), BodyType.SHEET)`;
const blendProbe = returns({ faces: `faceProbe(context, ${allSheets})`, edges: `edgeProbe(context, ${allSheets})` });
const lean = (deg) => [0, Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180)];
const offset = [upright(0), level([10, 50])];

batch.add('face-blend', 'an upright sheet along x 0..30 and a level one along 10..50, conic, 5 mm, rho 0.7, trimmed to the walls', [offset, blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CONIC', rho: '0.7' }), blendProbe]);
batch.add('face-blend', 'the same, curvature, magnitude 0.5', [offset, blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CURVATURE', magnitude: '0.5' }), blendProbe]);
batch.add('face-blend', 'the same, circular, radii 5 and 3 mm', [offset, blend({ asymmetric: 'true', secondRadius: mm(3), switchAsymmetric: 'false' }), blendProbe]);
batch.add('face-blend', 'an upright sheet at 120° to a level one, circular, radii 5 and 3 mm', [upright(0, lean(120)), level(), blend({ asymmetric: 'true', secondRadius: mm(3), switchAsymmetric: 'false' }), blendProbe]);
batch.add('face-blend', 'an upright sheet at 120° to a level one, circular, 6 mm wide', [upright(0, lean(120)), level(), blend({ blendControlType: 'BlendControlType.WIDTH', width: mm(6) }), blendProbe]);
batch.add('face-blend', 'an upright sheet at 60° to a level one, circular, 6 mm wide, ratio 2', [
  upright(0, lean(60)), level(), blend({ blendControlType: 'BlendControlType.WIDTH', width: mm(6), asymmetric: 'true', widthRatio: '2', switchAsymmetric: 'false' }), blendProbe,
]);
batch.add('face-blend', 'an upright sheet at 120° to a level one, circular, 6 mm wide, ratio 0.5', [
  upright(0, lean(120)), level(), blend({ blendControlType: 'BlendControlType.WIDTH', width: mm(6), asymmetric: 'true', widthRatio: '0.5', switchAsymmetric: 'false' }), blendProbe,
]);

// ----- curvature fillets from magnitude 0.72 -----

const base = solidsOf('base');
for (const magnitude of [0.72, 0.76, 0.8, 0.82, 0.84]) {
  batch.add('fillet', `a block's upright edge, curvature, 4 mm, magnitude ${magnitude}`, [
    box('base', [0, 0, 0], [30, 20, 10]),
    call('fillet', 'fillet1', { entities: edgeAt(base, [0, 0, 5]), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(4), magnitude: String(magnitude) }),
    returns({ faces: `faceProbe(context, ${base})` }),
  ]);
}

// ----- spheres' bands extended round -----

const sheet = `qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SHEET)`;
// An arc of radius 10 in the xz plane from latitude a to b, swept 90° about z.
const band = (a, b) => {
  const p = (deg) => [10 * Math.cos((deg * Math.PI) / 180), 10 * Math.sin((deg * Math.PI) / 180)];
  return [
    sketch('profile', PLANES.xz), arc('profile', 'arc1', p(a), p((a + b) / 2), p(b)), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
    call('revolve', 'base', {
      bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
      endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '90 * degree',
    }),
  ];
};
const extendRound = (a, b, d) => {
  const m = ((a + b) / 2) * Math.PI / 180;
  return [
    band(a, b),
    call('extendSurface', 'extend1', {
      entities: edgeAt(sheet, [10 * Math.cos(m), 0, 10 * Math.sin(m)]), endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(d), oppositeDirection: 'false',
      maintainCurvature: 'false', tangentPropagation: 'false',
    }),
    returns({ faces: `faceProbe(context, ${sheet})`, edges: `edgeProbe(context, ${sheet})` }),
  ];
};
batch.add('extend-surface', 'a band of a sphere from latitude 0° to 30° extended 5 mm round it', extendRound(0, 30, 5));
batch.add('extend-surface', 'a band of a sphere from latitude 30° to 60° extended 5 mm round it', extendRound(30, 60, 5));
batch.add('extend-surface', 'a band of a sphere from latitude -30° to 30° extended 5 mm round it', extendRound(-30, 30, 5));

batch.write('tools/fixtures/gen-batch-056.mjs');
