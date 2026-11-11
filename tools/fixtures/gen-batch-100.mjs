// Generates fixture batch 100: std's API functions whose builtins no std feature calls, so no batch had reached:
// flipping a sheet's orientation, a sheet from a B-spline surface, evaluating a spline, parsing JSON, approximate
// mass properties, points' deviation from a body, a body's tolerances and faults, a face's curvature derivative, a
// face approximated as a B-spline, and debug display calls.
//
// Usage: node tools/fixtures/gen-batch-100.mjs
//
// Writes fixtures/batch-100/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, cylinder, mm, vec3 } from './lib.mjs';

const helpers = `
function round6(v is Vector) returns array
{
    return [roundToPrecision(v[0], 6), roundToPrecision(v[1], 6), roundToPrecision(v[2], 6)];
}

function roundAll(a is array) returns array
{
    return mapArray(a, function(x) { return x is number ? roundToPrecision(x, 6) : (x is array ? roundAll(x) : x); });
}
`;

const batch = new Batch({ number: 100, title: "std's API builtins no feature calls", helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const block = call('fCuboid', 'base', { corner1: vec3(0, 0, 0), corner2: vec3(10, 20, 30) });
const base = 'qCreatedBy(id + "base", EntityType.BODY)';
const sheetFace = 'qOwnedByBody(qCreatedBy(id + "sheet", EntityType.BODY), EntityType.FACE)';
const normal = `round6(evFaceTangentPlane(context, { "face" : ${sheetFace}, "parameter" : vector(0.5, 0.5) }).normal)`;

batch.add('flip', "a block's top extracted as a sheet, its orientation flipped: its normal before and after", [
  block,
  `opExtractSurface(context, id + "sheet", { "faces" : qContainsPoint(qOwnedByBody(${base}, EntityType.FACE), ${vec3(5, 10, 30)}) });`,
  `const before = ${normal};`,
  `opFlipOrientation(context, id + "flip", { "bodies" : qCreatedBy(id + "sheet", EntityType.BODY) });`,
  returns({ before: 'before', after: normal }),
]);
batch.add('bspline', 'a sheet from a B-spline surface, degree 2 by 1, its middle row raised 10 mm', [
  `const surface = bSplineSurface({ "uDegree" : 2, "vDegree" : 1, "isUPeriodic" : false, "isVPeriodic" : false,
        "controlPoints" : controlPointMatrix([[${vec3(0, 0, 0)}, ${vec3(0, 20, 0)}], [${vec3(15, 0, 10)}, ${vec3(15, 20, 10)}], [${vec3(30, 0, 0)}, ${vec3(30, 20, 0)}]]),
        "uKnots" : knotArray([0, 0, 0, 1, 1, 1]), "vKnots" : knotArray([0, 0, 1, 1]) });`,
  `opCreateBSplineSurface(context, id + "surface1", { "bSplineSurface" : surface });`,
  returns({ normal: `round6(evFaceTangentPlane(context, { "face" : qOwnedByBody(qCreatedBy(id + "surface1", EntityType.BODY), EntityType.FACE), "parameter" : vector(0.5, 0.5) }).normal)` }),
]);
batch.add('evaluate', 'a quadratic spline evaluated at four parameters, with two derivatives', [
  `const spline = bSplineCurve({ "degree" : 2, "isPeriodic" : false, "controlPoints" : [${vec3(0, 0, 0)}, ${vec3(10, 10, 0)}, ${vec3(20, 0, 5)}], "knots" : knotArray([0, 0, 0, 1, 1, 1]) });`,
  `const values = evaluateSpline({ "spline" : spline, "parameters" : [0, 0.25, 0.5, 1], "nDerivatives" : 2 });`,
  returns({ values: 'mapArray(values, function(row) { return mapArray(row, function(p) { return round6(p / millimeter); }); })' }),
]);
batch.add('json', 'a JSON string parsed', [
  `return { "parsed" : parseJson("{\\"a\\" : 1, \\"b\\" : [1, 2.5, \\"x\\"], \\"c\\" : null, \\"d\\" : true, \\"e\\" : { \\"f\\" : \\"g\\" } }") };`,
]);
batch.add('mass', "a block's approximate mass properties (1 kg/m³), and its faces' (1 kg/m²)", [
  block,
  `const solid = evApproximateMassProperties(context, { "entities" : ${base}, "density" : 1 * kilogram / meter ^ 3 });`,
  `const faces = evApproximateMassProperties(context, { "entities" : qOwnedByBody(${base}, EntityType.FACE), "density" : 1 * kilogram / meter ^ 2 });`,
  returns({
    mass: 'roundToPrecision(solid.mass / kilogram * 1e9, 3)', centroid: 'round6(solid.centroid / millimeter)', volume: 'roundToPrecision(solid.volume / millimeter ^ 3, 3)',
    faceMass: 'roundToPrecision(faces.mass / kilogram * 1e6, 3)', faceCentroid: 'round6(faces.centroid / millimeter)',
  }),
]);
batch.add('deviation', "points' deviations from a block (one inside, one out past a face, one off a corner)", [
  block,
  `const deviations = evPointsDeviation(context, { "points" : [${vec3(5, 5, 5)}, ${vec3(5, 10, 40)}, ${vec3(13, 24, 30)}], "topologies" : ${base}, "allDeviations" : true });`,
  returns({ deviations: 'mapArray(deviations, function(d) { return { "deviation" : roundToPrecision(d.deviation / millimeter, 6), "topologyPoint" : round6(d.topologyPoint / millimeter) }; })' }),
]);
batch.add('tolerance', "a block's tolerances, its greatest tolerance and its faults", [
  block,
  `const tolerances = evTolerances(context, { "entities" : ${base} });`,
  returns({ count: 'size(tolerances)', max: `evMaxTolerance(context, { "entities" : ${base} })`, faults: `size(evFaults(context, { "entities" : ${base} }))` }),
]);
batch.add('curvature', "a cylinder's (radius 10) curvature derivative along its circle and along its axis", [
  cylinder('base', [0, 0], 10, 0, 30),
  `const side = qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CYLINDER);`,
  `const around = evFaceCurvatureDerivative(context, { "face" : side, "parameter" : vector(0.25, 0.5), "direction" : vector(0, 1, 0) });`,
  `const along = evFaceCurvatureDerivative(context, { "face" : side, "parameter" : vector(0.25, 0.5), "direction" : vector(0, 0, 1) });`,
  returns({ around: 'roundAll([around[0] * millimeter ^ 2, around[1] * millimeter ^ 2, around[2] * millimeter ^ 2])', along: 'roundAll([along[0] * millimeter ^ 2, along[1] * millimeter ^ 2, along[2] * millimeter ^ 2])' }),
]);
batch.add('approximate', "a cylinder's side approximated as a B-spline surface", [
  cylinder('base', [0, 0], 10, 0, 30),
  `const result = evApproximateBSplineSurface(context, { "face" : qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CYLINDER) });`,
  returns({
    degrees: '[result.bSplineSurface.uDegree, result.bSplineSurface.vDegree]', rational: 'result.bSplineSurface.isRational',
    periodic: '[result.bSplineSurface.isUPeriodic, result.bSplineSurface.isVPeriodic]', uKnots: 'roundAll(result.bSplineSurface.uKnots)', vKnots: 'roundAll(result.bSplineSurface.vKnots)',
    rows: 'size(result.bSplineSurface.controlPoints)', columns: 'size(result.bSplineSurface.controlPoints[0])', boundaries: 'size(result.boundaryBSplineCurves)',
  }),
]);
batch.add('debug', 'debug display calls: auxiliary entities, a reference frame, a timer', [
  block,
  `addAuxiliaryEntities(context, id + "aux", ${base}, DebugColor.RED);`,
  `addReferenceCSysFrame(context, id + "frame", ${vec3(0, 0, 0)}, vector(1, 0, 0), vector(0, 0, 1));`,
  `startTimer("fixtureTimer");`,
  `printTimer("fixtureTimer");`,
  returns({ done: 'true' }),
]);

batch.write('tools/fixtures/gen-batch-100.mjs');
