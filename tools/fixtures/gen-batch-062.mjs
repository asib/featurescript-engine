// Generates fixture batch 062: boundary surfaces probed as B-splines (batch 005 c019's two arcs, a Coons patch of
// four curves, one end tangent to its profile), a fill of a non-planar loop, extruding in the flat pattern of a
// channel (two bends), a circle on a bend of a flanged plate's flat, a sketch on a flange's face in the flat, and a
// loft through three squares tangent to the first.
//
// Usage: node tools/fixtures/gen-batch-062.mjs
//
// Writes fixtures/batch-062/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, line, mm, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

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

// Each edge of a body: its ends and its length.
function edgeEnds(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, { "start" : round6(lines[0].origin), "middle" : round6(lines[1].origin), "end" : round6(lines[2].origin),
                            "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) });
    }
    return out;
}

// A coordinate system's numbers, in mm, for comparing.
function frameProbe(cs) returns map
{
    return { "origin" : round6(cs.origin), "x" : [roundToPrecision(cs.xAxis[0], 6), roundToPrecision(cs.xAxis[1], 6), roundToPrecision(cs.xAxis[2], 6)],
             "z" : [roundToPrecision(cs.zAxis[0], 6), roundToPrecision(cs.zAxis[1], 6), roundToPrecision(cs.zAxis[2], 6)] };
}

// The coordinate systems of a body's cylindrical faces.
function cylinderFrames(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qGeometry(qOwnedByBody(body, EntityType.FACE), GeometryType.CYLINDER)))
        out = append(out, frameProbe(evSurfaceDefinition(context, { "face" : face }).coordSystem));
    return out;
}

// A sheet metal plate 40 x 30, 2 mm thick, with a 10 mm flange up from its y = 30 edge.
function smBracket(context is Context, id is Id)
{
    const sketch = newSketchOnPlane(context, id + "s1", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(sketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });
    skSolve(sketch);
    sheetMetalStart(context, id + "sm1", { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "s1"),
                                   "thickness" : 2 * millimeter, "oppositeDirection" : false, "radius" : 1 * millimeter,
                                   "flipDirectionUp" : false, "tangentPropagation" : false, "bends" : qNothing() });
    const part = qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID);
    sheetMetalFlange(context, id + "flange1", {
            "edges" : qClosestTo(qOwnedByBody(part, EntityType.EDGE), vector(20, 30, 2) * millimeter),
            "flangeAlignment" : SMFlangeAlignment.INNER, "limitType" : SMFlangeBoundingType.BLIND, "distance" : 10 * millimeter,
            "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE, "oppositeDirection" : false, "bendAngle" : 90 * degree,
            "autoMiter" : true, "useDefaultRadius" : true });
}

// A sheet metal plate 40 x 30, 2 mm thick, with 10 mm flanges up from both its y = 0 and y = 30 edges (a channel).
function smChannel(context is Context, id is Id)
{
    const sketch = newSketchOnPlane(context, id + "s1", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(sketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });
    skSolve(sketch);
    sheetMetalStart(context, id + "sm1", { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "s1"),
                                   "thickness" : 2 * millimeter, "oppositeDirection" : false, "radius" : 1 * millimeter,
                                   "flipDirectionUp" : false, "tangentPropagation" : false, "bends" : qNothing() });
    const part = qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID);
    sheetMetalFlange(context, id + "flange1", {
            "edges" : qUnion([qClosestTo(qOwnedByBody(part, EntityType.EDGE), vector(20, 30, 2) * millimeter),
                              qClosestTo(qOwnedByBody(part, EntityType.EDGE), vector(20, 0, 2) * millimeter)]),
            "flangeAlignment" : SMFlangeAlignment.INNER, "limitType" : SMFlangeBoundingType.BLIND, "distance" : 10 * millimeter,
            "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE, "oppositeDirection" : false, "bendAngle" : 90 * degree,
            "autoMiter" : true, "useDefaultRadius" : true });
}
`;

const batch = new Batch({ number: 62, title: 'boundary surfaces as B-splines, a non-planar fill, flats of a channel and across a bend, a three-profile tangent loft', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wires = (name) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;

// ----- boundary surfaces -----

const arcs = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('s1', 'arc1', [0, 0], [15, 6], [30, 0]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), arc('s2', 'arc1', [0, 0], [15, 10], [30, 0]), solve('s2'),
];
const profile = (uv, name, condition = 'DEFAULT', extra = {}) =>
  `{ "${uv}ProfileEntities" : ${wires(name)}, "${uv}Condition" : BSurfEndDerivativeType.${condition}${Object.entries(extra).map(([k, v]) => `, "${uv}${k}" : ${v}`).join('')} }`;
batch.add('boundary-surface', 'two arcs 20 mm apart as u profiles (batch 005 c019)', [
  arcs, call('boundarySurface', 'surface1', { uProfilesArray: `[${profile('u', 's1')}, ${profile('u', 's2')}]`, vProfilesArray: '[]' }),
  returns({ faces: `faceProbe(context, ${sheetsOf('surface1')})` }),
]);
batch.add('boundary-surface', 'the two arcs and the two lines joining their ends: u and v profiles', [
  arcs,
  sketch('s3', PLANES.xy), line('s3', 'l1', [0, 0], [0, 20]), solve('s3'),
  sketch('s4', PLANES.xy), line('s4', 'l1', [30, 0], [30, 20]), solve('s4'),
  call('boundarySurface', 'surface1', {
    uProfilesArray: `[${profile('u', 's1')}, ${profile('u', 's2')}]`, vProfilesArray: `[${profile('v', 's3')}, ${profile('v', 's4')}]`,
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('surface1')})` }),
]);
batch.add('boundary-surface', 'the two arcs, the first tangent to its profile\'s plane, magnitude 1', [
  arcs, call('boundarySurface', 'surface1', { uProfilesArray: `[${profile('u', 's1', 'TANGENT_TO_PROFILE', { Magnitude: '1' })}, ${profile('u', 's2')}]`, vProfilesArray: '[]' }),
  returns({ faces: `faceProbe(context, ${sheetsOf('surface1')})` }),
]);

