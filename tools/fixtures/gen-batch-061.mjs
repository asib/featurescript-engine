// Generates fixture batch 061: extruding in the flat pattern of a plate with a flange (batch 060's cases with the
// right feature id), extruded cylinders about y and about a tilted axis (their coordinate systems), crossing
// cylinders' intersection curves with the groups swapped, and polygon lofts tangent to the end profile, to both, from
// a square to one off its axis, and between triangles.
//
// Usage: node tools/fixtures/gen-batch-061.mjs
//
// Writes fixtures/batch-061/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, cylinder, faceAt, mm, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

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
`;

const batch = new Batch({ number: 61, title: 'extruding in a bent flat, cylinder frames about y and tilted, swapped intersection groups, more polygon tangent lofts', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const createdWires = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE)`;

// ----- extruding in the flat pattern of a plate with a flange -----

const bracketPart = `qBodyType(qCreatedBy(id + "bracket" + "sm1", EntityType.BODY), BodyType.SOLID)`;
const flatTop = `qCorrespondingInFlat(qContainsPoint(qOwnedByBody(${bracketPart}, EntityType.FACE), ${vec3(20, 15, 2)}))`;
const bentFlat = (shape, type) => [
  'smBracket(context, id + "bracket");',
  `const flatPlane = evPlane(context, { "face" : ${flatTop} });`,
  `const f = newSketch(context, id + "f", { "sketchPlane" : ${flatTop} });`, shape, solve('f'),
  call('extrude', 'flat1', {
    bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: `FlatOperationType.${type}`,
    endBound: 'BoundingType.BLIND', depth: mm(2),
  }),
  returns({ flatPlane: '{ "origin" : round6(flatPlane.origin), "normal" : flatPlane.normal, "x" : flatPlane.x }', faces: `faceProbe(context, ${bracketPart})` }),
];
batch.add('sheet-metal-flat', 'a plate with a 10 mm flange, a 6 mm circle at (0, 0) of its flat sketch cut in its flat pattern', bentFlat(circle('f', 'circle1', [0, 0], 3), 'REMOVE'));
batch.add('sheet-metal-flat', 'a plate with a 10 mm flange, a 4 x 30 slot across its bend cut in its flat pattern', bentFlat(rect('f', 'r', [-2, -5], [2, 25]), 'REMOVE'));
batch.add('sheet-metal-flat', 'a plate with a 10 mm flange, a 10 x 6 tab added at the flange\'s far edge in its flat pattern', bentFlat(rect('f', 'r', [-5, 20], [5, 40]), 'ADD'));

// ----- extruded cylinders' coordinate systems -----

batch.add('cylinder-frame', 'a cylinder of radius 5 extruded 20 mm along y, sketched on the front plane, and one along -y', [
  sketch('s1', planeAt([0, 0, 0], [0, 1, 0], [1, 0, 0])), circle('s1', 'c1', [0, 0], 5), solve('s1'),
  call('extrude', 'up', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(20) }),
  sketch('s2', planeAt([20, 0, 0], [0, -1, 0], [0, 0, 1])), circle('s2', 'c2', [0, 0], 5), solve('s2'),
  call('extrude', 'down', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(20) }),
  returns({ up: `cylinderFrames(context, ${solidsOf('up')})`, down: `cylinderFrames(context, ${solidsOf('down')})` }),
]);
batch.add('cylinder-frame', 'cylinders of radius 5 extruded 20 mm along (1, 1, 1) and along (1, 0, 1), sketched with their x along (1, -1, 0) and y', [
  sketch('s1', planeAt([0, 0, 0], [1, 1, 1], [1, -1, 0])), circle('s1', 'c1', [0, 0], 5), solve('s1'),
  call('extrude', 'a', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(20) }),
  sketch('s2', planeAt([40, 0, 0], [1, 0, 1], [0, 1, 0])), circle('s2', 'c2', [0, 0], 5), solve('s2'),
  call('extrude', 'b', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(20) }),
  returns({ a: `cylinderFrames(context, ${solidsOf('a')})`, b: `cylinderFrames(context, ${solidsOf('b')})` }),
]);

// ----- intersection curves -----

const crossCylinder = (depth, symmetric) => [
  cylinder('base', [0, 0], 10, -20, 20),
  symmetric ? sketch('s2', PLANES.yz) : sketch('s2', planeAt([-20, 0, 0], [1, 0, 0], [0, 1, 0])),
  circle('s2', 'circle1', [0, 0], 6), solve('s2'),
  call('extrude', 'cross', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(depth), ...(symmetric ? { symmetric: 'true' } : {}) }),
];
const swapped = call('intersectionCurve', 'curve1', {
  group1: `qGeometry(qOwnedByBody(${solidsOf('cross')}, EntityType.FACE), GeometryType.CYLINDER)`, group2: faceAt(base, [10, 0, 0]),
});
batch.add('intersection-curve', 'batch 060 c002 (the cross cylinder ending on the base) with the groups swapped', [
  crossCylinder(20, true), swapped, returns({ edges: `edgeEnds(context, ${createdWires('curve1')})` }),
]);
batch.add('intersection-curve', 'batch 060 c003 (the cross cylinder 40 mm long) with the groups swapped', [
  crossCylinder(40, false), swapped, returns({ edges: `edgeEnds(context, ${createdWires('curve1')})` }),
]);

// ----- polygon lofts tangent to a profile -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const tangentLoft = (bottom, top, conditions) => [
  sketch('s1', at(0)), bottom('s1'), solve('s1'), sketch('s2', at(20)), top('s2'), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }]`,
    ...conditions,
  }),
  returns({ faces: `faceProbe(context, ${loftBody})` }),
];
const start = { startCondition: 'LoftEndDerivativeType.TANGENT_TO_PROFILE', startMagnitude: '1' };
const end = { endCondition: 'LoftEndDerivativeType.TANGENT_TO_PROFILE', endMagnitude: '1' };
const square = (c, h) => (s) => rect(s, 'r', [c[0] - h, c[1] - h], [c[0] + h, c[1] + h]);
batch.add('loft', 'a 30 x 10 rectangle to a 15 x 5 one 20 mm above, tangent to the end profile', tangentLoft((s) => rect(s, 'r', [-15, -5], [15, 5]), (s) => rect(s, 'r', [-7.5, -2.5], [7.5, 2.5]), end));
batch.add('loft', 'the same, tangent to both profiles', tangentLoft((s) => rect(s, 'r', [-15, -5], [15, 5]), (s) => rect(s, 'r', [-7.5, -2.5], [7.5, 2.5]), { ...start, ...end }));
batch.add('loft', 'a 20 mm square to a 10 mm one centred at (4, 3) 20 mm above, tangent to the start profile', tangentLoft(square([0, 0], 10), square([4, 3], 5), start));
batch.add('loft', 'a triangle to a smaller turned one 20 mm above, tangent to the start profile, magnitude 0.5', tangentLoft(
  (s) => polygon(s, [[10, 0], [-5, 8.660254], [-5, -8.660254]]), (s) => polygon(s, [[4, 3], [-5, 4], [0, -5]]),
  { startCondition: 'LoftEndDerivativeType.TANGENT_TO_PROFILE', startMagnitude: '0.5' }));

batch.write('tools/fixtures/gen-batch-061.mjs');
