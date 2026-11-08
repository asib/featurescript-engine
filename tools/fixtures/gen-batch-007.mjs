// Generates fixture batch 007: where surfaces' coordinate systems sit, the selection queries (convex, concave, hole,
// fillet, loop, parallel, matching, bounded faces), and more options of hole, rib, body draft, revolve, split face,
// transform, isocline, name entity and query variable.
//
// Usage: node tools/fixtures/gen-batch-007.mjs
//
// Writes fixtures/batch-007/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, axisLine, box, call, createdBy, cylinder, deg, dir3, edgeAt, edgeOf, faceAt, line, mm, planeAt,
  planeBody, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt,
} from './lib.mjs';

const helpers = `
// A face's surface as numbers: its coordinate system (origin in mm) and size.
function surfaceProbe(context is Context, face is Query) returns map
{
    const s = evSurfaceDefinition(context, { "face" : face });
    var out = {};
    if (s.coordSystem != undefined)
    {
        out.origin = fixtureNumbers(s.coordSystem.origin, millimeter);
        out.z = fixtureNumbers(s.coordSystem.zAxis, 1);
        out.x = fixtureNumbers(s.coordSystem.xAxis, 1);
    }
    if (s.radius != undefined)
        out.radius = s.radius / millimeter;
    if (s.halfAngle != undefined)
        out.halfAngle = s.halfAngle / degree;
    return out;
}

// What a query finds: how many entities, and the total area of its faces and length of its edges.
function queryProbe(context is Context, q is Query) returns map
{
    const faces = qEntityFilter(q, EntityType.FACE);
    const edges = qEntityFilter(q, EntityType.EDGE);
    return {
            "count" : evaluateQueryCount(context, q),
            "area" : isQueryEmpty(context, faces) ? 0 : evArea(context, { "entities" : faces }) / millimeter ^ 2,
            "length" : isQueryEmpty(context, edges) ? 0 : evLength(context, { "entities" : edges }) / millimeter
        };
}
`;

const batch = new Batch({
  number: 7,
  title: 'surface coordinate systems, selection queries, and more options of hole, rib, body draft, revolve, split face, transform, isocline, name entity, query variable',
  helpers,
});

const base = solidsOf('base');
const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const pointVertices = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
const fuse = (name, ...bodies) =>
  call('booleanBodies', name, { operationType: 'BooleanOperationType.UNION', tools: union(...bodies) });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const surface = (face) => `surfaceProbe(context, ${face})`;
const probe = (q) => `queryProbe(context, ${q})`;

// ----- where surfaces' coordinate systems sit -----

batch.add('surface', 'a cylinder extruded up from z = 0 to 20', [
  cylinder('base', [0, 0], 10, 0, 20), returns({ side: surface(faceAt(base, [10, 0, 10])) }),
]);
batch.add('surface', 'a cylinder extruded down 20 from z = 0', [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), `skCircle(s1, "circle1", { "center" : ${vec2(0, 0)}, "radius" : ${mm(10)} });`, solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
  returns({ side: surface(faceAt(base, [10, 0, -10])) }),
]);
batch.add('surface', 'a cylinder extruded 20 symmetrically about z = 0', [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), `skCircle(s1, "circle1", { "center" : ${vec2(0, 0)}, "radius" : ${mm(10)} });`, solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(20), symmetric: 'true' }),
  returns({ side: surface(faceAt(base, [10, 0, 5])) }),
]);
batch.add('surface', 'a cylinder off the axis, extruded up from z = 5 to 20', [
  cylinder('base', [3, 4], 6, 5, 20), returns({ side: surface(faceAt(base, [9, 4, 10])) }),
]);
batch.add('surface', 'the cylinders a revolved rectangle makes', [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), `skRectangle(s1, "rect1", { "firstCorner" : ${vec2(10, 0)}, "secondCorner" : ${vec2(20, 15)} });`, solve('s1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('s1'), axis: edgeOf('axis1'), angle: deg(360) }),
  returns({ outer: surface(faceAt(base, [20, 0, 7])), inner: surface(faceAt(base, [10, 0, 7])), top: surface(faceAt(base, [15, 0, 15])) }),
]);
batch.add('surface', 'fCylinder, fCone and fSphere', [
  call('fCylinder', 'base', { bottomCenter: vec3(40, 0, 0), topCenter: vec3(40, 0, 20), radius: mm(10) }),
  call('fCone', 'cone', { bottomCenter: vec3(80, 0, 0), topCenter: vec3(80, 0, 20), bottomRadius: mm(10), topRadius: mm(5) }),
  call('fSphere', 'ball', { radius: mm(10) }),
  returns({
    cylinder: surface(faceAt(base, [50, 0, 10])),
    cone: surface(faceAt(solidsOf('cone'), [87.5, 0, 10])),
    sphere: surface(faceAt(solidsOf('ball'), [10, 0, 0])),
  }),
]);
batch.add('surface', 'a fillet and a hole in a block', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [0, 0, 6]), radius: mm(4) }),
  sketch('h', planeAt([0, 0, 12], [0, 0, 1], [1, 0, 0])), `skCircle(h, "circle1", { "center" : ${vec2(25, 15)}, "radius" : ${mm(3)} });`, solve('h'),
  call('extrude', 'cut', {
    entities: regions('h'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE',
    defaultScope: 'false', booleanScope: base,
  }),
  returns({ fillet: surface(createdBy('fillet1', 'FACE')), hole: surface(faceAt(base, [28, 15, 6])) }),
]);

