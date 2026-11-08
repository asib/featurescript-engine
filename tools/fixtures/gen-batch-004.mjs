// Generates fixture batch 004: sketch curves, surface and thin extrudes, extrude bounds, sweep, loft, helix and
// fit spline.
//
// Usage: node tools/fixtures/gen-batch-004.mjs
//
// Writes fixtures/batch-004/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, circle, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, fsMap, line,
  mm, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt,
} from './lib.mjs';

const batch = new Batch({
  number: 4,
  title: 'sketch curves, surface and thin extrudes, extrude bounds, sweep, loft, helix, fit spline',
});

const base = solidsOf('base');
const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const extrudeRegions = (s, depth, extra = {}) =>
  call('extrude', 'extrude1', { entities: regions(s), endBound: 'BoundingType.BLIND', depth: mm(depth), ...extra });
const points2 = (points) => `[${points.map(([x, y]) => vec2(x, y)).join(', ')}]`;

// ----- sketch curves, extruded so their shapes are measured -----

const curveCase = (description, entities, depth = 6) =>
  batch.add('sketch-curve', description, [sketch('sketch1', PLANES.xy), entities, solve('sketch1'), extrudeRegions('sketch1', depth)]);
curveCase('ellipse', `skEllipse(sketch1, "ellipse1", ${fsMap({ center: vec2(0, 0), majorRadius: mm(20), minorRadius: mm(9) })});`);
curveCase('ellipse at an angle', `skEllipse(sketch1, "ellipse1", ${fsMap({ center: vec2(5, -3), majorRadius: mm(15), minorRadius: mm(6), majorAxis: 'normalize(vector(1, 1))' })});`);
curveCase('elliptical arc closed by a line', [
  `skEllipticalArc(sketch1, "arc1", ${fsMap({ center: vec2(0, 0), majorAxis: 'vector(1, 0)', majorRadius: mm(20), minorRadius: mm(10), startParameter: '0', endParameter: '0.5' })});`,
  line('sketch1', 'line1', [-20, 0], [20, 0]),
]);
curveCase('closed fit spline', `skFitSpline(sketch1, "spline1", { "points" : ${points2([[0, 0], [20, -5], [30, 10], [10, 25], [-5, 12], [0, 0]])} });`);
curveCase('open fit spline closed by a line', [
  `skFitSpline(sketch1, "spline1", { "points" : ${points2([[0, 0], [10, 12], [25, 8], [35, 0]])} });`,
  line('sketch1', 'line1', [35, 0], [0, 0]),
]);
curveCase('fit spline with end derivatives', [
  `skFitSpline(sketch1, "spline1", { "points" : ${points2([[0, 0], [15, 10], [30, 0]])}, "startDerivative" : ${vec2(0, 30)}, "endDerivative" : ${vec2(0, -30)} });`,
  line('sketch1', 'line1', [30, 0], [0, 0]),
]);
curveCase('Bezier closed by a line', [
  `skBezier(sketch1, "bezier1", { "points" : ${points2([[0, 0], [5, 25], [25, 25], [30, 0]])} });`,
  line('sketch1', 'line1', [30, 0], [0, 0]),
]);
curveCase('conic segment, rho 0.3, closed by lines', [
  `skConicSegment(sketch1, "conic1", ${fsMap({ start: vec2(20, 0), controlPoint: vec2(0, 0), end: vec2(0, 20), rho: '0.3' })});`,
  line('sketch1', 'line1', [0, 20], [-5, 20]), line('sketch1', 'line2', [-5, 20], [-5, -5]),
  line('sketch1', 'line3', [-5, -5], [20, -5]), line('sketch1', 'line4', [20, -5], [20, 0]),
]);
curveCase('conic segment, rho 0.7', [
  `skConicSegment(sketch1, "conic1", ${fsMap({ start: vec2(20, 0), controlPoint: vec2(0, 0), end: vec2(0, 20), rho: '0.7' })});`,
  line('sketch1', 'line1', [0, 20], [20, 20]), line('sketch1', 'line2', [20, 20], [20, 0]),
]);
curveCase('regular hexagon', `skRegularPolygon(sketch1, "polygon1", ${fsMap({ center: vec2(0, 0), firstVertex: vec2(15, 4), sides: '6' })});`);
curveCase('polyline', `skPolyline(sketch1, "polyline1", { "points" : ${points2([[0, 0], [30, 0], [30, 10], [12, 18], [0, 10], [0, 0]])} });`);

