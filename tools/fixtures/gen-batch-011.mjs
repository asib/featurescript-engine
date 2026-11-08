// Generates fixture batch 011: edit curve, path deviation, routing curves and their tables, computed mass, decals,
// form tags, cut lists and hole tables.
//
// Usage: node tools/fixtures/gen-batch-011.mjs
//
// Writes fixtures/batch-011/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, cylinder, deg, faceAt, line, mm, planeAt, sketch, solidsOf, solve, vec2, vec3 } from './lib.mjs';

const helpers = `
// A B-spline's degree, control points (mm), weights and knots, as numbers.
function curveProbe(context is Context, edge is Query) returns map
{
    const c = evCurveDefinition(context, { "edge" : edge });
    if (!(c is BSplineCurve))
        return { "bspline" : false };
    var points = [];
    for (var p in c.controlPoints)
        points = append(points, [roundToPrecision(p[0] / millimeter, 6), roundToPrecision(p[1] / millimeter, 6), roundToPrecision(p[2] / millimeter, 6)]);
    return { "degree" : c.degree, "rational" : c.isRational, "periodic" : c.isPeriodic, "points" : points,
             "weights" : c.weights, "knots" : c.knots };
}

// A table's column names and cells as text, without its title (it names parts and features, whose default names
// differ) or its queries.
function tableText(t is map) returns array
{
    var out = [mapArray(t.columnDefinitions, function(c) { return toString(c.name); })];
    for (var row in t.rows)
        out = append(out, mapArray(t.columnDefinitions, function(c) { return toString(row.columnIdToCell[c.id]); }));
    return out;
}

// The tables about bodies a case made: all the cases share one Part Studio.
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

// A square tube frame profile, the only sketch of its own context, as one from an element library would be.
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

// A routing curve point at an offset from the origin.
function routingPoint(index is number, position is Vector) returns map
{
    return { "index" : index, "referenceType" : ReferenceType.ORIGIN, "vertex" : qNothing(), "curve" : qNothing(),
             "curveParameter" : 0, "alignCSys" : false, "flipCSys" : false, "dx" : position[0], "dy" : position[1],
             "dz" : position[2], "rotationMatrix" : identityMatrix(3), "overrideBendRadius" : false,
             "pointBendRadius" : 0 * meter, "hasDerivative" : false, "derivativeAlignment" : DerivativeAlignment.X,
             "magnitude" : 1, "oppositeDirection" : false, "derivativeDirection" : qNothing(), "arrayAdded" : false };
}
`;

const batch = new Batch({ number: 11, title: 'edit curve, path deviation, routing curves, computed mass, decals, form tags, cut lists, hole tables', helpers });

const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sketchEdge = (s, name) => `sketchEntityQuery(id + "${s}", EntityType.EDGE, "${name}")`;
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
// The edges of the wires a case's feature left (edit curve replaces the wire it's given, or one it extracts).
const resultEdges = (name) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const editCurve = (name, wire, fields) =>
  call('editCurve', name, {
    wire, approximate: 'false', elevate: 'false', planarize: 'false', editControlPoints: 'false', showDetails: 'false',
    approximationShowDeviation: 'false', ...fields,
  });

// ----- edit curve -----

