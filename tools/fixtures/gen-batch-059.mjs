// Generates fixture batch 059: where Onshape splits closed intersection curves (batch 005 c006, with each edge's
// ends), offset curves on faces in the other direction (batch 020 c008's rule) and across a fillet's cylinder,
// extruding in a sheet metal flat pattern (`SMFlatOp`, through `qCorrespondingInFlat`), and curvature fillets of
// magnitude 0.87, 0.92 and 0.97.
//
// Usage: node tools/fixtures/gen-batch-059.mjs
//
// Writes fixtures/batch-059/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, cylinder, edgeAt, faceAt, mm, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

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
`;

const batch = new Batch({ number: 59, title: 'intersection curve edges, offset curve directions, extruding in a sheet metal flat, curvature fillets 0.87 to 0.97', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const createdWires = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE)`;
const wireEnds = (name) => returns({ edges: `edgeEnds(context, ${createdWires(name)})` });

// ----- intersection curves of two crossing cylinders -----

batch.add('intersection-curve', 'two crossing cylinders (batch 005 c006), with each edge\'s ends', [
  cylinder('base', [0, 0], 10, -20, 20),
  sketch('s2', PLANES.yz), circle('s2', 'circle1', [0, 0], 6), solve('s2'),
  call('extrude', 'cross', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), symmetric: 'true' }),
  call('intersectionCurve', 'curve1', {
    group1: faceAt(base, [10, 0, 0]), group2: `qGeometry(qOwnedByBody(${solidsOf('cross')}, EntityType.FACE), GeometryType.CYLINDER)`,
  }),
  wireEnds('curve1'),
]);
batch.add('intersection-curve', 'the same, the cross cylinder a primitive along x', [
  cylinder('base', [0, 0], 10, -20, 20),
  call('fCylinder', 'cross', { bottomCenter: vec3(-20, 0, 0), topCenter: vec3(20, 0, 0), radius: mm(6) }),
  call('intersectionCurve', 'curve1', {
    group1: faceAt(base, [10, 0, 0]), group2: `qGeometry(qOwnedByBody(${solidsOf('cross')}, EntityType.FACE), GeometryType.CYLINDER)`,
  }),
  wireEnds('curve1'),
]);

// ----- offset curves on faces -----

const offset = (edges, distance, opposite, targets) =>
  call('offsetCurveOnFace', 'offset1', {
    edges, distance: mm(distance), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: opposite ? 'true' : 'false',
    gapFill: 'GapFill.LINEAR', scope: 'OffsetCurveScope.OFFSET', targets,
  });
batch.add('offset-curve-on-face', 'a cylinder\'s top edge offset 2 mm across its top, the other way (batch 020 c008 flipped)', [
  cylinder('base', [0, 0], 10, 0, 30), offset(edgeAt(base, [10, 0, 30]), 2, true, faceAt(base, [0, 0, 30])), wireEnds('offset1'),
]);
batch.add('offset-curve-on-face', 'a block\'s top edge along x offset 3 mm across its top, the other way (batch 005 c009 flipped)', [
  box('base', [0, 0, 0], [30, 20, 10]), offset(edgeAt(base, [15, 0, 10]), 3, true, faceAt(base, [15, 10, 10])), wireEnds('offset1'),
]);
batch.add('offset-curve-on-face', 'a block\'s top edge along x offset 3 mm across its top (batch 005 c009), with the curve\'s ends', [
  box('base', [0, 0, 0], [30, 20, 10]), offset(edgeAt(base, [15, 0, 10]), 3, false, faceAt(base, [15, 10, 10])), wireEnds('offset1'),
]);
// A block with its top edge along x at y = 0 rounded 5 mm: the fillet meets the top along y = 5.
const filleted = [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [15, 0, 10]), radius: mm(5) }),
];
const filletFace = `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CYLINDER)`;
batch.add('offset-curve-on-face', 'the edge where a 5 mm fillet meets a block\'s top offset 2 mm across the fillet', [
  filleted, offset(edgeAt(base, [15, 5, 10]), 2, false, filletFace), wireEnds('offset1'),
]);
batch.add('offset-curve-on-face', 'the same, the other way', [filleted, offset(edgeAt(base, [15, 5, 10]), 2, true, filletFace), wireEnds('offset1')]);

// ----- extruding in a sheet metal flat pattern -----

const plate = [
  sketch('s1', PLANES.xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
const smParts = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const flatTop = `qCorrespondingInFlat(qContainsPoint(qOwnedByBody(${smParts}, EntityType.FACE), ${vec3(20, 15, 2)}))`;
const flatSketch = (body) => [`const f = newSketch(context, id + "f", { "sketchPlane" : ${flatTop} });`, body, solve('f')];
const inFlat = (type) =>
  call('extrude', 'flat1', {
    bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: `FlatOperationType.${type}`,
    endBound: 'BoundingType.BLIND', depth: mm(2),
  });
batch.add('sheet-metal-flat', 'a plate with a 10 mm circle cut out in its flat pattern', [
  plate, flatSketch(circle('f', 'circle1', [20, 15], 5)), inFlat('REMOVE'),
]);
batch.add('sheet-metal-flat', 'a plate with a 10 × 10 tab added past its edge in its flat pattern', [
  plate, flatSketch(rect('f', 'rect1', [40, 10], [50, 20])), inFlat('ADD'),
]);

// ----- curvature fillets past magnitude 0.86 -----

for (const magnitude of [0.87, 0.92, 0.97]) {
  batch.add('fillet', `a block's upright edge, curvature, 4 mm, magnitude ${magnitude}`, [
    box('base', [0, 0, 0], [30, 20, 10]),
    call('fillet', 'fillet1', { entities: edgeAt(base, [0, 0, 5]), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(4), magnitude: String(magnitude) }),
    returns({ faces: `faceProbe(context, ${base})` }),
  ]);
}

batch.write('tools/fixtures/gen-batch-059.mjs');
