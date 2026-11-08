// Generates fixture batch 006: holes, ribs, modify fillet, body draft, face patterns and mirrors, splitting faces,
// tangent planes, composite parts, and the remaining options of thin and surface features.
//
// Usage: node tools/fixtures/gen-batch-006.mjs
//
// Writes fixtures/batch-006/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, circle, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm,
  planeAt, planeBody, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt,
} from './lib.mjs';

const batch = new Batch({
  number: 6,
  title: 'holes, ribs, modify fillet, body draft, face patterns, face splits, tangent planes, composite parts',
});

const base = solidsOf('base');
const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
// Sketch points' vertices, as the hole feature's locations are picked in the UI.
const pointsOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
const block = box('base', [0, 0, 0], [40, 30, 12]);
// Sketch points on the block's top face.
const topPoints = (points) => [
  sketch('pts', planeAt([0, 0, 12], [0, 0, 1], [1, 0, 0])),
  points.map(([x, y], i) => `skPoint(pts, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve('pts'),
];

// ----- holes -----

const hole = (description, points, fields) =>
  batch.add('hole', description, [
    block, topPoints(points),
    call('hole', 'hole1', { locations: pointsOf('pts'), scope: base, ...fields }),
  ]);
hole('a simple hole, 6 mm across and 8 deep', [[20, 15]], { style: 'HoleStyle.SIMPLE', holeDiameter: mm(6), holeDepth: mm(8), endStyle: 'HoleEndStyle.BLIND' });
hole('a simple hole through', [[10, 10]], { style: 'HoleStyle.SIMPLE', holeDiameter: mm(5), endStyle: 'HoleEndStyle.THROUGH' });
hole('two simple holes', [[10, 10], [30, 20]], { style: 'HoleStyle.SIMPLE', holeDiameter: mm(4), holeDepth: mm(6), endStyle: 'HoleEndStyle.BLIND' });
hole('a counterbored hole through', [[20, 15]], {
  style: 'HoleStyle.C_BORE', holeDiameter: mm(6), cBoreDiameter: mm(10), cBoreDepth: mm(4), endStyle: 'HoleEndStyle.THROUGH',
});
hole('a countersunk hole, 8 deep', [[20, 15]], {
  style: 'HoleStyle.C_SINK', holeDiameter: mm(6), cSinkDiameter: mm(12), cSinkAngle: deg(90), holeDepth: mm(8), endStyle: 'HoleEndStyle.BLIND',
});
hole('a blind hole with a flat tip', [[20, 15]], {
  style: 'HoleStyle.SIMPLE', holeDiameter: mm(6), holeDepth: mm(8), endStyle: 'HoleEndStyle.BLIND', tipAngleStyle: 'TipAngleStyle.FLAT',
});

// ----- ribs -----

const walls = [
  box('base', [0, 0, 0], [40, 30, 4]),
  box('wall', [0, 0, 4], [4, 30, 24]),
  call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: union(base, solidsOf('wall')) }),
];
batch.add('rib', 'a rib in the corner of an L', [
  walls,
  sketch('profile', planeAt([0, 15, 0], [0, -1, 0], [1, 0, 0])), line('profile', 'line1', [4, 20], [30, 4]), solve('profile'),
  call('rib', 'rib1', { profiles: wireEdges('profile'), parts: base, thickness: mm(3), ribExtrusionDirection: 'RibExtrusionDirection.PARALLEL_TO_SKETCH_PLANE', oppositeDirection: 'true' }),
]);
batch.add('rib', 'a rib with a draft', [
  walls,
  sketch('profile', planeAt([0, 15, 0], [0, -1, 0], [1, 0, 0])), line('profile', 'line1', [4, 20], [30, 4]), solve('profile'),
  call('rib', 'rib1', {
    profiles: wireEdges('profile'), parts: base, thickness: mm(3), ribExtrusionDirection: 'RibExtrusionDirection.PARALLEL_TO_SKETCH_PLANE',
    oppositeDirection: 'true', hasDraft: 'true', draftAngle: deg(3),
  }),
]);

// ----- modify fillet -----

const filleted = [block, call('fillet', 'fillet1', { entities: edgeAt(base, [0, 0, 6]), radius: mm(4) })];
batch.add('modify-fillet', 'change a fillet to 7 mm', [
  filleted, call('modifyFillet', 'modify1', { faces: createdBy('fillet1', 'FACE'), modifyFilletType: 'ModifyFilletType.CHANGE_RADIUS', radius: mm(7), reFillet: 'false' }),
]);
batch.add('modify-fillet', 'remove a fillet', [
  filleted, call('modifyFillet', 'modify1', { faces: createdBy('fillet1', 'FACE'), modifyFilletType: 'ModifyFilletType.REMOVE_FILLET', reFillet: 'false' }),
]);

// ----- body draft -----

batch.add('body-draft', 'the side faces of a block, 4 degrees from the bottom', [
  block, axisLine('pull', [0, 0, 0], [0, 0, 1]),
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.FACES', faces: union(...[[20, 0, 6], [40, 15, 6], [20, 30, 6], [0, 15, 6]].map((p) => faceAt(base, p))),
    pullDirection: edgeOf('pull'), partingObject: faceAt(base, [20, 15, 0]), angle: deg(4),
  }),
]);

// ----- face patterns and mirrors -----

const boss = [block, cylinder('boss', [8, 8], 3, 12, 18), call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: union(base, solidsOf('boss')) })];
const bossFaces = union(faceAt(base, [11, 8, 15]), faceAt(base, [8, 8, 18]));
batch.add('face-pattern', 'a boss patterned along a block', [
  boss, axisLine('dirX', [0, 0, 0], [1, 0, 0]),
  call('linearPattern', 'pattern1', { patternType: 'PatternType.FACE', faces: bossFaces, directionOne: edgeOf('dirX'), distance: mm(10), instanceCount: '3' }),
]);
batch.add('face-pattern', 'a boss patterned round a block\'s middle', [
  boss, axisLine('axis1', [20, 15, 0], [20, 15, 30]),
  call('circularPattern', 'pattern1', { patternType: 'PatternType.FACE', faces: bossFaces, axis: edgeOf('axis1'), angle: deg(360), instanceCount: '4', equalSpace: 'true' }),
]);
batch.add('face-pattern', 'a boss mirrored across the block\'s middle', [
  boss, planeBody('mid', [20, 15, 6], [1, 0, 0], [0, 1, 0]),
  call('mirror', 'mirror1', { patternType: 'MirrorType.FACE', faces: bossFaces, mirrorPlane: createdBy('mid', 'FACE') }),
]);

// ----- splitting faces -----

batch.add('split-face', 'the top of a block split by a sketch line', [
  block, sketch('cut', planeAt([0, 0, 12], [0, 0, 1], [1, 0, 0])), line('cut', 'line1', [10, -5], [30, 35]), solve('cut'),
  call('splitPart', 'split1', { splitType: 'SplitType.FACE', faceTargets: faceAt(base, [20, 15, 12]), faceTools: wireEdges('cut') }),
]);

// ----- planes -----

batch.add('cplane', 'tangent to a cylinder, parallel to a plane', [
  cylinder('base', [0, 0], 10, 0, 20), planeBody('ref', [30, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('cPlane', 'plane1', { cplaneType: 'CPlaneType.TANGENT_PLANE', entities: union(faceAt(base, [10, 0, 10]), createdBy('ref', 'FACE')), width: mm(60), height: mm(40) }),
]);

// ----- composite parts -----

batch.add('composite-part', 'two blocks made one composite part', [
  box('a', [0, 0, 0], [10, 10, 10]), box('b', [20, 0, 0], [30, 10, 10]),
  call('compositePart', 'composite1', { bodies: union(solidsOf('a'), solidsOf('b')), closed: 'true' }),
  `return { "composites" : evaluateQueryCount(context, qBodyType(qEverything(EntityType.BODY), BodyType.COMPOSITE)) };`,
]);

// ----- remaining options of features matched before -----

batch.add('revolve', 'a thin revolve of a line', [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('s1', 'line1', [10, 0], [20, 15]), solve('s1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'revolve1', {
    bodyType: 'ExtendedToolBodyType.THIN', wallShape: wireEdges('s1'), axis: edgeOf('axis1'), midplane: 'false', thickness1: mm(2),
    thickness2: mm(0), flipWall: 'false',
  }),
]);
batch.add('sweep', 'a thin sweep of a line along an arc', [
  sketch('profile', PLANES.yz), line('profile', 'line1', [-4, 0], [4, 0]), solve('profile'),
  sketch('path', PLANES.xy), arc('path', 'arc1', [0, 0], [20, 8.28427], [28.28427, 28.28427]), solve('path'),
  call('sweep', 'sweep1', {
    bodyType: 'ExtendedToolBodyType.THIN', profiles: 'qNothing()', wallShape: wireEdges('profile'), path: wireEdges('path'), midplane: 'false', thickness1: mm(1.5),
    thickness2: mm(0), flipWall: 'false', trimEnds: 'false',
  }),
]);
batch.add('loft', 'a surface loft between two arcs', [
  sketch('s1', PLANES.xz), arc('s1', 'arc1', [0, 0], [15, 6], [30, 0]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), arc('s2', 'arc1', [0, 0], [15, 6], [30, 0]), solve('s2'),
  call('loft', 'loft1', {
    bodyType: 'ExtendedToolBodyType.SURFACE',
    wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges('s1')}]) }, { "wireProfileEntities" : qUnion([${wireEdges('s2')}]) }]`,
  }),
]);
batch.add('revolve', 'up to a face', [
  box('base', [-40, 5, -20], [40, 15, 20]),
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [10, -5], [20, 5]), solve('s1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'revolve1', {
    entities: regions('s1'), axis: edgeOf('axis1'), fullRevolve: 'false', endBound: 'RevolveBoundingType.UP_TO_SURFACE',
    endBoundEntityFace: faceAt(base, [0, 5, 0]),
  }),
]);

batch.write('tools/fixtures/gen-batch-006.mjs');
