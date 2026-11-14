// Generates fixture batch 172: face blend options the engine refused or got wrong: batch 071's floor sheet and upright
// cylinder sheet blended 3 mm detached, with two radii (either way round), by width, untrimmed, limited by a plane,
// with curvature and chamfer sections; batch 054's sheets at right angles blended with a chamfer section; and swept
// profile blends between them along a spine parallel to their corner and slanting across it.
//
// Usage: node tools/fixtures/gen-batch-172.mjs
//
// Writes fixtures/batch-172/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, call, circle, line, mm, planeAt, rect, sketch, solve } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each edge of a body: its curve type and length, and a B-spline's degree, knots and poles (indexed, as std does).
function wireProbe(context is Context, wires is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(wires, EntityType.EDGE)))
    {
        var entry = { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) };
        try
        {
            const c = evCurveDefinition(context, { "edge" : edge });
            if (c is BSplineCurve)
            {
                var points = [];
                for (var i = 0; i < size(c.controlPoints); i += 1)
                    points = append(points, round6(c.controlPoints[i]));
                var knots = [];
                for (var i = 0; i < size(c.knots); i += 1)
                    knots = append(knots, roundToPrecision(c.knots[i], 9));
                entry.bspline = { "degree" : c.degree, "rational" : c.isRational, "points" : points, "knots" : knots };
            }
            else if (c is Line)
                entry.line = true;
            else
                entry.other = true;
        }
        catch (error)
        {
            entry.curve = "failed";
        }
        const ends = evaluateQuery(context, qAdjacent(edge, AdjacencyType.VERTEX, EntityType.VERTEX));
        var points = [];
        for (var v in ends)
            points = append(points, round6(evVertexPoint(context, { "vertex" : v })));
        entry.vertices = points;
        out = append(out, entry);
    }
    return out;
}

// Each face of a body: its area and its surface's type.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType;
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

