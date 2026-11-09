// Generates fixture batch 063: curvature fillets from magnitude 0.98 to 1, polygon lofts normal to a profile, a
// fill of four arcs round a non-planar loop, boundary surfaces with arcs both ways and tangent to a profile.
//
// Usage: node tools/fixtures/gen-batch-063.mjs
//
// Writes fixtures/batch-063/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, edgeAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 63, title: 'curvature fillets 0.98 to 1, polygon lofts normal to a profile, a fill of arcs, boundary surfaces of arcs', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const wires = (name) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;

// ----- curvature fillets near magnitude 1 -----

for (const magnitude of [0.98, 0.985, 0.995, 1]) {
  batch.add('fillet', `a block's upright edge, curvature, 4 mm, magnitude ${magnitude}`, [
    box('base', [0, 0, 0], [30, 20, 10]),
    call('fillet', 'fillet1', { entities: edgeAt(base, [0, 0, 5]), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(4), magnitude: String(magnitude) }),
    returns({ faces: `faceProbe(context, ${base})` }),
  ]);
}

// ----- polygon lofts normal to a profile -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loft2 = (bottom, top, conditions) => [
  sketch('s1', at(0)), bottom('s1'), solve('s1'), sketch('s2', at(20)), top('s2'), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }]`,
    ...conditions,
  }),
  returns({ faces: `faceProbe(context, ${solidsOf('loft1')})` }),
];
const normal = (m) => ({ startCondition: 'LoftEndDerivativeType.NORMAL_TO_PROFILE', startMagnitude: String(m) });
batch.add('loft', 'a 30 x 10 rectangle to a 15 x 5 one 20 mm above, normal to the start profile', loft2((s) => rect(s, 'r', [-15, -5], [15, 5]), (s) => rect(s, 'r', [-7.5, -2.5], [7.5, 2.5]), normal(1)));
batch.add('loft', 'a 20 mm square to a 10 mm one centred at (4, 3) 20 mm above, normal to the start profile', loft2((s) => rect(s, 'r', [-10, -10], [10, 10]), (s) => rect(s, 'r', [-1, -2], [9, 8]), normal(1)));
batch.add('loft', 'the rectangles, normal to the start profile, magnitude 2', loft2((s) => rect(s, 'r', [-15, -5], [15, 5]), (s) => rect(s, 'r', [-7.5, -2.5], [7.5, 2.5]), normal(2)));

// ----- a fill of four arcs -----

batch.add('fill', 'four arcs round a non-planar loop: (0, 0, 0), (20, 0, 5), (20, 20, 0), (0, 20, 5), each bowed 2 mm up on an upright plane', [
  sketch('e1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('e1', 'a1', [0, 0], [10, 4.5], [20, 5]), solve('e1'),
  sketch('e2', planeAt([20, 0, 0], [1, 0, 0], [0, 1, 0])), arc('e2', 'a1', [0, 5], [10, 4.5], [20, 0]), solve('e2'),
  sketch('e3', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), arc('e3', 'a1', [20, 0], [10, 4.5], [0, 5]), solve('e3'),
  sketch('e4', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), arc('e4', 'a1', [20, 5], [10, 4.5], [0, 0]), solve('e4'),
  call('fill', 'fill1', {
    edges: `[{ "entities" : qUnion([${['e1', 'e2', 'e3', 'e4'].map(wires).join(', ')}]), "continuity" : GeometricContinuity.G0 }]`,
    surfaceOperationType: 'NewSurfaceOperationType.NEW',
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('fill1')})` }),
]);

// ----- boundary surfaces -----

const arcs = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('s1', 'arc1', [0, 0], [15, 6], [30, 0]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), arc('s2', 'arc1', [0, 0], [15, 10], [30, 0]), solve('s2'),
];
const profile = (uv, name, condition = 'DEFAULT', extra = {}) =>
  `{ "${uv}ProfileEntities" : ${wires(name)}, "${uv}Condition" : BSurfEndDerivativeType.${condition}${Object.entries(extra).map(([k, v]) => `, "${uv}${k}" : ${v}`).join('')} }`;
batch.add('boundary-surface', 'the arcs of batch 062 c001 and arcs bowed 3 mm down joining their ends', [
  arcs,
  sketch('s3', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), arc('s3', 'a1', [0, 0], [10, -3], [20, 0]), solve('s3'),
  sketch('s4', planeAt([30, 0, 0], [1, 0, 0], [0, 1, 0])), arc('s4', 'a1', [0, 0], [10, -3], [20, 0]), solve('s4'),
  call('boundarySurface', 'surface1', {
    uProfilesArray: `[${profile('u', 's1')}, ${profile('u', 's2')}]`, vProfilesArray: `[${profile('v', 's3')}, ${profile('v', 's4')}]`,
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('surface1')})` }),
]);
batch.add('boundary-surface', 'the arcs of batch 062 c001, the first tangent to its profile\'s plane, magnitude 1', [
  arcs, call('boundarySurface', 'surface1', {
    uProfilesArray: `[${profile('u', 's1', 'TANGENT_TO_PROFILE', { Magnitude: '1', Reference: 'qNothing()', Direction: 'qNothing()' })}, ${profile('u', 's2')}]`, vProfilesArray: '[]',
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('surface1')})` }),
]);
batch.add('boundary-surface', 'the arcs of batch 062 c001, the first normal to its profile\'s plane, magnitude 1', [
  arcs, call('boundarySurface', 'surface1', {
    uProfilesArray: `[${profile('u', 's1', 'NORMAL_TO_PROFILE', { Magnitude: '1', Reference: 'qNothing()', Direction: 'qNothing()' })}, ${profile('u', 's2')}]`, vProfilesArray: '[]',
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('surface1')})` }),
]);

batch.write('tools/fixtures/gen-batch-063.mjs');
