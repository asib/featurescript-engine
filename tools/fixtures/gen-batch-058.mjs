// Generates fixture batch 058: a draft whose parting object is a cylindrical sheet, faces replaced by an off-axis
// cylinder and a slanted plane, a sweep locked to a cylinder's side, a tilted torus split at its isoclines, and lofts
// matching the tangency of slanted faces next to the profile. Cases return their faces' surfaces or points along
// their edges.
//
// Usage: node tools/fixtures/gen-batch-058.mjs
//
// Writes fixtures/batch-058/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, circle, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, planeBody, rect, regions, sketch, solidsOf, solve, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 58, title: 'curved draft parting, faces replaced by an off-axis cylinder and a slanted plane, a sweep locked to a cylinder, a tilted torus split, lofts matching slanted faces', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const base = solidsOf('base');
const probe = (body) => returns({ faces: `faceProbe(context, ${body})` });

// ----- a draft parted by a cylindrical sheet -----

batch.add('body-draft', 'a block drafted 3 degrees from a cylindrical sheet through it', [
  box('base', [0, 0, 0], [40, 30, 12]), axisLine('pull', [0, 0, 0], [0, 0, 1]),
  sketch('arcSketch', PLANES.xz), `skArc(arcSketch, "arc1", { "start" : vector(-5, 6) * millimeter, "mid" : vector(20, 9) * millimeter, "end" : vector(45, 6) * millimeter });`, solve('arcSketch'),
  call('extrude', 'part', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('arcSketch'), endBound: 'BoundingType.BLIND', depth: mm(40), oppositeDirection: 'true' }),
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.PARTS', bodies: base, excludeFaces: 'qNothing()', draftOnSelf: 'false', pullDirection: edgeOf('pull'), flipPullDirection: 'false',
    partingObject: `qOwnedByBody(qBodyType(qCreatedBy(id + "part", EntityType.BODY), BodyType.SHEET), EntityType.FACE)`, angle: deg(3), bothSides: 'false',
    cornerType: 'BodyDraftCornerType.EXTEND', keepMaterial: 'false',
  }),
]);

// ----- faces replaced -----

const sheetFaces = (name) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET), EntityType.FACE)`;
batch.add('replace-face', 'a cylinder\'s side replaced by a cylindrical sheet of radius 14 whose axis is 2 mm off along x', [
  cylinder('base', [0, 0], 10, 0, 20),
  sketch('t', PLANES.xy), circle('t', 'circle1', [2, 0], 14), solve('t'),
  call('extrude', 'target', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('t'), endBound: 'BoundingType.BLIND', depth: mm(30) }),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [10, 0, 10]), templateFace: sheetFaces('target'), offset: mm(0) }),
  probe(base),
]);
// A trapezoid prism: its sides lean in.
const trapezoid = [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [25, 10]), line('s1', 'l3', [25, 10], [5, 10]), line('s1', 'l4', [5, 10], [0, 0]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
batch.add('replace-face', 'a trapezoid prism\'s top replaced by a plane rising 2 mm along x over its 30 mm', [
  trapezoid, planeBody('tilt', [15, 10, 11], [-0.06651901052377393, 0, 0.997785157856609], [0.997785157856609, 0, 0.06651901052377393]),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: createdBy('tilt', 'FACE'), offset: mm(0) }),
  probe(base),
]);

// ----- a sweep locked to a cylinder's side -----

batch.add('sweep', 'a 2 × 3 rectangle swept round a cylinder\'s top edge, locked to its side', [
  cylinder('base', [0, 0], 10, 0, 20),
  sketch('prof', planeAt([10, 0, 20], [0, 1, 0], [1, 0, 0])), rect('prof', 'r', [0, 0], [2, 3]), solve('prof'),
  call('sweep', 'sweep1', {
    profiles: regions('prof'), path: edgeAt(base, [10, 0, 20]), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
    profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: faceAt(base, [10, 0, 10]),
  }),
  probe(solidsOf('sweep1')),
]);

// ----- a tilted torus split at its isoclines -----

batch.add('isocline', 'a torus split at its 20 degree isoclines, the direction tilted 30° from its axis', [
  sketch('profile', PLANES.xz), circle('profile', 'c1', [10, 0], 3), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'true' }),
  axisLine('up', [0, 0, 0], [0, 0.5, 0.8660254037844386]),
  call('isocline', 'iso1', { faces: `qOwnedByBody(${base}, EntityType.FACE)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(20), split: 'true' }),
  probe(base),
]);

// ----- lofts matching slanted faces -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
// A frustum: a 20 mm square at z = -10 lofted to a 16 mm one at z = 0; its sides lean in by 2 in 10.
const frustum = [
  sketch('b1', at(-10)), rect('b1', 'r', [-10, -10], [10, 10]), solve('b1'),
  sketch('b2', at(0)), rect('b2', 'r', [-8, -8], [8, 8]), solve('b2'),
  call('loft', 'base', { sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('b1')} }, { "sheetProfileEntities" : ${regions('b2')} }]` }),
];
const top = faceAt(base, [0, 0, 0]);
const sides = `qSubtraction(qOwnedByBody(${base}, EntityType.FACE), qUnion([${top}, ${faceAt(base, [0, 0, -10])}]))`;
const loftUp = (half, magnitude, condition = 'MATCH_TANGENT') => [
  frustum, sketch('s2', at(20)), rect('s2', 'r', [-half, -half], [half, half]), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${top} }, { "sheetProfileEntities" : ${regions('s2')} }]`,
    startCondition: `LoftEndDerivativeType.${condition}`, startMagnitude: String(magnitude), adjacentFacesStart: sides,
  }),
  probe(loftBody),
];
batch.add('loft', 'a frustum\'s top (16 mm) to a 10 mm square 20 mm above, matching the frustum\'s leaning sides, magnitude 1', loftUp(5, 1));
batch.add('loft', 'the same to a 16 mm square, magnitude 1', loftUp(8, 1));
batch.add('loft', 'the same to a 16 mm square, normal to the top, magnitude 1', [
  frustum, sketch('s2', at(20)), rect('s2', 'r', [-8, -8], [8, 8]), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${top} }, { "sheetProfileEntities" : ${regions('s2')} }]`,
    startCondition: 'LoftEndDerivativeType.NORMAL_TO_PROFILE', startMagnitude: '1',
  }),
  probe(loftBody),
]);

batch.write('tools/fixtures/gen-batch-058.mjs');
