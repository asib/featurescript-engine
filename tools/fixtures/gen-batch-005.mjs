// Generates fixture batch 005: curve features (composite, projected, intersection, isoparametric and offset curves,
// trimming, curve patterns), surface features (offset, extend, fill, boundary, ruled, enclose, mutual trim) and mate
// connectors.
//
// Usage: node tools/fixtures/gen-batch-005.mjs
//
// Writes fixtures/batch-005/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, circle, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm,
  planeAt, planeBody, rect, regions, sketch, solidsOf, solve, vec3, vertexAt,
} from './lib.mjs';

const batch = new Batch({
  number: 5,
  title: 'curve features, surface features and mate connectors',
});

const base = solidsOf('base');
const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const block = box('base', [0, 0, 0], [30, 20, 10]);
const can = cylinder('base', [0, 0], 10, 0, 30);
// A sheet: a line extruded as a surface, 20 mm along z.
const sheet = (name, a, b) => [
  sketch(`${name}Sketch`, PLANES.xy), line(`${name}Sketch`, 'line1', a, b), solve(`${name}Sketch`),
  call('extrude', name, {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges(`${name}Sketch`), endBound: 'BoundingType.BLIND',
    depth: mm(20),
  }),
];

// ----- curves -----

batch.add('composite-curve', 'three edges of a block', [
  block,
  call('compositeCurve', 'curve1', { edges: union(edgeAt(base, [15, 0, 0]), edgeAt(base, [30, 10, 0]), edgeAt(base, [30, 20, 5])) }),
]);
batch.add('composite-curve', 'a line, an arc and a line from a sketch', [
  sketch('path', PLANES.xy), line('path', 'line1', [0, 0], [20, 0]), arc('path', 'arc1', [20, 0], [27.07107, 2.92893], [30, 10]),
  line('path', 'line2', [30, 10], [30, 30]), solve('path'),
  call('compositeCurve', 'curve1', { edges: wireEdges('path') }),
]);
batch.add('project-curves', 'a line seen from above and an arc seen from the front', [
  sketch('s1', PLANES.xy), line('s1', 'line1', [0, 0], [30, 20]), solve('s1'),
  sketch('s2', PLANES.xz), arc('s2', 'arc1', [0, 0], [15, 8], [30, 0]), solve('s2'),
  call('projectCurves', 'project1', { curveProjectionType: 'CurveProjectionType.TWO_SKETCHES', sketchEdges1: wireEdges('s1'), sketchEdges2: wireEdges('s2') }),
]);
batch.add('project-curves', 'a circle projected onto a cylinder along its normal', [
  cylinder('base', [0, 0], 20, -20, 20),
  sketch('s1', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), circle('s1', 'circle1', [0, 0], 6), solve('s1'),
  axisLine('dir', [0, 0, 0], [1, 0, 0]),
  call('projectCurves', 'project1', {
    curveProjectionType: 'CurveProjectionType.CURVE_TO_FACE', dropTools: wireEdges('s1'), targets: faceAt(base, [20, 0, 0]),
    projectionType: 'ProjectionType.DIRECTION', directionQuery: edgeOf('dir'), oppositeDirection: 'false',
  }),
]);
batch.add('intersection-curve', 'a cylinder and a slanted plane: an ellipse', [
  can, planeBody('cut', [0, 0, 15], [0.3, 0, 1], [1, 0, -0.3]),
  call('intersectionCurve', 'curve1', { group1: faceAt(base, [10, 0, 15]), group2: createdBy('cut', 'FACE') }),
]);
batch.add('intersection-curve', 'two crossing cylinders', [
  cylinder('base', [0, 0], 10, -20, 20),
  sketch('s2', PLANES.yz), circle('s2', 'circle1', [0, 0], 6), solve('s2'),
  call('extrude', 'cross', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), symmetric: 'true' }),
  call('intersectionCurve', 'curve1', {
    group1: faceAt(base, [10, 0, 0]), group2: `qGeometry(qOwnedByBody(${solidsOf('cross')}, EntityType.FACE), GeometryType.CYLINDER)`,
  }),
]);
batch.add('isoparametric-curve', 'three equally spaced curves along a cylinder', [
  can,
  call('isoparametricCurve', 'curve1', { face: faceAt(base, [10, 0, 15]), directionType: 'DirectionType.U_DIRECTION', equalSpacing: 'true', nCurves: '3' }),
]);
batch.add('isoparametric-curve', 'three equally spaced curves round a cylinder', [
  can,
  call('isoparametricCurve', 'curve1', { face: faceAt(base, [10, 0, 15]), directionType: 'DirectionType.V_DIRECTION', equalSpacing: 'true', nCurves: '3' }),
]);
batch.add('offset-curve', 'an edge offset 3 mm across the top of a block', [
  block,
  call('offsetCurveOnFace', 'offset1', {
    edges: edgeAt(base, [15, 0, 10]), distance: mm(3), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false',
    gapFill: 'GapFill.LINEAR', scope: 'OffsetCurveScope.OFFSET', targets: faceAt(base, [15, 10, 10]),
  }),
]);
// trimCurve takes curves that aren't sketches: a composite curve of a sketch line.
batch.add('trim-curve', 'a curve extended 10 mm', [
  sketch('s1', PLANES.xy), line('s1', 'line1', [0, 0], [20, 10]), solve('s1'),
  call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: createdBy('curve1'), endCondition: 'CurveExtensionEndCondition.BLIND',
    extensionDistance: mm(10), extensionShape: 'CurveExtensionShape.TANGENT', flipHeuristics: 'false',
    helpPoint: vertexAt(createdBy('curve1'), [20, 10, 0]),
  }),
]);
batch.add('curve-pattern', 'a cylinder patterned along an arc', [
  cylinder('base', [0, 0], 3, -3, 3),
  sketch('path', PLANES.xy), arc('path', 'arc1', [0, 0], [20, 8.28427], [28.28427, 28.28427]), solve('path'),
  call('curvePattern', 'pattern1', {
    entities: base, edges: wireEdges('path'), spacingType: 'CurvePatternSpacingType.EQUAL', instanceCount: '4',
  }),
]);

