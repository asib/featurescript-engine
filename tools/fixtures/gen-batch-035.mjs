// Generates fixture batch 035: more options of covered features. Routing curve splines with derivatives at inner
// points (their knots and poles probed, to find how Onshape fits them), a routing polyline through points on a
// curve, a decal realigned to a mate connector, and the cut list of a frame whose tangent segments merge.
//
// Usage: node tools/fixtures/gen-batch-035.mjs
//
// Writes fixtures/batch-035/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, cylinder, deg, edgeAt, line, mm, planeAt, sketch, solidsOf, solve, vertexAt } from './lib.mjs';

const helpers = `
function curveProbe(context is Context, edge is Query) returns map
{
    const c = evCurveDefinition(context, { "edge" : edge });
    if (!(c is BSplineCurve))
        return { "bspline" : false };
    var points = [];
    for (var p in c.controlPoints)
        points = append(points, [roundToPrecision(p[0] / millimeter, 6), roundToPrecision(p[1] / millimeter, 6), roundToPrecision(p[2] / millimeter, 6)]);
    var knots = [];
    for (var k in c.knots)
        knots = append(knots, roundToPrecision(k, 9));
    return { "degree" : c.degree, "rational" : c.isRational, "periodic" : c.isPeriodic, "points" : points, "knots" : knots };
}

function csysNumbers(cs) returns array
{
    return [[roundToPrecision(cs.origin[0] / millimeter, 6), roundToPrecision(cs.origin[1] / millimeter, 6), roundToPrecision(cs.origin[2] / millimeter, 6)],
            [roundToPrecision(cs.xAxis[0], 9), roundToPrecision(cs.xAxis[1], 9), roundToPrecision(cs.xAxis[2], 9)],
            [roundToPrecision(cs.zAxis[0], 9), roundToPrecision(cs.zAxis[1], 9), roundToPrecision(cs.zAxis[2], 9)]];
}

// A face's decals as numbers on one line: the mapping, the uv transform's matrix and translation, and the plane's or
// cylinder's frame (origin in mm, x and z axes).
function decalNumbers(context is Context, face is Query) returns array
{
    return mapArray(getAttribute(context, { "entity" : face, "name" : "decals" }), function(d) {
        const m = d.uvTransform.linear;
        var out = { "mapping" : d.imageMappingType,
                    "uv" : [roundToPrecision(m[0][0], 9), roundToPrecision(m[0][1], 9), roundToPrecision(m[1][0], 9), roundToPrecision(m[1][1], 9),
                            roundToPrecision(d.uvTransform.translation[0], 9), roundToPrecision(d.uvTransform.translation[1], 9)] };
        if (d.planeSystem != undefined)
            out.plane = csysNumbers(d.planeSystem.coordSystem);
        if (d.cylinderSystem != undefined)
            out.cylinder = csysNumbers(d.cylinderSystem.coordSystem);
        return out;
    });
}

function tableText(t is map) returns array
{
    var out = [mapArray(t.columnDefinitions, function(c) { return toString(c.name); })];
    for (var row in t.rows)
        out = append(out, mapArray(t.columnDefinitions, function(c) { return toString(row.columnIdToCell[c.id]); }));
    return out;
}

function caseTables(context is Context, caseId is Id, tables) returns array
{
    var out = [];
    for (var t in tables)
    {
        if (t.entities == undefined || !isQueryEmpty(context, qIntersection([t.entities, qCreatedBy(caseId, EntityType.BODY)])))
            out = append(out, tableText(t));
    }
    return out;
}

const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-5, -5) * millimeter, "secondCorner" : vector(5, 5) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-4, -4) * millimeter, "secondCorner" : vector(4, 4) * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}

function routingPoint(index is number, position is Vector) returns map
{
    return { "index" : index, "referenceType" : ReferenceType.ORIGIN, "vertex" : qNothing(), "curve" : qNothing(),
             "curveParameter" : 0, "alignCSys" : false, "flipCSys" : false, "dx" : position[0], "dy" : position[1],
             "dz" : position[2], "rotationMatrix" : identityMatrix(3), "overrideBendRadius" : false,
             "pointBendRadius" : 0 * meter, "hasDerivative" : false, "derivativeAlignment" : DerivativeAlignment.X,
             "magnitude" : 1, "oppositeDirection" : false, "derivativeDirection" : qNothing(), "arrayAdded" : false };
}

// A routing point with a derivative along one of its frame's axes (the world's, as the frame isn't turned).
function routingDerivativePoint(index is number, position is Vector, alignment is DerivativeAlignment, magnitude is number) returns map
{
    var point = routingPoint(index, position);
    point.hasDerivative = true;
    point.derivativeAlignment = alignment;
    point.magnitude = magnitude;
    return point;
}

// A routing point a fraction along a curve, moved by an offset.
function routingCurvePoint(index is number, curve is Query, parameter is number, offset is Vector) returns map
{
    var point = routingPoint(index, offset);
    point.referenceType = ReferenceType.CURVE;
    point.curve = curve;
    point.curveParameter = parameter;
    return point;
}
`;