// ----- selection queries on a test part: a block with a boss, a pocket, a hole and two fillets -----

const testPart = [
  box('base', [0, 0, 0], [60, 40, 20]),
  cylinder('boss', [15, 20], 5, 20, 28), fuse('union1', base, solidsOf('boss')),
  sketch('p', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), rect('p', 'rect1', [35, 15], [45, 25]), solve('p'),
  call('extrude', 'pocket', {
    entities: regions('p'), endBound: 'BoundingType.BLIND', depth: mm(6), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE',
    defaultScope: 'false', booleanScope: base,
  }),
  sketch('h', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), `skCircle(h, "circle1", { "center" : ${vec2(50, 8)}, "radius" : ${mm(3)} });`, solve('h'),
  call('extrude', 'hole', {
    entities: regions('h'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE',
    defaultScope: 'false', booleanScope: base,
  }),
  call('fillet', 'small', { entities: edgeAt(base, [0, 0, 10]), radius: mm(3) }),
  call('fillet', 'large', { entities: edgeAt(base, [60, 40, 10]), radius: mm(5) }),
];
const bossTop = faceAt(base, [15, 20, 28]);
const pocketFloor = faceAt(base, [40, 20, 14]);
const holeWall = faceAt(base, [53, 8, 10]);
const smallFillet = createdBy('small', 'FACE');
batch.add('query', 'convex, concave and hole faces', [
  testPart,
  returns({
    convex: probe(`qConvexConnectedFaces(${bossTop})`),
    concave: probe(`qConcaveConnectedFaces(${pocketFloor})`),
    hole: probe(`qHoleFaces(${holeWall})`),
  }),
]);
batch.add('query', 'fillet faces and tangent faces', [
  testPart,
  returns({
    equal: probe(`qFilletFaces(${smallFillet}, CompareType.EQUAL)`),
    lessEqual: probe(`qFilletFaces(${smallFillet}, CompareType.LESS_EQUAL)`),
    greaterEqual: probe(`qFilletFaces(${smallFillet}, CompareType.GREATER_EQUAL)`),
    tangent: probe(`qTangentConnectedFaces(${smallFillet})`),
  }),
]);
batch.add('query', 'loop edges, parallel edges and edge convexity', [
  testPart,
  returns({
    loopOfFace: probe(`qLoopEdges(${pocketFloor})`),
    loopOfEdge: probe(`qLoopEdges(${edgeAt(base, [30, 0, 0])})`),
    parallel: probe(`qParallelEdges(${edgeAt(base, [30, 0, 0])})`),
    parallelToZ: probe(`qParallelEdges(qOwnedByBody(${base}, EntityType.EDGE), ${dir3(0, 0, 1)})`),
    convex: probe(`qEdgeConvexityTypeFilter(qOwnedByBody(${base}, EntityType.EDGE), EdgeConvexityType.CONVEX)`),
    concave: probe(`qEdgeConvexityTypeFilter(qOwnedByBody(${base}, EntityType.EDGE), EdgeConvexityType.CONCAVE)`),
  }),
]);
batch.add('query', 'bounded faces, matching faces and planes', [
  testPart,
  returns({
    loopBounded: probe(`qLoopBoundedFaces(qUnion([${faceAt(base, [30, 0, 10])}, ${edgeAt(base, [30, 0, 20])}]))`),
    bounded: probe(`qFaceOrEdgeBoundedFaces(qUnion([${faceAt(base, [30, 30, 20])}, ${edgeAt(base, [40, 15, 20])}]))`),
    matching: probe(`qMatching(${faceAt(base, [40, 15, 17])})`),
    parallelPlanes: probe(`qParallelPlanes(qOwnedByBody(${base}, EntityType.FACE), ${dir3(0, 0, 1)}, true)`),
    inFront: probe(`qInFrontOfPlane(qOwnedByBody(${base}, EntityType.FACE), plane(${vec3(0, 0, 15)}, ${dir3(0, 0, 1)}))`),
  }),
]);
batch.add('query-variable', 'the fillets of the test part, read back', [
  testPart,
  call('queryVariable', 'qv1', {
    name: '"fillets"', description: '""', selectionType: 'SelectionType.FILLETS', seedFaces: smallFillet, filletCompareType: 'FilletCompare.GREATER_EQUAL',
    addAdditionalQueries: 'false', evaluateOnUse: 'false', showSelection: 'false',
  }),
  returns({ fillets: probe('getQueryVariable(context, "fillets")') }),
]);
batch.add('name-entity', 'a named face, found again by name', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('nameEntity', 'name1', { entity: faceAt(base, [20, 15, 12]), entityName: '"top"' }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 0, 12]), radius: mm(3) }),
  returns({ named: probe('qNamed("top")') }),
]);