// ----- surfaces -----

batch.add('offset-surface', 'the top of a block offset 5 mm', [
  block, call('offsetSurface', 'offset1', { surfacesAndFaces: faceAt(base, [15, 10, 10]), offset: mm(5), oppositeDirection: 'false' }),
]);
batch.add('offset-surface', 'a cylinder side offset 3 mm inwards', [
  can, call('offsetSurface', 'offset1', { surfacesAndFaces: faceAt(base, [10, 0, 15]), offset: mm(3), oppositeDirection: 'true' }),
]);
batch.add('offset-surface', 'a sheet offset 4 mm', [
  sheet('base', [0, 0], [25, 10]), call('offsetSurface', 'offset1', { surfacesAndFaces: base.replace('BodyType.SOLID', 'BodyType.SHEET'), offset: mm(4), oppositeDirection: 'false' }),
]);
batch.add('extend-surface', 'a sheet extended 6 mm', [
  sheet('base', [0, 0], [25, 10]),
  call('extendSurface', 'extend1', {
    entities: `qOwnedByBody(qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SHEET), EntityType.EDGE)`,
    extendDistance: mm(6),
  }),
]);
batch.add('extend-surface', 'one edge of a sheet extended 5 mm', [
  sheet('base', [0, 0], [25, 0]),
  call('extendSurface', 'extend1', { entities: edgeAt(`qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SHEET)`, [25, 0, 10]), extendDistance: mm(5) }),
]);
const fillCase = (description, setup, edges) =>
  batch.add('fill', description, [
    setup,
    call('fill', 'fill1', {
      edges: `[${edges.map((e) => `{ "entities" : ${e}, "continuity" : GeometricContinuity.G0 }`).join(', ')}]`,
      surfaceOperationType: 'NewSurfaceOperationType.NEW',
    }),
  ]);