const batch = new Batch({ number: 35, title: 'more feature options: routing splines with inner derivatives, routing points on curves, realigned decals, cut lists of merged frames', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const p3 = ([x, y, z]) => `vector(${x}, ${y}, ${z}) * millimeter`;

// ----- routing splines with derivatives -----

const route = (points) =>
  call('routingCurve', 'route1', {
    preselectedEntities: 'qNothing()', curveType: 'RoutingCurveType.INTERPOLATED_SPLINE', step: 'CurveStep.POINTS', inputType: 'InputType.VERTEX',
    inputVertices: 'qNothing()', isPeriodic: 'false', curveBaseCSys: 'qNothing()', pointsEditType: 'PointsEditType.SINGLE', pointIndex: '0',
    addPointOnAxisDrag: 'false', showCSys: 'false', keepPoints: 'false', curveLength: mm(0), bendRadius: mm(5), hasTargetLength: 'false',
    points: `[${points.join(', ')}]`,
  });
const positions = [[0, 0, 0], [20, 10, 0], [40, 0, 10], [60, 10, 10]];
// Derivatives at the points listed: [index, alignment, magnitude].
const withDerivatives = (derivatives) =>
  positions.map((p, i) => {
    const d = derivatives.find(([k]) => k === i);
    return d ? `routingDerivativePoint(${i}, ${p3(p)}, DerivativeAlignment.${d[1]}, ${d[2]})` : `routingPoint(${i}, ${p3(p)})`;
  });
const routeProbe = returns({ curve: `curveProbe(context, qCreatedBy(id + "route1", EntityType.EDGE))` });

batch.add('routing-curve', 'a spline through four points, along x at the second', [route(withDerivatives([[1, 'X', 1]])), routeProbe]);
batch.add('routing-curve', 'a spline through four points, along y at the first and along x at the third', [
  route(withDerivatives([[0, 'Y', 1], [2, 'X', 0.5]])), routeProbe,
]);
batch.add('routing-curve', 'a spline through four points, along x at the second and along z at the third', [
  route(withDerivatives([[1, 'X', 1], [2, 'Z', 1]])), routeProbe,
]);

// Five points spaced unevenly, so that a point's spans either side differ: which of them the derivative's extra knot
// goes into.
const uneven = [[0, 0, 0], [40, 0, 0], [45, 5, 0], [50, 10, 5], [70, 10, 10]];
const unevenRoute = (index) =>
  route(uneven.map((p, i) => (i === index ? `routingDerivativePoint(${i}, ${p3(p)}, DerivativeAlignment.X, 1)` : `routingPoint(${i}, ${p3(p)})`)));
batch.add('routing-curve', 'a spline through five uneven points, along x at the second (its span before longer)', [unevenRoute(1), routeProbe]);
batch.add('routing-curve', 'a spline through five uneven points, along x at the third', [unevenRoute(2), routeProbe]);
batch.add('routing-curve', 'a spline through five uneven points, along x at the fourth (its span after longer)', [unevenRoute(3), routeProbe]);

// Two more to tell apart which span the knot goes into: a middle point whose span before is longer, neither span
// at an end; and the only inner point of three, its span before longer.
const sixPoints = [[0, 0, 0], [10, 0, 0], [40, 0, 5], [45, 5, 5], [55, 10, 5], [70, 10, 10]];
batch.add('routing-curve', 'a spline through six uneven points, along x at the third (its span before longer)', [
  route(sixPoints.map((p, i) => (i === 2 ? `routingDerivativePoint(${i}, ${p3(p)}, DerivativeAlignment.X, 1)` : `routingPoint(${i}, ${p3(p)})`))),
  routeProbe,
]);
const threePoints = [[0, 0, 0], [40, 0, 0], [50, 10, 5]];
batch.add('routing-curve', 'a spline through three points, along y at the second (its span before longer)', [
  route(threePoints.map((p, i) => (i === 1 ? `routingDerivativePoint(${i}, ${p3(p)}, DerivativeAlignment.Y, 1)` : `routingPoint(${i}, ${p3(p)})`))),
  routeProbe,
]);

// ----- routing points on a curve -----

batch.add('routing-curve', 'a polyline through the ends and middle of an arc, the middle raised 5 mm, with 2 mm bends', [
  sketch('guide', xy), arc('guide', 'a1', [0, 0], [14.142135623730951, 5.857864376269049], [20, 20]), solve('guide'),
  call('routingCurve', 'route1', {
    preselectedEntities: 'qNothing()', curveType: 'RoutingCurveType.POLYLINE', step: 'CurveStep.POINTS', inputType: 'InputType.VERTEX',
    inputVertices: 'qNothing()', isPeriodic: 'false', curveBaseCSys: 'qNothing()', pointsEditType: 'PointsEditType.SINGLE', pointIndex: '0',
    addPointOnAxisDrag: 'false', showCSys: 'false', keepPoints: 'false', curveLength: mm(0), bendRadius: mm(2),
    points: `[${[[0, [0, 0, 0]], [0.5, [0, 0, 5]], [1, [0, 0, 0]]].map(([t, o], i) => `routingCurvePoint(${i}, ${wireEdges('guide')}, ${t}, ${p3(o)})`).join(', ')}]`,
  }),
]);

// ----- a realigned decal -----

const image = '{ "imageWidth" : 200, "imageHeight" : 100, "mediaType" : "image/png" } as ImageData';
const top = `qContainsPoint(qOwnedByBody(${base}, EntityType.FACE), vector(20, 15, 12) * millimeter)`;
batch.add('decal', 'an image on a block\'s top, realigned to a mate connector at a corner and the edge along y', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('mateConnector', 'mc1', { originQuery: vertexAt(base, [0, 0, 12]), entityInferenceType: 'EntityInferenceType.POINT', ownerPart: base }),
  call('decal', 'decal1', {
    image, face: top, angle: deg(0), angleOppositeDirection: 'false', uShift: mm(0), vShift: mm(0), realign: 'true',
    originConnector: `qCreatedBy(id + "mc1", EntityType.BODY)`, horizontalReference: edgeAt(base, [0, 15, 12]),
    maintainAspectRatio: 'true', aspectRatioConstraint: 'ImageAspectRatioConstraint.WIDTH_DRIVING', width: mm(20), imageOriginIndex: '4',
  }),
  returns({ decals: `decalNumbers(context, ${top})` }),
]);
// Batch 011's decals again, their probes now on one line (batch 011's printed only the frames' origins).
const decal = (face, fields = {}) =>
  call('decal', 'decal1', {
    image, face, angle: deg(0), angleOppositeDirection: 'false', uShift: mm(0), vShift: mm(0), realign: 'false',
    maintainAspectRatio: 'true', aspectRatioConstraint: 'ImageAspectRatioConstraint.WIDTH_DRIVING', width: mm(20), imageOriginIndex: '4', ...fields,
  });
batch.add('decal', 'an image on the top of a block, turned 30 degrees and shifted (batch 011, c011)', [
  box('base', [0, 0, 0], [40, 30, 12]), decal(top, { angle: deg(30), uShift: mm(3), vShift: mm(-2) }), returns({ decals: `decalNumbers(context, ${top})` }),
]);
const side = `qContainsPoint(qOwnedByBody(${base}, EntityType.FACE), vector(0, -10, 15) * millimeter)`;
batch.add('decal', 'an image wrapped on a cylinder (batch 011)', [
  cylinder('base', [0, 0], 10, 0, 30), decal(side), returns({ decals: `decalNumbers(context, ${side})` }),
]);

// ----- the cut list of a merged frame -----

// A block's top edges along y = 0 and x = 60, joined by the top edge of the upright fillet between them: exactly
// tangent, as a three point sketch arc's ends aren't (a line and an arc so drawn meet 3e-8 radians off, in Onshape
// as in the engine, and the frame mitres them).
const rounded = [
  box('base', [0, 0, -10], [60, 40, 0]),
  call('fillet', 'round1', { entities: edgeAt(base, [60, 0, -5]), radius: mm(10) }),
];
const topEdges = `qUnion([${edgeAt(base, [30, 0, 0])}, ${edgeAt(base, [50 + 10 * Math.SQRT1_2, 10 - 10 * Math.SQRT1_2, 0])}, ${edgeAt(base, [60, 25, 0])}])`;
const mergedFrame = call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: topEdges, mergeTangentSegments: 'true' });
batch.add('cutlist', 'a square tube along two top edges of a block and the rounded corner between, merged into one segment, and its table', [
  rounded, mergedFrame,
  call('cutlist', 'cutlist1', { selectAllFrames: 'false', frames: `qBodyType(qCreatedBy(id + "frame1", EntityType.BODY), BodyType.SOLID)` }),
  returns({ table: 'caseTables(context, id, cutlistTable(context, {}))' }),
]);
batch.add('cutlist', 'the same merged frame, its cut list given only the closed composite part of its segments', [
  rounded, mergedFrame,
  call('cutlist', 'cutlist1', {
    selectAllFrames: 'false', frames: `qCompositePartTypeFilter(qCreatedBy(id + "frame1", EntityType.BODY), CompositePartType.CLOSED)`,
  }),
  returns({ table: 'caseTables(context, id, cutlistTable(context, {}))' }),
]);

batch.write('tools/fixtures/gen-batch-035.mjs');