batch.add('edit-curve', 'a quarter circle as a B-spline, raised to degree 4', [
  sketch('s1', xy), arc('s1', 'a1', [20, 0], [14.1421356, 14.1421356], [0, 20]), solve('s1'),
  editCurve('edit1', sketchEdge('s1', 'a1'), { elevate: 'true', elevationDegree: '4' }),
  returns({ curve: `curveProbe(context, ${resultEdges('edit1')})` }),
]);
batch.add('edit-curve', 'a spline through points in space, flattened onto the XY plane', [
  call('opFitSpline', 'spline1', { points: `[${vec3(0, 0, 0)}, ${vec3(10, 5, 3)}, ${vec3(20, 0, -2)}, ${vec3(30, 8, 4)}]` }),
  editCurve('edit1', `qCreatedBy(id + "spline1", EntityType.BODY)`, { planarize: 'true', planeReference: 'PlaneReference.XYPLANE' }),
  returns({ curve: `curveProbe(context, ${resultEdges('spline1')})` }),
]);
batch.add('edit-curve', 'a line and a tangent arc approximated by one cubic, showing its deviation', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [20, 0]), arc('s1', 'a1', [20, 0], [27.0710678, 2.9289322], [30, 10]), solve('s1'),
  editCurve('edit1', wireEdges('s1'), {
    approximate: 'true', approximationDegree: '3', approximationMaxCPs: '15', approximationTolerance: mm(0.01),
    keepStartDerivative: 'true', keepEndDerivative: 'true', approximationShowDeviation: 'true', maxDeviation: mm(0),
  }),
]);
batch.add('edit-curve', 'a line raised to degree 3 and its second control point moved', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [30, 0]), solve('s1'),
  editCurve('edit1', sketchEdge('s1', 'l1'), {
    elevate: 'true', elevationDegree: '3', editControlPoints: 'true', editPointMode: 'EditPointMode.XYZ',
    selectedIndices: '[{ "indexValue" : 1 }]',
    controlPointEdits: `[{ "index" : 1, "reference" : qNothing(), "x" : ${mm(0)}, "y" : ${mm(10)}, "z" : ${mm(5)}, "weight" : 1 }]`,
  }),
  returns({ curve: `curveProbe(context, ${resultEdges('edit1')})` }),
]);
batch.add('path-deviation', 'between a line and an arc with the same ends', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [20, 0]), arc('s1', 'a1', [0, 0], [10, 4], [20, 0]), solve('s1'),
  `const deviation = evMaxPathDeviation(context, { "side1" : ${sketchEdge('s1', 'l1')}, "side2" : ${sketchEdge('s1', 'a1')} });`,
  returns({
    deviation: 'roundToPrecision(deviation.deviation / millimeter, 6)',
    side1: 'mapArray(deviation.side1Point, function(x) { return roundToPrecision(x / millimeter, 6); })',
    side2: 'mapArray(deviation.side2Point, function(x) { return roundToPrecision(x / millimeter, 6); })',
  }),
]);

// ----- constrained surfaces -----

const gridPoints = [[0, 0, 0], [20, 0, 0], [40, 0, 0], [0, 15, 0], [20, 15, 4], [40, 15, 0], [0, 30, 0], [20, 30, 0], [40, 30, 0]];
batch.add('constrained-surface', 'through nine points with a bump in the middle', [
  gridPoints.map((p, i) => call('opPoint', `p${i + 1}`, { point: vec3(...p) })),
  call('constrainedSurface', 'surface1', {
    csType: 'ConstrainedSurfaceType.POINTS',
    vertices: `[${gridPoints.map((_, i) => `{ "vertex" : qCreatedBy(id + "p${i + 1}", EntityType.VERTEX), "normal" : qNothing(), "flipNormal" : false }`).join(', ')}]`,
    tolerance: mm(0.01), optimize: 'OptimizationMethod.SMOOTH', computeDeviation: 'false',
  }),
]);

// ----- routing curves -----

const routing = (name, curveType, points, fields = {}) =>
  call('routingCurve', name, {
    preselectedEntities: 'qNothing()', curveType, step: 'CurveStep.POINTS', inputType: 'InputType.VERTEX', inputVertices: 'qNothing()',
    isPeriodic: 'false', curveBaseCSys: 'qNothing()', pointsEditType: 'PointsEditType.SINGLE', pointIndex: '0',
    addPointOnAxisDrag: 'false', showCSys: 'false', keepPoints: 'false', curveLength: mm(0),
    points: `[${points.map((p, i) => `routingPoint(${i}, ${vec3(...p)})`).join(', ')}]`, ...fields,
  });
batch.add('routing-curve', 'a polyline through four points with 5 mm bends, and its table', [
  routing('route1', 'RoutingCurveType.POLYLINE', [[0, 0, 0], [40, 0, 0], [40, 30, 0], [40, 30, 20]], { bendRadius: mm(5) }),
  returns({
    xyz: 'caseTables(context, id, routingCurveTable(context, { "showLRA" : false }))',
    lra: 'caseTables(context, id, routingCurveTable(context, { "showLRA" : true }))',
  }),
]);
batch.add('routing-curve', 'a spline through three points, and its table', [
  routing('route1', 'RoutingCurveType.INTERPOLATED_SPLINE', [[0, 0, 0], [20, 10, 5], [40, 0, 10]], { bendRadius: mm(0) }),
  returns({ table: 'caseTables(context, id, routingCurveTable(context, { "showLRA" : false }))' }),
]);