// ----- a fill of a non-planar loop -----

batch.add('fill', 'a saddle: four lines between (0, 0, 0), (20, 0, 5), (20, 20, 0), (0, 20, 5), each drawn on an upright plane', [
  sketch('e1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('e1', 'l1', [0, 0], [20, 5]), solve('e1'),
  sketch('e2', planeAt([20, 0, 0], [1, 0, 0], [0, 1, 0])), line('e2', 'l1', [0, 5], [20, 0]), solve('e2'),
  sketch('e3', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), line('e3', 'l1', [20, 0], [0, 5]), solve('e3'),
  sketch('e4', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), line('e4', 'l1', [20, 5], [0, 0]), solve('e4'),
  call('fill', 'fill1', {
    edges: `[{ "entities" : qUnion([${['e1', 'e2', 'e3', 'e4'].map(wires).join(', ')}]), "continuity" : GeometricContinuity.G0 }]`,
    surfaceOperationType: 'NewSurfaceOperationType.NEW',
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('fill1')})` }),
]);

// ----- extruding in flat patterns -----

const inFlat = (part, point, shape, type, extra = []) => {
  const flatFace = `qCorrespondingInFlat(qContainsPoint(qOwnedByBody(${part}, EntityType.FACE), ${vec3(...point)}))`;
  return [
    `const flatPlane = evPlane(context, { "face" : ${flatFace} });`,
    `const f = newSketch(context, id + "f", { "sketchPlane" : ${flatFace} });`, shape, solve('f'),
    call('extrude', 'flat1', {
      bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: `FlatOperationType.${type}`,
      endBound: 'BoundingType.BLIND', depth: mm(2),
    }),
    returns({ flatPlane: '{ "origin" : round6(flatPlane.origin), "normal" : flatPlane.normal, "x" : flatPlane.x }', faces: `faceProbe(context, ${part})` }),
  ];
};
const channelPart = `qBodyType(qCreatedBy(id + "channel" + "sm1", EntityType.BODY), BodyType.SOLID)`;
const bracketPart = `qBodyType(qCreatedBy(id + "bracket" + "sm1", EntityType.BODY), BodyType.SOLID)`;
batch.add('sheet-metal-flat', 'a channel (flanges on both long edges), a 4 x 50 slot across both bends cut in its flat pattern', [
  'smChannel(context, id + "channel");', inFlat(channelPart, [20, 15, 2], rect('f', 'r', [-2, -25], [2, 25]), 'REMOVE'),
]);
batch.add('sheet-metal-flat', 'a plate with a flange, a 6 mm circle centred on the bend cut in its flat pattern', [
  'smBracket(context, id + "bracket");', inFlat(bracketPart, [20, 15, 2], circle('f', 'circle1', [0, 16], 3), 'REMOVE'),
]);
batch.add('sheet-metal-flat', 'a plate with a flange, a 4 mm circle cut in the flat of the flange\'s outer face', [
  'smBracket(context, id + "bracket");', inFlat(bracketPart, [20, 32, 6], circle('f', 'circle1', [0, 0], 2), 'REMOVE'),
]);

// ----- a loft through three polygons tangent to the first -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
batch.add('loft', 'squares of 20, 14 and 10 mm at 0, 10 and 25 mm, tangent to the first profile', [
  sketch('s1', at(0)), rect('s1', 'r', [-10, -10], [10, 10]), solve('s1'),
  sketch('s2', at(10)), rect('s2', 'r', [-7, -7], [7, 7]), solve('s2'),
  sketch('s3', at(25)), rect('s3', 'r', [-5, -5], [5, 5]), solve('s3'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }, { "sheetProfileEntities" : ${regions('s3')} }]`,
    startCondition: 'LoftEndDerivativeType.TANGENT_TO_PROFILE', startMagnitude: '1',
  }),
  returns({ faces: `faceProbe(context, ${solidsOf('loft1')})` }),
]);

batch.write('tools/fixtures/gen-batch-062.mjs');
