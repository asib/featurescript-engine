// Generates fixture batch 027: more options of covered curve and surface features. An offset curve on a curved
// face, a curve extended up to a cylinder, a sheet extended up to a part, a
// fit spline matching curvature, closed and periodic curves, and a mirrored feature reapplied. (Offsets extended
// across a face are left out: for a chamfer's edge on a block's top Onshape found no result, warning
// OFFSET_CURVE_ON_FACE_CHAIN_YIELDED_NO_RESULT, and the rule isn't known yet.)
//
// Usage: node tools/fixtures/gen-batch-027.mjs
//
// Writes fixtures/batch-027/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, axisLine, box, call, createdBy, cylinder, edgeAt, edgeOf, faceAt, line, mm, planeBody, sketch, solidsOf, solve, vec2, vec3,
  vertexAt, PLANES,
} from './lib.mjs';

const helpers = `
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
`;

const batch = new Batch({ number: 27, title: 'more curve and surface options: offset curves on curved faces and extended, curves and sheets extended up to entities, curvature, periodic curves, reapplied mirrors', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const wireOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE)`;

// ----- offset curves -----

batch.add('offset-curve-on-face', 'a cylinder\'s top edge offset 5 mm down its side', [
  cylinder('base', [0, 0], 10, 0, 30),
  call('offsetCurveOnFace', 'offset1', {
    edges: edgeAt(base, [10, 0, 30]), distance: mm(5), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false',
    gapFill: 'GapFill.LINEAR', scope: 'OffsetCurveScope.OFFSET', targets: faceAt(base, [10, 0, 15]),
  }),
]);
// ----- extending up to entities -----

batch.add('trim-curve', 'a curve extended up to a cylinder', [
  cylinder('base', [40, 5], 8, -10, 10),
  sketch('s1', PLANES.xy), line('s1', 'line1', [0, 0], [20, 4]), solve('s1'),
  call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: wireOf('curve1'), endCondition: 'CurveExtensionEndCondition.UP_TO_ENTITY',
    extendTo: faceAt(base, [32, 5, 0]), extensionShape: 'CurveExtensionShape.TANGENT', flipHeuristics: 'false',
    helpPoint: vertexAt(wireOf('curve1'), [20, 4, 0]),
  }),
]);
batch.add('extend-surface', 'a sheet extended up to a part', [
  sketch('baseSketch', PLANES.xy), line('baseSketch', 'line1', [0, 0], [20, 0]), solve('baseSketch'),
  call('extrude', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(20),
  }),
  box('wall', [30, -10, -5], [36, 10, 25]),
  call('extendSurface', 'extend1', {
    entities: edgeAt(sheetOf('base'), [20, 0, 10]), endCondition: 'ExtendBoundingType.UP_TO_BODY', targetPart: solidsOf('wall'),
    hasOffset: 'false',
  }),
]);

// ----- curves -----

batch.add('fit-spline', 'through three vertices, leaving along x and matching a curvature there', [
  box('base', [0, 0, 0], [30, 20, 10]), axisLine('dirX', [0, 0, 0], [1, 0, 0]),
  call('fitSpline', 'spline1', {
    vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 10])}, ${vertexAt(base, [30, 20, 0])}])`,
    hasStartDirection: 'true', startDirection: edgeOf('dirX'), startMagnitude: '1', matchStartCurvature: 'true',
  }),
  returns({ curve: `curveProbe(context, qCreatedBy(id + "spline1", EntityType.EDGE))` }),
]);
batch.add('edit-curve', 'a closed fit spline raised to degree 4', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fitSpline', 'spline1', {
    vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 0])}, ${vertexAt(base, [30, 20, 10])}])`, closed: 'true',
  }),
  call('editCurve', 'edit1', {
    wire: `qCreatedBy(id + "spline1", EntityType.BODY)`, approximate: 'false', elevate: 'true', elevationDegree: '4', planarize: 'false',
    editControlPoints: 'false', showDetails: 'false', approximationShowDeviation: 'false',
  }),
  returns({ curve: `curveProbe(context, qCreatedBy(id + "spline1", EntityType.EDGE))` }),
]);

batch.add('fit-spline', 'closed through three vertices, as built', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fitSpline', 'spline1', {
    vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 0])}, ${vertexAt(base, [30, 20, 10])}])`, closed: 'true',
  }),
  returns({ curve: `curveProbe(context, qCreatedBy(id + "spline1", EntityType.EDGE))` }),
]);
batch.add('edit-curve', 'an open fit spline through four vertices raised to degree 4', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fitSpline', 'spline1', {
    vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 0])}, ${vertexAt(base, [30, 20, 10])}, ${vertexAt(base, [0, 20, 0])}])`,
  }),
  call('editCurve', 'edit1', {
    wire: `qCreatedBy(id + "spline1", EntityType.BODY)`, approximate: 'false', elevate: 'true', elevationDegree: '4', planarize: 'false',
    editControlPoints: 'false', showDetails: 'false', approximationShowDeviation: 'false',
  }),
  returns({ curve: `curveProbe(context, qCreatedBy(id + "spline1", EntityType.EDGE))` }),
]);

// ----- a mirrored feature, reapplied -----

// A boss up to a plane rising 1 in 3 along x, mirrored across the block's middle: reapplied, the mirrored boss ends
// higher.
const bossFeature = (name) =>
  `extrude(context, ${name}, { "entities" : qSketchRegion(id + "bossSketch"), "endBound" : BoundingType.UP_TO_SURFACE, "endBoundEntityFace" : qCreatedBy(id + "roof", EntityType.FACE), "operationType" : NewBodyOperationType.ADD, "defaultScope" : false, "booleanScope" : ${base} });`;
batch.add('feature-pattern', 'a boss up to a slanted plane mirrored across a block, reapplied', [
  box('base', [0, 0, 0], [60, 40, 10]),
  planeBody('roof', [30, 0, 30], [-0.31622776601683794, 0, 0.9486832980505138], [0.9486832980505138, 0, 0.31622776601683794]),
  `const bossSketch = newSketchOnPlane(context, id + "bossSketch", { "sketchPlane" : plane(${vec3(0, 0, 10)}, vector(0, 0, 1), vector(1, 0, 0)) });`,
  `skCircle(bossSketch, "circle1", { "center" : ${vec2(10, 20)}, "radius" : ${mm(3)} });`,
  'skSolve(bossSketch);',
  bossFeature('id + "boss"'),
  planeBody('mid', [30, 20, 5], [1, 0, 0], [0, 1, 0]),
  call('mirror', 'mirror1', {
    patternType: 'MirrorType.FEATURE', instanceFunction: `{ (id + "boss") : function(instanceId) { ${bossFeature('instanceId')} } } as FeatureList`,
    mirrorPlane: createdBy('mid', 'FACE'), fullFeaturePattern: 'true',
  }),
]);

batch.write('tools/fixtures/gen-batch-027.mjs');