// ----- computed mass, decals, tags -----

batch.add('mass', 'a steel block\'s computed mass', [
  box('base', [0, 0, 0], [40, 30, 12]),
  `setProperty(context, { "entities" : ${base}, "propertyType" : PropertyType.MATERIAL, "value" : material("Steel", 7850 * kilogram / meter ^ 3) });`,
  returns({ mass: `computeMass(context, ${base}, {}) / kilogram` }),
]);
const image = '{ "imageWidth" : 200, "imageHeight" : 100, "mediaType" : "image/png" } as ImageData';
const decalProbe = (face) =>
  returns({ decals: `mapArray(getAttribute(context, { "entity" : ${face}, "name" : "decals" }), function(d) { return { "mapping" : d.imageMappingType, "uv" : d.uvTransform, "plane" : d.planeSystem == undefined ? undefined : d.planeSystem.coordSystem, "cylinder" : d.cylinderSystem == undefined ? undefined : d.cylinderSystem.coordSystem }; })` });
const decal = (face, fields = {}) =>
  call('decal', 'decal1', {
    image, face, angle: deg(0), angleOppositeDirection: 'false', uShift: mm(0), vShift: mm(0), realign: 'false',
    maintainAspectRatio: 'true', aspectRatioConstraint: 'ImageAspectRatioConstraint.WIDTH_DRIVING', width: mm(20),
    imageOriginIndex: '4', ...fields,
  });
batch.add('decal', 'an image on the top of a block, turned 30 degrees and shifted', [
  box('base', [0, 0, 0], [40, 30, 12]),
  decal(faceAt(base, [20, 15, 12]), { angle: deg(30), uShift: mm(3), vShift: mm(-2) }),
  decalProbe(faceAt(base, [20, 15, 12])),
]);
batch.add('decal', 'an image wrapped on a cylinder', [
  cylinder('base', [0, 0], 10, 0, 30),
  decal(faceAt(base, [0, -10, 15])),
  decalProbe(faceAt(base, [0, -10, 15])),
]);
batch.add('tag', 'a block tagged as a form\'s positive part', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('tag', 'tag1', {
    tagPurpose: 'TagPurpose.FORM', positivePart: base, negativePart: 'qNothing()', flatFormSketch: '{} as FeatureList',
    cSysMateConnector: 'qNothing()',
  }),
  returns({ attribute: `getAttribute(context, { "entity" : ${base}, "name" : "formBodyAttribute" })` }),
]);

// ----- cut lists and hole tables -----

batch.add('cutlist', 'the two tubes of a mitred L, and its table', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path') }),
  call('cutlist', 'cutlist1', { selectAllFrames: 'false', frames: `qBodyType(qCreatedBy(id + "frame1", EntityType.BODY), BodyType.SOLID)` }),
  returns({
    composites: `evaluateQueryCount(context, qBodyType(qCreatedBy(id + "cutlist1", EntityType.BODY), BodyType.COMPOSITE))`,
    table: 'caseTables(context, id, cutlistTable(context, {}))',
  }),
]);
const topPoints = (s, points) => [
  sketch(s, planeAt([0, 0, 12], [0, 0, 1], [1, 0, 0])),
  points.map(([x, y], i) => `skPoint(${s}, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve(s),
];
const pointsOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
batch.add('hole-table', 'two simple holes and a counterbored one in a block', [
  box('base', [0, 0, 0], [40, 30, 12]), topPoints('pts1', [[10, 10], [30, 10]]), topPoints('pts2', [[20, 22]]),
  call('hole', 'hole1', { locations: pointsOf('pts1'), scope: base, style: 'HoleStyle.SIMPLE', holeDiameter: mm(4), holeDepth: mm(6), endStyle: 'HoleEndStyle.BLIND' }),
  call('hole', 'hole2', {
    locations: pointsOf('pts2'), scope: base, style: 'HoleStyle.C_BORE', holeDiameter: mm(6), cBoreDiameter: mm(10), cBoreDepth: mm(4),
    endStyle: 'HoleEndStyle.THROUGH',
  }),
  returns({ table: 'caseTables(context, id, holeTable(context, {}))' }),
]);

batch.write('tools/fixtures/gen-batch-011.mjs');