// What a solved sketch is made of: how many bodies of each type, and what qCreatedBy finds.
batch.add('sketch-topology', 'the bodies and entities of a sketch', [
  sketch('sketch1', PLANES.xy),
  rect('sketch1', 'rect1', [0, 0], [20, 10]),
  line('sketch1', 'line1', [30, 0], [40, 5]),
  line('sketch1', 'line2', [40, 5], [50, 0]),
  circle('sketch1', 'circle1', [60, 5], 4),
  line('sketch1', 'line3', [0, 20], [20, 20], true),
  `skPoint(sketch1, "point1", { "position" : ${vec2(10, 30)} });`,
  solve('sketch1'),
  `const all = qCreatedBy(id + "sketch1", EntityType.BODY);`,
  `var counts = {};`,
  `for (var bodyType in [BodyType.SOLID, BodyType.SHEET, BodyType.WIRE, BodyType.POINT])`,
  `    counts[toString(bodyType)] = evaluateQueryCount(context, qBodyType(all, bodyType));`,
  `counts.construction = evaluateQueryCount(context, qConstructionFilter(all, ConstructionObject.YES));`,
  `counts.faces = evaluateQueryCount(context, qCreatedBy(id + "sketch1", EntityType.FACE));`,
  `counts.edges = evaluateQueryCount(context, qCreatedBy(id + "sketch1", EntityType.EDGE));`,
  `counts.vertices = evaluateQueryCount(context, qCreatedBy(id + "sketch1", EntityType.VERTEX));`,
  `counts.regions = evaluateQueryCount(context, qSketchRegion(id + "sketch1"));`,
  `counts.wireEdges = evaluateQueryCount(context, qOwnedByBody(qBodyType(all, BodyType.WIRE), EntityType.EDGE));`,
  `counts.sheetEdges = evaluateQueryCount(context, qOwnedByBody(qBodyType(all, BodyType.SHEET), EntityType.EDGE));`,
  `counts.wireVertices = evaluateQueryCount(context, qOwnedByBody(qBodyType(all, BodyType.WIRE), EntityType.VERTEX));`,
  `counts.line1 = evaluateQueryCount(context, sketchEntityQuery(id + "sketch1", EntityType.EDGE, "line1"));`,
  `counts.rectTop = evaluateQueryCount(context, sketchEntityQuery(id + "sketch1", EntityType.EDGE, "rect1.top"));`,
  `return counts;`,
]);

// ----- surfaces and thin walls from sketch edges -----

// A sketch's edges as wire edges: qCreatedBy on a sketch also finds the edges of its regions.
const edges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const surfaceCase = (description, entities, fields) =>
  batch.add('surface-extrude', description, [
    sketch('sketch1', PLANES.xy), entities, solve('sketch1'),
    call('extrude', 'extrude1', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: edges('sketch1'), endBound: 'BoundingType.BLIND', depth: mm(12), ...fields }),
  ]);
surfaceCase('a line extruded as a surface', line('sketch1', 'line1', [0, 0], [25, 10]), {});
surfaceCase('an arc extruded as a surface', arc('sketch1', 'arc1', [0, 0], [10, 8], [20, 0]), {});
surfaceCase('a circle extruded as a surface, symmetric', circle('sketch1', 'circle1', [0, 0], 8), { endBound: 'BoundingType.BLIND', symmetric: 'true' });
surfaceCase('an open polyline extruded as a surface', polygon('sketch1', [[0, 0], [15, 0], [15, 10]]).slice(0, 2), {});
batch.add('surface-revolve', 'a line revolved as a surface', [
  sketch('sketch1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('sketch1', 'line1', [10, 0], [20, 15]), solve('sketch1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'revolve1', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: edges('sketch1'), axis: edgeOf('axis1') }),
]);
batch.add('surface-revolve', 'an arc revolved 180 degrees as a surface', [
  sketch('sketch1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('sketch1', 'arc1', [0, 15], [15, 0], [0, -15]), solve('sketch1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'revolve1', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: edges('sketch1'), axis: edgeOf('axis1'), fullRevolve: 'false', angle: deg(180),
  }),
]);
const thinCase = (description, entities, fields) =>
  batch.add('thin-extrude', description, [
    sketch('sketch1', PLANES.xy), entities, solve('sketch1'),
    call('extrude', 'extrude1', {
      bodyType: 'ExtendedToolBodyType.THIN', wallShape: edges('sketch1'), endBound: 'BoundingType.BLIND', depth: mm(10),
      midplane: 'false', thickness1: mm(2), thickness2: mm(0), flipWall: 'false', ...fields,
    }),
  ]);
