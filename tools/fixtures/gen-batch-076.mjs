// Generates fixture batch 076: a sweep and a frame locked to a sphere along a tilted great circle, a quartic
// B-spline (a fit spline raised to degree 4) extended both ways, offset curves extended (trimmed) and split, and a
// thin loft with its ends trimmed to the profiles' planes.
//
// Usage: node tools/fixtures/gen-batch-076.mjs
//
// Writes fixtures/batch-076/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
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

// Each edge of a body: its length, and a B-spline's degree, knots and poles (indexed, as std does).
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
            else if (c is Circle)
                entry.circle = { "radius" : roundToPrecision(c.radius / millimeter, 6) };
            else if (c is Line)
                entry.line = true;
        }
        catch (error)
        {
            entry.curve = "failed";
        }
        out = append(out, entry);
    }
    return out;
}

// Each edge's ends.
function edgeEnds(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 1], "arcLengthParameterization" : false });
        out = append(out, mapArray(lines, function(l) { return round6(l.origin); }));
    }
    return out;
}

// A frame profile, the only sketch of its own context, as a profile from an element library would be.
const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-2, -2) * millimeter, "secondCorner" : vector(2, 2) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-1.5, -1.5) * millimeter, "secondCorner" : vector(1.5, 1.5) * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}
`;

const batch = new Batch({
  number: 76,
  title: 'a sweep and a frame locked to a sphere, a quartic extended, offset curves extended and split, a thin loft trimmed to its profiles',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const createdBy = (name) => `qCreatedBy(id + "${name}", EntityType.BODY)`;
const xy = PLANES.xy;

// ----- locked to a sphere -----

// A sphere of radius 20 about the origin; a quarter of the great circle in the plane through the origin with normal
// (0, sin 30°, cos 30°), from (20, 0, 0) round towards (0, 17.32, -10).
const n = [0, 0.5, 0.8660254037844386];
const great = [
  call('opSphere', 'base', { center: vec3(0, 0, 0), radius: mm(20) }),
  sketch('path', planeAt([0, 0, 0], n, [1, 0, 0])), arc('path', 'a1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('path'),
];
const sphereFace = `qOwnedByBody(${solidsOf('base')}, EntityType.FACE)`;
batch.add('sweep', 'a 2 x 3 rectangle swept along a quarter of a tilted great circle of a sphere, locked to the sphere', [
  great,
  sketch('prof', planeAt([20, 0, 0], [0, 0.8660254037844386, -0.5], [1, 0, 0])), rect('prof', 'r', [0, 0], [2, 3]), solve('prof'),
  call('sweep', 'sweep1', {
    profiles: regions('prof'), path: wireEdges('path'), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
    profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: sphereFace,
  }),
  returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);
batch.add('frame', 'a 4 mm square tube along the same quarter great circle, its profile locked to the sphere', [
  great,
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path'), lockProfile: 'true', profileLockFaces: sphereFace }),
  returns({ faces: `faceProbe(context, ${solidsOf('frame1')})` }),
]);

// ----- a quartic extended -----

const quartic = [
  sketch('s1', xy), `skFitSpline(s1, "spline1", { "points" : [${vec2(0, 0)}, ${vec2(15, 10)}, ${vec2(30, 0)}] });`, solve('s1'),
  call('editCurve', 'curve1', {
    wire: `sketchEntityQuery(id + "s1", EntityType.EDGE, "spline1")`, approximate: 'false', elevate: 'true', elevationDegree: '4', planarize: 'false',
    editControlPoints: 'false', showDetails: 'false', approximationShowDeviation: 'false',
  }),
];
const extend = (shape) =>
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: createdBy('curve1'), flipHeuristics: 'false', helpPoint: vertexAt(createdBy('curve1'), [30, 0, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: `CurveExtensionShape.${shape}`,
  });
const curveProbe = returns({ edges: `wireProbe(context, ${createdBy('curve1')})` });
batch.add('trim-curve', 'a fit spline through (0, 0), (15, 10), (30, 0) raised to degree 4, extended 10 mm at its end, straight on', [quartic, extend('TANGENT'), curveProbe]);
batch.add('trim-curve', 'the same, curving on', [quartic, extend('CURVATURE'), curveProbe]);

// ----- offset curves extended and split -----

const base = solidsOf('base');
const offset = (edges, targets, distance, scope, extra = {}) =>
  call('offsetCurveOnFace', 'offset1', {
    edges, distance: mm(distance), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false', gapFill: 'GapFill.LINEAR',
    scope: `OffsetCurveScope.${scope}`, targets, ...extra,
  });
batch.add('offset-curve-on-face', 'a 40 x 30 x 10 block\'s top front edge offset 5 mm across its top, extended and trimmed 2 mm at each end', [
  box('base', [0, 0, 0], [40, 30, 10]),
  offset(edgeAt(base, [20, 0, 10]), faceAt(base, [20, 15, 10]), 5, 'OFFSET_AND_EXTEND', { trim: 'true', startTrim: mm(2), equalTrim: 'true', endTrim: mm(2) }),
  returns({ edges: `edgeEnds(context, ${createdBy('offset1')})` }),
]);
const rounded = [
  box('base', [0, 0, 0], [40, 30, 10]),
  call('fillet', 'round1', { entities: edgeAt(base, [40, 30, 5]), radius: mm(8) }),
];
// The arc where the round meets the top: about (32, 22, 10), radius 8.
const cornerArc = edgeAt(base, [32 + 8 * Math.SQRT1_2, 22 + 8 * Math.SQRT1_2, 10]);
batch.add('offset-curve-on-face', 'a block\'s top with a rounded corner (radius 8): the round\'s arc offset 3 mm into the top, extended', [
  rounded, offset(cornerArc, faceAt(base, [20, 15, 10]), 3, 'OFFSET_AND_EXTEND', { trim: 'false' }),
  returns({ edges: `edgeEnds(context, ${createdBy('offset1')})` }),
]);
batch.add('offset-curve-on-face', 'the same, extended and splitting the top', [
  rounded, offset(cornerArc, faceAt(base, [20, 15, 10]), 3, 'OFFSET_EXTEND_AND_SPLIT'),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

// ----- a thin loft trimmed to its profiles -----

batch.add('loft', 'a thin loft 1 mm thick between a square and a smaller square, its ends trimmed to the profiles\' planes', [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), rect('s1', 'rect1', [-10, -10], [10, 10]), solve('s1'),
  sketch('s2', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), rect('s2', 'rect1', [-5, -5], [5, 5]), solve('s2'),
  call('loft', 'loft1', {
    bodyType: 'ExtendedToolBodyType.THIN', operationType: 'NewBodyOperationType.NEW',
    wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges('s1')}]) }, { "wireProfileEntities" : qUnion([${wireEdges('s2')}]) }]`,
    midplane: 'false', thickness1: mm(1), flipWall: 'false', thickness2: mm(0), trimEnds: 'true',
  }),
  returns({ faces: `faceProbe(context, ${solidsOf('loft1')})` }),
]);

batch.write('tools/fixtures/gen-batch-076.mjs');