fillCase('the top edges of a block: a flat face', block, [union(...[[15, 0], [30, 10], [15, 20], [0, 10]].map(([x, y]) => edgeAt(base, [x, y, 10])))]);
fillCase('a circle: a disc', [sketch('s1', PLANES.xy), circle('s1', 'circle1', [0, 0], 8), solve('s1')], [wireEdges('s1')]);
batch.add('boundary-surface', 'between two parallel arcs', [
  sketch('s1', PLANES.xz), arc('s1', 'arc1', [0, 0], [15, 6], [30, 0]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), arc('s2', 'arc1', [0, 0], [15, 10], [30, 0]), solve('s2'),
  call('boundarySurface', 'surface1', {
    uProfilesArray: `[{ "uProfileEntities" : ${wireEdges('s1')}, "uCondition" : BSurfEndDerivativeType.DEFAULT }, { "uProfileEntities" : ${wireEdges('s2')}, "uCondition" : BSurfEndDerivativeType.DEFAULT }]`,
    vProfilesArray: '[]',
  }),
]);
batch.add('ruled-surface', 'from the top edges of a block, 5 mm along the faces\' normals', [
  block,
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [15, 0, 10]), ruledType: 'RuledSurfaceInterfaceType.NORMAL', distance: mm(5), referenceFaces: faceAt(base, [15, 10, 10]),
  }),
]);
batch.add('ruled-surface', 'from an edge, aligned with a direction', [
  block, axisLine('dir', [0, 0, 0], [0, 0, 1]),
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [15, 0, 10]), ruledType: 'RuledSurfaceInterfaceType.ALIGNED_WITH_VECTOR', axis: edgeOf('dir'), distance: mm(8),
  }),
]);
batch.add('enclose', 'a tube closed by two planes', [
  sketch('s1', PLANES.xy), circle('s1', 'circle1', [0, 0], 10), solve('s1'),
  call('extrude', 'tube', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(30), symmetric: 'true' }),
  planeBody('p1', [0, 0, -8], [0, 0, 1], [1, 0, 0]), planeBody('p2', [0, 0, 9], [0.2, 0, 1], [1, 0, -0.2]),
  call('enclose', 'enclose1', { entities: union(createdBy('tube'), createdBy('p1'), createdBy('p2')) }),
]);
batch.add('mutual-trim', 'two crossing sheets trimmed to each other', [
  sheet('a', [0, 0], [30, 10]), sheet('b', [20, -5], [5, 15]),
  call('mutualTrim', 'trim1', {
    body1: createdBy('a'), keepOtherSide1: 'false', body2: createdBy('b'), keepOtherSide2: 'false', merge: 'true',
  }),
]);

// ----- mate connectors -----

const mc = (description, setup, fields) =>
  batch.add('mate-connector', description, [setup, call('mateConnector', 'mc1', { ...fields })]);
mc('at the centroid of a face', block, { originQuery: faceAt(base, [15, 10, 10]), entityInferenceType: 'EntityInferenceType.CENTROID', ownerPart: base });
mc('at a vertex', block, { originQuery: vertexAt(base, [30, 20, 10]), entityInferenceType: 'EntityInferenceType.POINT', ownerPart: base });
mc('at the centre of a circular edge', can, { originQuery: edgeAt(base, [10, 0, 30]), entityInferenceType: 'EntityInferenceType.CENTER', ownerPart: base });
mc('at the mid point of an edge, moved and turned', block, {
  originQuery: edgeAt(base, [15, 0, 10]), entityInferenceType: 'EntityInferenceType.MID_POINT', ownerPart: base,
  transform: 'true', translationX: mm(2), translationY: mm(-3), translationZ: mm(4), rotationType: 'RotationType.ABOUT_Z', rotation: deg(30),
});
mc('at the centroid of a side face, flipped', block, {
  originQuery: faceAt(base, [30, 10, 5]), entityInferenceType: 'EntityInferenceType.CENTROID', ownerPart: base, flipPrimary: 'true',
  secondaryAxisType: 'MateConnectorAxisType.PLUS_Y',
});
batch.add('transform', 'move a block from one mate connector to another', [
  block,
  call('mateConnector', 'from', { originQuery: faceAt(base, [15, 10, 10]), entityInferenceType: 'EntityInferenceType.CENTROID', ownerPart: base }),
  cylinder('target', [60, 0], 5, 0, 12),
  call('mateConnector', 'to', { originQuery: faceAt(solidsOf('target'), [60, 0, 12]), entityInferenceType: 'EntityInferenceType.CENTROID', ownerPart: solidsOf('target') }),
  call('transform', 'transform1', {
    entities: base, transformType: 'TransformType.TRANSFORM_MATE_CONNECTORS', baseConnector: `qBodyType(qCreatedBy(id + "from", EntityType.BODY), BodyType.MATE_CONNECTOR)`,
    destinationConnector: `qBodyType(qCreatedBy(id + "to", EntityType.BODY), BodyType.MATE_CONNECTOR)`, makeCopy: 'false',
    oppositeDirectionMateAxis: 'false', secondaryAxisType: 'MateConnectorAxisType.PLUS_X',
  }),
]);

batch.write('tools/fixtures/gen-batch-005.mjs');