// The bend lines a finished flex PCB keeps on each solid.
function pcbBendLines(context is Context) returns array
{
    var out = [];
    for (var part in evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)))
    {
        for (var info in getPcbBendLinesAttribute(context, part))
        {
            out = append(out, { "start" : round6(pcbBendLineStartPoint(info)), "end" : round6(pcbBendLineEndPoint(info)),
                                "angle" : roundToPrecision(info.angle / degree, 6), "radius" : roundToPrecision(info.radius / millimeter, 6),
                                "bendUp" : info.bendUp });
        }
    }
    return out;
}
`;

const batch = new Batch({ number: 172, title: 'face blend options between a floor and an upright cylinder', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const xy = PLANES.xy;
// A floor sheet (z = 0, 40 × 40 about the origin) and an upright cylinder sheet (radius 10 about z, 20 tall).
const floorAndCylinder = [
  sketch('s1', xy), rect('s1', 'r1', [-20, -20], [20, 20]), solve('s1'),
  sketch('s2', xy), circle('s2', 'c1', [0, 0], 10), solve('s2'),
  call('extrude', 'cyl', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
];
const blendFields = {
  propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS', asymmetric: 'false',
  crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false', conicHoldLines: 'false', hasCliffEdges: 'false',
  hasCaps: 'false', hasLimits: 'false', hasHelpPoint: 'false', trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false',
  showIsocurves: 'false',
};
const blend = (fields) => call('faceBlend', 'blend1', {
  side1: `qOwnedByBody(${sheetsOf('s1')}, EntityType.FACE)`, flipSide1: 'false', side2: `qOwnedByBody(${sheetsOf('cyl')}, EntityType.FACE)`,
  flipSide2: 'false', radius: mm(3), ...blendFields, ...fields,
});
const probe = returns({ faces: `faceProbe(context, qBodyType(qCreatedBy(id, EntityType.BODY), BodyType.SHEET))` });

batch.add('face-blend', 'a floor and an upright cylinder (radius 10) blended 3 mm, detached', [...floorAndCylinder, blend({ detach: 'true' }), probe]);
batch.add('face-blend', 'the same blended with radii 3 from the floor and 5 from the cylinder', [...floorAndCylinder, blend({ asymmetric: 'true', secondRadius: mm(5), switchAsymmetric: 'false' }), probe]);
batch.add('face-blend', 'the same blended by a width of 4', [...floorAndCylinder, blend({ blendControlType: 'BlendControlType.WIDTH', width: mm(4), widthRatio: '1' }), probe]);
batch.add('face-blend', 'the same blended 3 mm, untrimmed', [...floorAndCylinder, blend({ trim: 'FaceBlendTrimType.NO_TRIM' }), probe]);
batch.add('face-blend', 'the same blended 3 mm, limited by the plane x = 0', [
  ...floorAndCylinder, call('opPlane', 'limit1', { plane: planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0]), width: mm(60), height: mm(60) }),
  blend({ hasLimits: 'true', limitPlane1: `qCreatedBy(id + "limit1", EntityType.FACE)`, limitPlane1Flip: 'false', limitPlane2: 'qNothing()', limitPlane2Flip: 'false', faceLimits: 'qNothing()', edgeLimits: '[]' }), probe,
]);

batch.add('face-blend', 'the floor and cylinder blended with radii 3 and 5, switched', [...floorAndCylinder, blend({ asymmetric: 'true', secondRadius: mm(5), switchAsymmetric: 'true' }), probe]);
batch.add('face-blend', 'the floor and cylinder blended 3 mm, curvature section, magnitude 0.5', [...floorAndCylinder, blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CURVATURE', magnitude: '0.5' }), probe]);
batch.add('face-blend', 'the floor and cylinder blended 3 mm, chamfer section', [...floorAndCylinder, blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CHAMFER' }), probe]);

// Batch 054's sheets: an upright one (y = 0, x 0..30, z 0..20) and a level one (z = 0, x 0..30, y 0..20), meeting
// along the x axis, the blend in the corner between them.
const corner = [
  sketch('u1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), line('u1', 'l1', [0, 0], [30, 0]), solve('u1'),
  call('extrude', 'upright', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('u1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
  sketch('u2', PLANES.xz), line('u2', 'l1', [0, 0], [30, 0]), solve('u2'),
  call('extrude', 'level', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('u2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const cornerBlend = (fields) => call('faceBlend', 'blend1', {
  side1: `qOwnedByBody(${sheetsOf('upright')}, EntityType.FACE)`, flipSide1: 'true', side2: `qOwnedByBody(${sheetsOf('level')}, EntityType.FACE)`,
  flipSide2: 'false', radius: mm(5), ...blendFields, ...fields,
});
const cornerProbe = returns({ faces: `faceProbe(context, qBodyType(qCreatedBy(id, EntityType.BODY), BodyType.SHEET))` });
// A spine 30 above the level sheet: (0, 10) to (30, 10), or slanting from (0, 0) to (30, 10).
const spine = (to) => [sketch('sp', planeAt([0, 0, 30], [0, 0, 1], [1, 0, 0])), line('sp', 'l1', to[1] === 10 ? [0, 10] : [0, 0], to), solve('sp')];
batch.add('face-blend', 'two sheets at right angles blended 5 mm, chamfer section', [...corner, cornerBlend({ crossSectionShape: 'FaceBlendCrossSectionShape.CHAMFER' }), cornerProbe]);
batch.add('face-blend', 'two sheets at right angles, a swept profile 5 mm along a spine parallel to their corner', [
  ...corner, ...spine([30, 10]), cornerBlend({ crossSection: 'FaceBlendCrossSection.SWEPT_PROFILE', spine: `qCreatedBy(id + "sp", EntityType.EDGE)` }), cornerProbe,
]);
batch.add('face-blend', 'two sheets at right angles, a swept profile 5 mm along a spine slanting across their corner', [
  ...corner, ...spine([30, 11]), cornerBlend({ crossSection: 'FaceBlendCrossSection.SWEPT_PROFILE', spine: `qCreatedBy(id + "sp", EntityType.EDGE)` }), cornerProbe,
]);

batch.write('tools/fixtures/gen-batch-172.mjs');