thinCase('a thin wall from an open polyline', polygon('sketch1', [[0, 0], [25, 0], [25, 15]]).slice(0, 2), {});
thinCase('a thin wall from a rectangle, flipped', rect('sketch1', 'rect1', [0, 0], [30, 20]), { flipWall: 'true' });
thinCase('a thin midplane wall from a circle', circle('sketch1', 'circle1', [0, 0], 10), { midplane: 'true', thickness: mm(3) });

// ----- extrude bounds and draft -----

const target = [box('base', [-20, -20, 20], [20, 20, 28])];
const extrudeTo = (description, fields, setup = target) =>
  batch.add('extrude-bound', description, [
    setup, sketch('sketch1', PLANES.xy), rect('sketch1', 'rect1', [-5, -5], [5, 5]), solve('sketch1'),
    call('extrude', 'extrude1', { entities: regions('sketch1'), ...fields }),
  ]);
extrudeTo('up to a face', { endBound: 'BoundingType.UP_TO_SURFACE', endBoundEntityFace: faceAt(base, [0, 0, 20]) });
extrudeTo('up to a face with an offset', {
  endBound: 'BoundingType.UP_TO_SURFACE', endBoundEntityFace: faceAt(base, [0, 0, 28]), hasOffset: 'true', offsetDistance: mm(3),
  offsetOppositeDirection: 'false',
});
extrudeTo('up to a body', { endBound: 'BoundingType.UP_TO_BODY', endBoundEntityBody: base });
extrudeTo('up to a vertex', { endBound: 'BoundingType.UP_TO_VERTEX', endBoundEntityVertex: vertexAt(base, [20, 20, 28]) });
extrudeTo('up to next, added', {
  endBound: 'BoundingType.UP_TO_NEXT', operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base,
});
extrudeTo('through all, removed', {
  endBound: 'BoundingType.THROUGH_ALL', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
}, [box('base', [-20, -20, 5], [20, 20, 13])]);
extrudeTo('blind with a 10 degree draft', { endBound: 'BoundingType.BLIND', depth: mm(15), hasDraft: 'true', draftAngle: deg(10), draftPullDirection: 'false' });
extrudeTo('blind with an inward 5 degree draft', { endBound: 'BoundingType.BLIND', depth: mm(15), hasDraft: 'true', draftAngle: deg(5), draftPullDirection: 'true' });

// ----- sweep -----

const sweepCase = (description, profile, path, fields = {}) =>
  batch.add('sweep', description, [
    profile, path,
    call('sweep', 'sweep1', { profiles: regions('profile'), path: edges('path'), ...fields }),
  ]);
const circleProfile = (r) => [sketch('profile', PLANES.yz), circle('profile', 'circle1', [0, 0], r), solve('profile')];
const rectProfile = [sketch('profile', PLANES.yz), rect('profile', 'rect1', [-3, -2], [3, 2]), solve('profile')];
const straightPath = [sketch('path', PLANES.xy), line('path', 'line1', [0, 0], [40, 0]), solve('path')];
const arcPath = [sketch('path', PLANES.xy), arc('path', 'arc1', [0, 0], [20, 8.28427], [28.28427, 28.28427]), solve('path')];
const lPath = [
  sketch('path', PLANES.xy), line('path', 'line1', [0, 0], [20, 0]), arc('path', 'arc1', [20, 0], [27.07107, 2.92893], [30, 10]),
  line('path', 'line2', [30, 10], [30, 30]), solve('path'),
];
sweepCase('a circle along a line', circleProfile(4), straightPath);
sweepCase('a circle along an arc', circleProfile(3), arcPath);
sweepCase('a rectangle along a line, an arc and a line', rectProfile, lPath);
sweepCase('a circle along a line, an arc and a line', circleProfile(2), lPath);
batch.add('sweep', 'an edge swept along an arc as a surface', [
  sketch('profile', PLANES.yz), line('profile', 'line1', [-4, 0], [4, 0]), solve('profile'), arcPath,
  call('sweep', 'sweep1', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceProfiles: edges('profile'), path: edges('path') }),
]);

