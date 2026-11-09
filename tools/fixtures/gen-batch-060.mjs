// Generates fixture batch 060: derivedMirror (first, so that it mirrors only its own parts: it mirrors every part
// in the Part Studio), where intersection curves of cylinders are split (with the cylinders' coordinate systems,
// whose x axes are their seams), lofts tangent to a rectangle's and a hexagon's planes and from a square off axis,
// and extruding in the flat pattern of a plate with a flange.
//
// Usage: node tools/fixtures/gen-batch-060.mjs
//
// Writes fixtures/batch-060/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

// The part studio derivedMirror mirrors: a 20 x 10 x 10 box above the xy plane and a cylinder beside it.
const mirrorStudio = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    fCuboid(context, makeId("box"), { "corner1" : vector(0, 0, 5) * millimeter, "corner2" : vector(20, 10, 15) * millimeter });
    fCylinder(context, makeId("can"), { "bottomCenter" : vector(30, 5, 5) * millimeter, "topCenter" : vector(30, 5, 25) * millimeter, "radius" : 4 * millimeter });
    return context;
};

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

const batch = new Batch({ number: 60, title: 'derived mirror, intersection curve seams, loft tangency on rectangles and hexagons, extruding in a bent flat', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const createdWires = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE)`;

// ----- derived mirror (must be first) -----

batch.add('derived', 'a box and a cylinder above the xy plane, mirrored across it (derivedMirror)', [
  call('derivedMirror', 'mirror1', { partStudio: '{ "buildFunction" : mirrorStudio, "configuration" : {}, "partQuery" : qBodyType(qEverything(EntityType.BODY), BodyType.SOLID) } as PartStudioData' }),
]);

// ----- intersection curves and seams -----

const crossProbe = (cross) => returns({
  edges: `edgeEnds(context, ${createdWires('curve1')})`, baseFrames: `cylinderFrames(context, ${base})`, crossFrames: `cylinderFrames(context, ${solidsOf(cross)})`,
});
const intersect = (cross) => call('intersectionCurve', 'curve1', {
  group1: faceAt(base, [10, 0, 0]), group2: `qGeometry(qOwnedByBody(${solidsOf(cross)}, EntityType.FACE), GeometryType.CYLINDER)`,
});
batch.add('intersection-curve', 'a cylinder about z crossed by an extruded one about x, symmetric (batch 059 c001), with the seams', [
  cylinder('base', [0, 0], 10, -20, 20),
  sketch('s2', PLANES.yz), circle('s2', 'circle1', [0, 0], 6), solve('s2'),
  call('extrude', 'cross', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), symmetric: 'true' }),
  intersect('cross'), crossProbe('cross'),
]);
batch.add('intersection-curve', 'the same, the cross cylinder extruded one way from x = -20', [
  cylinder('base', [0, 0], 10, -20, 20),
  sketch('s2', planeAt([-20, 0, 0], [1, 0, 0], [0, 1, 0])), circle('s2', 'circle1', [0, 0], 6), solve('s2'),
  call('extrude', 'cross', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(40) }),
  intersect('cross'), crossProbe('cross'),
]);
batch.add('intersection-curve', 'the same, the cross cylinder sketched with its x axis along z', [
  cylinder('base', [0, 0], 10, -20, 20),
  sketch('s2', planeAt([-20, 0, 0], [1, 0, 0], [0, 0, 1])), circle('s2', 'circle1', [0, 0], 6), solve('s2'),
  call('extrude', 'cross', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(40) }),
  intersect('cross'), crossProbe('cross'),
]);
batch.add('intersection-curve', 'a primitive cylinder about z crossed by an extruded one about x', [
  call('fCylinder', 'base', { bottomCenter: vec3(0, 0, -20), topCenter: vec3(0, 0, 20), radius: mm(10) }),
  sketch('s2', PLANES.yz), circle('s2', 'circle1', [0, 0], 6), solve('s2'),
  call('extrude', 'cross', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), symmetric: 'true' }),
  intersect('cross'), crossProbe('cross'),
]);

// ----- lofts tangent to the profile -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const tangentLoft = (bottom, top, magnitude = 1) => [
  sketch('s1', at(0)), bottom('s1'), solve('s1'), sketch('s2', at(20)), top('s2'), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }]`,
    startCondition: 'LoftEndDerivativeType.TANGENT_TO_PROFILE', startMagnitude: String(magnitude),
  }),
  returns({ faces: `faceProbe(context, ${loftBody})` }),
];
const hexagon = (r) => (s) => polygon(s, [0, 1, 2, 3, 4, 5].map((k) => [r * Math.cos((k * Math.PI) / 3), r * Math.sin((k * Math.PI) / 3)]));
batch.add('loft', 'a 30 x 10 rectangle to a 15 x 5 one 20 mm above, tangent to the start profile', tangentLoft((s) => rect(s, 'r', [-15, -5], [15, 5]), (s) => rect(s, 'r', [-7.5, -2.5], [7.5, 2.5])));
batch.add('loft', 'a hexagon of radius 10 to one of radius 5 20 mm above, tangent to the start profile', tangentLoft(hexagon(10), hexagon(5)));
batch.add('loft', 'a 20 mm square to a 10 mm one 20 mm above, tangent to the start profile, magnitude 2', tangentLoft((s) => rect(s, 'r', [-10, -10], [10, 10]), (s) => rect(s, 'r', [-5, -5], [5, 5]), 2));

// ----- extruding in the flat pattern of a plate with a flange -----

const smParts = `qBodyType(qCreatedBy(id + "bracket", EntityType.BODY), BodyType.SOLID)`;
const flatTop = `qCorrespondingInFlat(qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "bracketsm1", EntityType.BODY), BodyType.SOLID), EntityType.FACE), ${vec3(20, 15, 2)}))`;
const bentFlat = (shape, type) => [
  'smBracket(context, id + "bracket");',
  `const f = newSketch(context, id + "f", { "sketchPlane" : ${flatTop} });`, shape, solve('f'),
  `const flatPlane = evPlane(context, { "face" : ${flatTop} });`,
  call('extrude', 'flat1', {
    bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: `FlatOperationType.${type}`,
    endBound: 'BoundingType.BLIND', depth: mm(2),
  }),
  returns({ flatPlane: '{ "origin" : round6(flatPlane.origin), "normal" : flatPlane.normal, "x" : flatPlane.x }' }),
];
batch.add('sheet-metal-flat', 'a plate with a flange, a 6 mm circle at (0, 0) cut in its flat pattern', bentFlat(circle('f', 'circle1', [0, 0], 3), 'REMOVE'));
batch.add('sheet-metal-flat', 'a plate with a flange, a 4 x 30 slot across its bend cut in its flat pattern', bentFlat(rect('f', 'r', [-2, -5], [2, 25]), 'REMOVE'));

batch.write('tools/fixtures/gen-batch-060.mjs');