// ----- more options of features matched before -----

const pts = (points, z = 20) => [
  sketch('pts', planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])),
  points.map(([x, y], i) => `skPoint(pts, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve('pts'),
];
const twoBlocks = (gap) => [
  box('base', [0, 0, 10 + gap], [40, 30, 20 + gap]), box('lower', [0, 0, 0], [40, 30, 10]),
];
batch.add('hole', 'up to next: through a block into a gap', [
  box('base', [0, 0, 12], [40, 30, 20]), box('lower', [0, 0, 0], [40, 30, 6]), pts([[20, 15]]),
  call('hole', 'hole1', {
    locations: pointVertices('pts'), scope: union(base, solidsOf('lower')), style: 'HoleStyle.SIMPLE', holeDiameter: mm(6),
    endStyle: 'HoleEndStyle.UP_TO_NEXT', offset: 'false', holeDepthComputed: mm(10),
  }),
]);
batch.add('hole', 'up to a face of another block', [
  box('base', [0, 0, 12], [40, 30, 20]), box('lower', [0, 0, 0], [40, 30, 6]), pts([[20, 15]]),
  call('hole', 'hole1', {
    locations: pointVertices('pts'), scope: union(base, solidsOf('lower')), style: 'HoleStyle.SIMPLE', holeDiameter: mm(6),
    endStyle: 'HoleEndStyle.UP_TO_ENTITY', endBoundEntity: faceAt(solidsOf('lower'), [20, 15, 6]), offset: 'false', holeDepthComputed: mm(10),
  }),
]);
batch.add('hole', 'blind in the last of two stacked blocks', [
  twoBlocks(0), pts([[20, 15]]),
  call('hole', 'hole1', {
    locations: pointVertices('pts'), scope: union(base, solidsOf('lower')), style: 'HoleStyle.SIMPLE', holeDiameter: mm(6),
    tapDrillDiameter: mm(4), holeDepth: mm(5), endStyle: 'HoleEndStyle.BLIND_IN_LAST',
  }),
]);
batch.add('rib', 'normal to its sketch plane, down onto a plate', [
  box('base', [0, 0, 0], [40, 30, 4]), box('wall', [0, 0, 4], [4, 30, 24]), fuse('union1', base, solidsOf('wall')),
  sketch('profile', planeAt([0, 0, 24], [0, 0, 1], [1, 0, 0])), line('profile', 'line1', [4, 15], [36, 15]), solve('profile'),
  call('rib', 'rib1', {
    profiles: wireEdges('profile'), parts: base, thickness: mm(3), ribExtrusionDirection: 'RibExtrusionDirection.NORMAL_TO_SKETCH_PLANE',
    oppositeDirection: 'true',
  }),
]);
batch.add('body-draft', 'by top edges, from the bottom', [
  box('base', [0, 0, 0], [40, 30, 12]), axisLine('pull', [0, 0, 0], [0, 0, 1]),
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.EDGES', topEdges: union(edgeAt(base, [20, 0, 12]), edgeAt(base, [40, 15, 12])),
    pullDirection: edgeOf('pull'), partingObject: faceAt(base, [20, 15, 0]), angle: deg(5),
  }),
]);
batch.add('body-draft', 'a whole part, from a plane through its middle', [
  box('base', [0, 0, 0], [40, 30, 12]), axisLine('pull', [0, 0, 0], [0, 0, 1]), planeBody('mid', [20, 15, 6], [0, 0, 1], [1, 0, 0]),
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.PARTS', bodies: base, excludeFaces: 'qNothing()',
    pullDirection: edgeOf('pull'), partingObject: createdBy('mid', 'FACE'), angle: deg(3),
  }),
]);
const revolveProfile = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [10, -5], [20, 5]), solve('s1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
];
batch.add('revolve', 'up to a part', [
  box('base', [-40, 5, -20], [40, 15, 20]), revolveProfile,
  call('revolve', 'revolve1', { entities: regions('s1'), axis: edgeOf('axis1'), fullRevolve: 'false', endBound: 'RevolveBoundingType.UP_TO_BODY', endBoundEntityBody: base }),
]);
batch.add('revolve', 'up to a vertex', [
  box('base', [-40, 30, -20], [-30, 40, 20]), revolveProfile,
  call('revolve', 'revolve1', {
    entities: regions('s1'), axis: edgeOf('axis1'), fullRevolve: 'false', endBound: 'RevolveBoundingType.UP_TO_VERTEX',
    endBoundEntityVertex: vertexAt(base, [-30, 30, 20]),
  }),
]);
batch.add('revolve', 'up to a face, 20 degrees short', [
  box('base', [-40, 5, -20], [40, 15, 20]), revolveProfile,
  call('revolve', 'revolve1', {
    entities: regions('s1'), axis: edgeOf('axis1'), fullRevolve: 'false', endBound: 'RevolveBoundingType.UP_TO_SURFACE',
    endBoundEntityFace: faceAt(base, [0, 5, 0]), endBoundHasOffset: 'true', endBoundOffset: deg(20), endBoundOffsetFlip: 'false',
  }),
]);
batch.add('split-face', 'the top of a block split by a plane', [
  box('base', [0, 0, 0], [40, 30, 12]), planeBody('cut', [15, 15, 6], [1, 0.5, 0], [0, 0, 1]),
  call('splitPart', 'split1', { splitType: 'SplitType.FACE', faceTargets: faceAt(base, [20, 15, 12]), faceTools: createdBy('cut', 'FACE') }),
]);
batch.add('split-face', 'two faces of a block split by a face of a cylinder', [
  box('base', [0, 0, 0], [40, 30, 12]), cylinder('tool', [40, 30], 10, -5, 20),
  call('splitPart', 'split1', {
    splitType: 'SplitType.FACE', faceTargets: union(faceAt(base, [20, 15, 12]), faceAt(base, [40, 15, 6])),
    faceTools: faceAt(solidsOf('tool'), [30, 30, 5]), keepToolSurfaces: 'true',
  }),
]);
batch.add('transform', 'a block scaled differently along each axis about a corner', [
  box('base', [0, 0, 0], [20, 10, 5]),
  call('transform', 'scale1', {
    entities: base, transformType: 'TransformType.SCALE_UNIFORMLY', uniform: 'false', scaleX: '2', scaleY: '0.5', scaleZ: '3',
    scalePoint: vertexAt(base, [0, 0, 0]), makeCopy: 'false', oppositeDirectionEntity: 'false',
  }),
]);
batch.add('isocline', 'a sphere split at 30 degrees from up', [
  call('fSphere', 'base', { radius: mm(10) }), axisLine('up', [0, 0, 0], [0, 0, 1]),
  call('isocline', 'iso1', { faces: `qOwnedByBody(${base}, EntityType.FACE)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(30), split: 'true' }),
]);
batch.add('isocline', 'the 20 degree isoclines of a torus-like revolve, as curves', [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), `skCircle(s1, "circle1", { "center" : ${vec2(20, 0)}, "radius" : ${mm(5)} });`, solve('s1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('s1'), axis: edgeOf('axis1'), angle: deg(360) }), axisLine('up', [0, 0, 0], [0, 0, 1]),
  call('isocline', 'iso1', { faces: `qOwnedByBody(${base}, EntityType.FACE)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(20), split: 'false' }),
]);

batch.write('tools/fixtures/gen-batch-007.mjs');