// ----- loft -----

const loftCase = (description, profiles, fields = {}) =>
  batch.add('loft', description, [
    profiles.map(([name, plane, body]) => [sketch(name, plane), body(name), solve(name)]),
    call('loft', 'loft1', {
      sheetProfilesArray: `[${profiles.map(([name]) => `{ "sheetProfileEntities" : ${regions(name)} }`).join(', ')}]`,
      ...fields,
    }),
  ]);
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
loftCase('square to a smaller square', [
  ['s1', at(0), (s) => rect(s, 'rect1', [-10, -10], [10, 10])],
  ['s2', at(20), (s) => rect(s, 'rect1', [-5, -5], [5, 5])],
]);
loftCase('square to an offset square', [
  ['s1', at(0), (s) => rect(s, 'rect1', [-10, -10], [10, 10])],
  ['s2', at(25), (s) => rect(s, 'rect1', [0, -5], [10, 5])],
]);
loftCase('circle to a smaller circle', [
  ['s1', at(0), (s) => circle(s, 'circle1', [0, 0], 12)],
  ['s2', at(30), (s) => circle(s, 'circle1', [0, 0], 6)],
]);
loftCase('circle to circle to circle', [
  ['s1', at(0), (s) => circle(s, 'circle1', [0, 0], 10)],
  ['s2', at(15), (s) => circle(s, 'circle1', [0, 0], 14)],
  ['s3', at(30), (s) => circle(s, 'circle1', [0, 0], 8)],
]);
loftCase('square to a circle', [
  ['s1', at(0), (s) => rect(s, 'rect1', [-10, -10], [10, 10])],
  ['s2', at(25), (s) => circle(s, 'circle1', [0, 0], 8)],
]);

// ----- helix -----

const helixCase = (description, setup, fields) =>
  batch.add('helix', description, [setup, call('helix', 'helix1', { startAngle: deg(0), handedness: 'Direction.CCW', ...fields })]);
const can = cylinder('base', [0, 0], 10, 0, 30);
helixCase('around a cylinder, 4 turns', can, {
  axisType: 'AxisType.SURFACE', entities: faceAt(base, [10, 0, 15]), pathType: 'PathType.TURNS', revolutions: '4',
});
helixCase('around a cylinder, 5 mm pitch, clockwise', can, {
  axisType: 'AxisType.SURFACE', entities: faceAt(base, [10, 0, 15]), pathType: 'PathType.PITCH', helicalPitch: mm(5), handedness: 'Direction.CW',
});
helixCase('about an axis, 3 turns, 20 mm high, radius 8', axisLine('axis1', [0, 0, 0], [0, 0, 20]), {
  axisType: 'AxisType.AXIS', axis: edgeOf('axis1'), pathType: 'PathType.TURNS', revolutions: '3', height: mm(20), startRadius: mm(8),
});
helixCase('about an axis, tapering from radius 10 to 4', axisLine('axis1', [0, 0, 0], [0, 0, 20]), {
  axisType: 'AxisType.AXIS', axis: edgeOf('axis1'), pathType: 'PathType.TURNS', revolutions: '2.5', height: mm(20), startRadius: mm(10),
  endRadToggle: 'true', endRadius: mm(4),
});

// ----- fit spline -----

batch.add('fit-spline', 'through four vertices of a box', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fitSpline', 'spline1', {
    vertices: union(vertexAt(base, [0, 0, 0]), vertexAt(base, [30, 0, 0]), vertexAt(base, [30, 20, 10]), vertexAt(base, [0, 20, 10])),
  }),
]);
batch.add('fit-spline', 'closed through three vertices', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fitSpline', 'spline1', {
    vertices: union(vertexAt(base, [0, 0, 0]), vertexAt(base, [30, 0, 0]), vertexAt(base, [30, 20, 10])), closed: 'true',
  }),
]);

batch.write('tools/fixtures/gen-batch-004.mjs');
