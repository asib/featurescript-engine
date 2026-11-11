// Generates fixture batch 112: Onshape's own surfaces and curves where the engine's differ, probed for their
// B-spline poles and knots, to find how Onshape builds them. Batch 075 c005's loft with a guide bulging out from the
// middle of one side (and a smaller bulge, and one bowing in); batch 075 c006's circles lofted along a straight
// path through 3, 5 and 9 sections; batch 055's sphere band extended round it 2 and 5 mm, its new edge's curve;
// batch 080 c004's hole cut across a rolled wall, its wall's faces.
//
// Usage: node tools/fixtures/gen-batch-112.mjs
//
// Writes fixtures/batch-112/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, axisLine, call, circle, edgeAt, edgeOf, mm, planeAt, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Knot arrays, weights and control point matrices are typed: indexed, as std does, rather than iterated.
function numbersProbe(numbers) returns array
{
    var out = [];
    for (var i = 0; i < size(numbers); i += 1)
        out = append(out, roundToPrecision(numbers[i], 9));
    return out;
}

function bsplineProbe(s) returns map
{
    var b = { "uDegree" : s.uDegree, "vDegree" : s.vDegree, "rational" : s.isRational };
    try
    {
        b.uKnots = numbersProbe(s.uKnots);
        b.vKnots = numbersProbe(s.vKnots);
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

// Each face of a body: its area, its surface's type, and for a B-spline its degrees, knots and poles.
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

// Each edge of a body that isn't straight: its ends and middle, and for a B-spline its degree, knots, poles and
// weights.
function curveProbe(context is Context, body is Query) returns array
{
    var out = [];
    const edges = qOwnedByBody(body, EntityType.EDGE);
    for (var edge in evaluateQuery(context, qSubtraction(edges, qGeometry(edges, GeometryType.LINE))))
    {
        var entry = {};
        try
        {
            const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
            var points = [];
            for (var l in lines)
                points = append(points, round6(l.origin));
            entry.points = points;
        }
        catch (error)
        {
            entry.points = "failed";
        }
        try
        {
            const c = evCurveDefinition(context, { "edge" : edge });
            if (c is BSplineCurve)
            {
                entry.degree = c.degree;
                entry.rational = c.isRational;
                entry.knots = numbersProbe(c.knots);
                var poles = [];
                for (var i = 0; i < size(c.controlPoints); i += 1)
                    poles = append(poles, round6(c.controlPoints[i]));
                entry.poles = poles;
                if (c.isRational)
                    entry.weights = numbersProbe(c.weights);
            }
        }
        catch (error)
        {
            entry.curve = "failed";
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 112, title: "Onshape's guided and pathed lofts, sphere extensions and rolled-wall holes, probed for their poles", helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const profiles = (names) => `[${names.map((n) => `{ "sheetProfileEntities" : ${regions(n)} }`).join(', ')}]`;
const loft = (fields) => call('loft', 'loft1', { sheetProfilesArray: profiles(['s1', 's2']), ...fields });
const loftProbe = returns({ faces: `faceProbe(context, ${loftBody})`, curves: `curveProbe(context, ${loftBody})` });

// ----- batch 075 c005: a guide bulging from the middle of a side -----

const square = (s) => [sketch(s, at(0)), rect(s, 'rect1', [-10, -10], [10, 10]), solve(s)];
const small = (s) => [sketch(s, at(20)), rect(s, 'rect1', [-5, -5], [5, 5]), solve(s)];
const guided = {
  addGuides: 'true',
  guidesArray: '[{ "guideEntities" : qUnion([qCreatedBy(id + "g0", EntityType.EDGE)]), "guideDerivativeType" : LoftGuideDerivativeType.DEFAULT, "guideDerivativeMagnitude" : 1 }]',
};
for (const [x, what] of [[13, 'bulging out, through (13, 0, 10) (batch 075 c005)'], [11, 'bulging out less, through (11, 0, 10)'], [6.5, 'bowing in, through (6.5, 0, 10)']]) {
  batch.add('loft', `a 20 mm square to a 10 mm one 20 mm above, one guide from (10, 0, 0) to (5, 0, 20) ${what}`, [
    square('s1'), small('s2'),
    sketch('g0', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('g0', 'a1', [10, 0], [x, 10], [5, 20]), solve('g0'),
    loft(guided), loftProbe,
  ]);
}

// ----- batch 075 c006: circles lofted along a straight path -----

for (const n of [3, 5, 9]) {
  batch.add('loft', `circles of radius 10 at z = 0 and 5 at z = 30, lofted along the straight line between their centres (${n} sections)`, [
    sketch('s1', at(0)), circle('s1', 'c1', [0, 0], 10), solve('s1'), sketch('s2', at(30)), circle('s2', 'c1', [0, 0], 5), solve('s2'),
    axisLine('path', [0, 0, 0], [0, 0, 30]),
    loft({ addSections: 'true', spine: edgeOf('path'), sectionCount: `${n}` }),
    loftProbe,
  ]);
}

// ----- batch 055: a sphere's band extended round it -----

const sheet = sheetsOf('base');
const sphere = [
  sketch('profile', PLANES.xz), arc('profile', 'arc1', [10, 0], [8.660254037844387, 5], [5, 8.660254037844386]), solve('profile'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '90 * degree',
  }),
];
for (const d of [2, 5]) {
  batch.add('extend-surface', `batch 055's band of a sphere (radius 10, latitude 0° to 60°, 90° round) extended ${d} mm round it at its edge in the xz plane`, [
    sphere,
    call('extendSurface', 'extend1', {
      entities: edgeAt(sheet, [8.660254037844387, 0, 5]), endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(d), oppositeDirection: 'false',
      maintainCurvature: 'false', tangentPropagation: 'false',
    }),
    returns({ faces: `faceProbe(context, ${sheet})`, curves: `curveProbe(context, ${sheet})` }),
  ]);
}

// ----- batch 080 c004: a hole cut across a rolled wall -----

const smParts = solidsOf('sm1');
batch.add('sheet-metal', "batch 080 c004: batch 069's half tube (radius 20, 2 mm thick, 25 long) cut through its top by an 8 mm circle extruded down from y = 30", [
  sketch('s1', PLANES.xy), arc('s1', 'a1', [20, 0], [0, 20], [-20, 0]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  sketch('c1', planeAt([0, 30, 0], [0, 1, 0], [1, 0, 0])), circle('c1', 'circle1', [0, -12.5], 4), solve('c1'),
  call('extrude', 'cut1', {
    bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.REMOVE', entities: regions('c1'), endBound: 'BoundingType.BLIND',
    depth: mm(15), oppositeDirection: 'true', booleanScope: smParts,
  }),
  returns({ faces: `faceProbe(context, ${smParts})`, curves: `curveProbe(context, ${smParts})` }),
]);

batch.write('tools/fixtures/gen-batch-112.mjs');
