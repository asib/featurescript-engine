// Generates fixture batch 200: frames and sweeps locked to other curved faces (frame's untested faces): a cone
// frustum's side, along a quarter circle on it and along a ruling, and a cylinder's rounded top edge (a torus),
// along a quarter circle on it. Each returns its part's faces' types and areas.
//
// Usage: node tools/fixtures/gen-batch-200.mjs
//
// Writes fixtures/batch-200/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, cylinder, edgeAt, edgeOf, faceAt, line, mm, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 200, title: 'frames and sweeps locked to cones and tori', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const createdBy = (name) => `qCreatedBy(id + "${name}", EntityType.BODY)`;
const xy = PLANES.xy;

// ----- locked to a sphere -----

// A sphere of radius 20 about the origin; a quarter of the great circle in the plane through the origin with normal
// (0, sin 30°, cos 30°), from (20, 0, 0) round towards (0, 17.32, -10).
// A cone frustum, radius 10 at z = 0 to 5 at z = 20, revolved about z; a quarter circle of radius 7.5 on it at z = 10.
const cone = [
  sketch('s', PLANES.xz), polygon('s', [[0, 0], [10, 0], [5, 20], [0, 20]]), solve('s'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('s'), axis: edgeOf('axis'), fullRevolve: 'true' }),
];
const coneFace = `qGeometry(qOwnedByBody(${solidsOf('base')}, EntityType.FACE), GeometryType.CONE)`;
const coneArc = [sketch('path', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), arc('path', 'a1', [7.5, 0], [5.303300858899107, 5.303300858899107], [0, 7.5]), solve('path')];
// A cylinder of radius 10 and 10 tall, its top edge rounded to radius 3 (a torus of radii 7 and 3 about z at z = 7);
// a quarter circle on it 45° up the tube, radius 9.121320343559642 at z = 9.121320343559642.
const torus = [
  ...cylinder('base', [0, 0], 10, 0, 10),
  call('fillet', 'fillet1', { entities: edgeAt(solidsOf('base'), [10, 0, 10]), radius: mm(3) }),
];
const torusFace = `qGeometry(qOwnedByBody(${solidsOf('base')}, EntityType.FACE), GeometryType.TORUS)`;
const torusArc = [sketch('path', planeAt([0, 0, 9.121320343559642], [0, 0, 1], [1, 0, 0])), arc('path', 'a1', [9.121320343559642, 0], [6.449747468305832, 6.449747468305832], [0, 9.121320343559642]), solve('path')];
const frame = (faces) => call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path'), lockProfile: 'true', profileLockFaces: faces });
const sweep = (start, faces) => [
  sketch('prof', planeAt(start, [0, 1, 0], [1, 0, 0])), rect('prof', 'r', [0, 0], [2, 3]), solve('prof'),
  call('sweep', 'sweep1', {
    profiles: regions('prof'), path: wireEdges('path'), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
    profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: faces,
  }),
];

batch.add('frame', 'a 4 mm square tube along a quarter circle on a cone frustum\'s side, its profile locked to the cone', [
  cone, coneArc, frame(coneFace), returns({ faces: `faceProbe(context, ${solidsOf('frame1')})` }),
]);
batch.add('sweep', 'a 2 x 3 rectangle swept along the quarter circle on the cone, locked to the cone', [
  cone, coneArc, sweep([7.5, 0, 10], coneFace), returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);
batch.add('frame', 'the square tube along the cone\'s ruling from (10, 0, 0) to (5, 0, 20), locked to the cone', [
  cone, sketch('path', PLANES.xz), line('path', 'l1', [10, 0], [5, 20]), solve('path'), frame(coneFace),
  returns({ faces: `faceProbe(context, ${solidsOf('frame1')})` }),
]);
batch.add('frame', 'the square tube along a quarter circle 45° up a cylinder\'s rounded top edge, locked to the round', [
  torus, torusArc, frame(torusFace), returns({ faces: `faceProbe(context, ${solidsOf('frame1')})` }),
]);
batch.add('sweep', 'the rectangle swept along that quarter circle on the round, locked to it', [
  torus, torusArc, sweep([9.121320343559642, 0, 9.121320343559642], torusFace), returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);

batch.write('tools/fixtures/gen-batch-200.mjs');
