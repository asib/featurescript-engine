// Generates fixture batch 173: probes of the poles of Onshape's B-spline face blends where batch 172 left the engine
// short: between a floor and an upright cylinder with two radii (either way round), a curvature section and a conic
// one (batch 084 c007), and swept along the cylinder's base; and swept profiles between sheets at right angles along a
// spine parallel to their corner and slanting across it. Each probe: every face of the case's sheets with its poles,
// and the edges that aren't lines with theirs.
//
// Usage: node tools/fixtures/gen-batch-173.mjs
//
// Writes fixtures/batch-173/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, call, circle, line, mm, planeAt, rect, sketch, solve } from './lib.mjs';

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

// Each edge of the faces that isn't a line: its length, and for a B-spline curve its degree, knots, weights and
// poles (rounded to 1e-6 mm); otherwise its type.
function edgeProbe(context is Context, faces is Query) returns array
{
    var out = [];
    const r = function(x) { return roundToPrecision(x, 6); };
    for (var e in evaluateQuery(context, qOwnedByBody(faces, EntityType.EDGE)))
    {
        const c = evCurveDefinition(context, { "edge" : e, "returnBSplinesAsOther" : false });
        if (c is Line)
            continue;
        var entry = { "length" : r(evLength(context, { "entities" : e }) / millimeter) };
        if (c is BSplineCurve)
        {
            entry.degree = c.degree;
            entry.knots = numbersProbe(c.knots);
            if (c.isRational)
                entry.weights = c.weights;
            var points = [];
            for (var q in c.controlPoints)
                points = append(points, [r(q[0] / millimeter), r(q[1] / millimeter), r(q[2] / millimeter)]);
            entry.poles = points;
        }
        else
            entry["type"] = (c is Circle) ? "CIRCLE" : ((c is Ellipse) ? "ELLIPSE" : "OTHER");
        out = append(out, entry);
    }
    return out;
}
`;
const batch = new Batch({ number: 173, title: 'probes: the poles of B-spline face blends between a floor and a cylinder, and of swept profile blends', helpers });
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const xy = PLANES.xy;
const mine = 'qBodyType(qCreatedBy(id, EntityType.BODY), BodyType.SHEET)';
const probe = `return { "faces" : surfaceProbe(context, qOwnedByBody(${mine}, EntityType.FACE)), "edges" : edgeProbe(context, ${mine}) };`;
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

batch.add('face-blend', 'a floor and an upright cylinder (radius 10) blended with radii 3 and 5 (batch 172 c002): its poles', [...floorAndCylinder, blend({ asymmetric: 'true', secondRadius: mm(5), switchAsymmetric: 'false' }), probe]);
batch.add('face-blend', 'the same, switched (batch 172 c006): its poles', [...floorAndCylinder, blend({ asymmetric: 'true', secondRadius: mm(5), switchAsymmetric: 'true' }), probe]);
batch.add('face-blend', 'the same blended 3 mm, curvature section, magnitude 0.5 (batch 172 c007): its poles', [...floorAndCylinder, blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CURVATURE', magnitude: '0.5' }), probe]);
batch.add('face-blend', 'the same blended 3 mm, conic section, rho 0.7 (batch 084 c007): its poles', [...floorAndCylinder, blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CONIC', rho: '0.7' }), probe]);
batch.add('face-blend', 'the same blended 3 mm, a swept profile along the circle at the cylinder\'s base', [
  ...floorAndCylinder, blend({ crossSection: 'FaceBlendCrossSection.SWEPT_PROFILE', spine: `qCreatedBy(id + "s2", EntityType.EDGE)` }), probe,
]);

// Batch 054's sheets at right angles, meeting along the x axis.
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
const spine = (from, to) => [sketch('sp', planeAt([0, 0, 30], [0, 0, 1], [1, 0, 0])), line('sp', 'l1', from, to), solve('sp')];
const swept = { crossSection: 'FaceBlendCrossSection.SWEPT_PROFILE', spine: `qCreatedBy(id + "sp", EntityType.EDGE)` };
batch.add('face-blend', 'two sheets at right angles, a swept profile 5 mm along a spine parallel to their corner (batch 172 c010): its poles', [...corner, ...spine([0, 10], [30, 10]), cornerBlend(swept), probe]);
batch.add('face-blend', 'the same along a spine slanting across their corner (batch 172 c011): its poles', [...corner, ...spine([0, 0], [30, 11]), cornerBlend(swept), probe]);

batch.write('tools/fixtures/gen-batch-173.mjs');
