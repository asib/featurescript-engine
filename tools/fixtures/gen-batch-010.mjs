// Generates fixture batch 010: full round, variable and partial fillets, splines through edges, bridging curves,
// wraps, face blends and moving faces.
//
// Usage: node tools/fixtures/gen-batch-010.mjs
//
// Writes fixtures/batch-010/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, sketch,
  solidsOf, solve, vec2, vec3, vertexAt,
} from './lib.mjs';

const batch = new Batch({ number: 10, title: 'full round, variable and partial fillets, splines through edges, bridging curves, wraps, face blends, moving faces' });

const base = solidsOf('base');
const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sketchEdge = (s, name) => `sketchEntityQuery(id + "${s}", EntityType.EDGE, "${name}")`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;

// ----- fillets -----

batch.add('fillet', 'a full round on the top of a thin wall', [
  box('base', [0, 0, 0], [40, 4, 20]),
  call('fillet', 'fillet1', {
    filletType: 'FilletType.FULL_ROUND', side1Face: faceAt(base, [20, 0, 10]), side2Face: faceAt(base, [20, 4, 10]),
    centerFaces: faceAt(base, [20, 2, 20]), tangentPropagation: 'false',
  }),
]);
batch.add('fillet', 'a variable fillet, 2 mm to 5 mm along an edge', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('fillet', 'fillet1', {
    entities: edgeAt(base, [20, 0, 12]), radius: mm(3), isVariable: 'true',
    vertexSettings: `[{ "vertex" : ${vertexAt(base, [0, 0, 12])}, "vertexRadius" : ${mm(2)} }, { "vertex" : ${vertexAt(base, [40, 0, 12])}, "vertexRadius" : ${mm(5)} }]`,
    smoothTransition: 'true',
  }),
]);
batch.add('fillet', 'a partial fillet from a quarter to three quarters of an edge', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('fillet', 'fillet1', {
    entities: edgeAt(base, [20, 0, 12]), radius: mm(3), isPartial: 'true', startPartialType: 'EndTypePartialFillet.PERCENTAGE',
    partialFirstEdgeTotalParameter: '0.25', partialOppositeParameter: 'false', secondBound: 'true',
    endPartialType: 'EndTypePartialFillet.PERCENTAGE', partialSecondEdgeTotalParameter: '0.75',
  }),
]);

// ----- curves -----

batch.add('fit-spline', 'through a line, a tangent arc and a tangent line', [
  sketch('s1', PLANES.xy), line('s1', 'l1', [0, 0], [20, 0]), arc('s1', 'a1', [20, 0], [27.0710678, 2.9289322], [30, 10]),
  line('s1', 'l2', [30, 10], [30, 30]), solve('s1'),
  call('fitSpline', 'spline1', { fitType: 'FitSplineType.EDGES', edges: wireEdges('s1') }),
]);
batch.add('bridging-curve', 'tangent to two lines', [
  sketch('s1', PLANES.xy), line('s1', 'l1', [0, 0], [20, 0]), line('s1', 'l2', [40, 20], [40, 40]), solve('s1'),
  call('bridgingCurve', 'bridge1', { side1: sketchEdge('s1', 'l1'), side2: sketchEdge('s1', 'l2'), magnitude: '1', bias: '0.5' }),
]);
batch.add('bridging-curve', 'matching the curvature of a line and an arc', [
  sketch('s1', PLANES.xy), line('s1', 'l1', [0, 0], [20, 0]), arc('s1', 'a1', [40, 10], [50, 20], [60, 10]), solve('s1'),
  call('bridgingCurve', 'bridge1', {
    side1: sketchEdge('s1', 'l1'), side2: sketchEdge('s1', 'a1'), match1: 'BridgingCurveMatchType.CURVATURE',
    match2: 'BridgingCurveMatchType.CURVATURE', magnitude: '1', bias: '0.5',
  }),
]);

// ----- wrap and face blend -----

batch.add('wrap', 'a rectangle wrapped onto a cylinder as a surface', [
  cylinder('base', [0, 0], 10, 0, 30),
  sketch('s1', planeAt([0, -10, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [-5, 10], [5, 20]), solve('s1'),
  call('wrap', 'wrap1', {
    resultType: 'WrapResultType.SURFACE', source: `qCreatedBy(id + "s1", EntityType.FACE)`, destination: faceAt(base, [0, -10, 15]),
    flipAlignment: 'false', trim: 'true', customAnchors: 'false',
  }),
]);
batch.add('face-blend', 'a round between two sheets at right angles', [
  sketch('s1', PLANES.xy), line('s1', 'l1', [0, 0], [30, 0]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
  sketch('s2', PLANES.xz), line('s2', 'l1', [0, 0], [30, 0]), solve('s2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('floor')}, EntityType.FACE)`, flipSide1: 'true', side2: `qOwnedByBody(${sheetsOf('wall')}, EntityType.FACE)`,
    flipSide2: 'false', propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS',
    radius: mm(5), asymmetric: 'false', crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false',
    conicHoldLines: 'false', hasCliffEdges: 'false', hasCaps: 'false', hasLimits: 'false', hasHelpPoint: 'false',
    trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false', showIsocurves: 'false',
  }),
]);

// ----- move face -----

batch.add('move-face', 'the top of a block moved 5 mm up and sideways', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('moveFace', 'move1', {
    moveFaces: faceAt(base, [20, 15, 12]), moveFaceType: 'MoveFaceType.TRANSLATE', direction: edgeAt(base, [0, 15, 0]),
    limitType: 'MoveFaceBoundingType.BLIND', translationDistance: mm(5), oppositeDirection: 'false', reFillet: 'false',
  }),
]);
batch.add('move-face', 'the side of a block turned 10 degrees about its bottom edge', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('moveFace', 'move1', {
    moveFaces: faceAt(base, [40, 15, 6]), moveFaceType: 'MoveFaceType.ROTATE', axis: edgeAt(base, [40, 15, 0]), angle: deg(10),
    oppositeDirection: 'false', reFillet: 'false',
  }),
]);

batch.write('tools/fixtures/gen-batch-010.mjs');
