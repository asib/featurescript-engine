// Generates fixture batch 002: revolve, primitives, boolean, transform, mirror, patterns, split, delete, planes,
// points and variables.
//
// Usage: node tools/fixtures/gen-batch-002.mjs
//
// Writes fixtures/batch-002/{cases.fs,generator.fs,cases.json}. The cases are written out by hand, so rerunning
// gives the same files.

import {
  Batch, PLANES, axisLine, box, call, createdBy, cylinder, deg, dir3, edgeAt, edgeOf, faceAt, line, arc, circle,
  mm, planeAt, planeBody, polygon, rect, regions, sketch, solidsOf, solve, vec3, vertexAt,
} from './lib.mjs';

const batch = new Batch({
  number: 2,
  title: 'revolve, primitives, boolean, transform, mirror, patterns, split, delete, planes, points, variables',
});

const scope = (name) => ({ defaultScope: 'false', booleanScope: solidsOf(name) });

// ----- revolve -----

// A profile on the xz plane (sketch x = world x, sketch y = world z), revolved about the world z axis.
const XZ_UP = planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0]);
const zAxis = (name) => axisLine(name, [0, 0, -50], [0, 0, 50]);

function revolveCase(description, profile, fields, extra = []) {
  batch.add('revolve', description, [
    sketch('sketch1', XZ_UP),
    profile('sketch1'),
    solve('sketch1'),
    zAxis('axis1'),
    extra,
    call('revolve', 'revolve1', { entities: regions('sketch1'), axis: edgeOf('axis1'), ...fields }),
  ]);
}

const ring = (s) => rect(s, 'rect1', [10, 0], [25, 15]);
revolveCase('rectangle away from the axis, full revolve: a ring', ring, {});
revolveCase('rectangle touching the axis, full revolve: a cylinder', (s) => rect(s, 'rect1', [0, 0], [20, 30]), {});
revolveCase('ring profile, blind 90 degrees', ring, { fullRevolve: 'false', angle: deg(90) });
revolveCase('ring profile, blind 120 degrees opposite', ring, {
  fullRevolve: 'false', angle: deg(120), oppositeDirection: 'true',
});
revolveCase('ring profile, symmetric 90 degrees', ring, { fullRevolve: 'false', angle: deg(90), symmetric: 'true' });
revolveCase('ring profile, two directions 60 and 45 degrees', ring, {
  fullRevolve: 'false', angle: deg(60), hasStartBound: 'true', startBound: 'RevolveBoundingType.BLIND', angleBack: deg(45),
});
revolveCase('circle away from the axis, full revolve: a torus', (s) => circle(s, 'circle1', [30, 0], 8), {});
revolveCase('circle away from the axis, 180 degrees', (s) => circle(s, 'circle1', [30, 0], 8), {
  fullRevolve: 'false', angle: deg(180),
});
revolveCase('right triangle on the axis, full revolve: a cone', (s) => polygon(s, [[0, 0], [20, 0], [0, 35]]), {});
revolveCase('trapezoid on the axis, full revolve: a frustum', (s) => polygon(s, [[0, 0], [25, 0], [12, 20], [0, 20]]), {});
revolveCase('semicircle on the axis, full revolve: a sphere', (s) => [
  line(s, 'line1', [0, -20], [0, 20]),
  arc(s, 'arc1', [0, 20], [20, 0], [0, -20]),
], {});
revolveCase('slanted quadrilateral, 270 degrees', (s) => polygon(s, [[8, -5], [30, 0], [26, 18], [12, 12]]), {
  fullRevolve: 'false', angle: deg(270),
});
revolveCase('ring profile cut from a box', (s) => rect(s, 'rect1', [15, -5], [25, 5]), {
  operationType: 'NewBodyOperationType.REMOVE', ...scope('base'),
}, box('base', [-30, -30, -10], [30, 30, 10]));
revolveCase('ring profile added to a cylinder', ring, {
  operationType: 'NewBodyOperationType.ADD', ...scope('base'),
}, cylinder('base', [0, 0], 12, -20, 20));
revolveCase('ring profile intersected with a box', ring, {
  operationType: 'NewBodyOperationType.INTERSECT', ...scope('base'),
}, box('base', [0, -40, 2], [40, 40, 10]));

// ----- primitives -----

batch.add('primitive', 'cube, 25 mm', [call('cube', 'cube1', { sideLength: mm(25) })]);
batch.add('primitive', 'cuboid between two corners', [
  call('fCuboid', 'cuboid1', { corner1: vec3(-10, 5, 2), corner2: vec3(30, -20, 17) }),
]);
batch.add('primitive', 'cylinder along z', [
  call('fCylinder', 'cylinder1', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 40), radius: mm(12) }),
]);
batch.add('primitive', 'cylinder along a slanted axis', [
  call('fCylinder', 'cylinder1', { bottomCenter: vec3(-10, -5, 0), topCenter: vec3(20, 15, 25), radius: mm(7.5) }),
]);
batch.add('primitive', 'cone frustum', [
  call('fCone', 'cone1', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 30), bottomRadius: mm(15), topRadius: mm(6) }),
]);
batch.add('primitive', 'cone to a point, slanted', [
  call('fCone', 'cone1', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(10, 20, 30), bottomRadius: mm(12), topRadius: mm(0) }),
]);
batch.add('primitive', 'sphere at the origin', [call('fSphere', 'sphere1', { radius: mm(18) })]);
batch.add('primitive', 'sphere centred on a vertex', [
  box('base', [0, 0, 0], [10, 10, 10]),
  call('fSphere', 'sphere1', { center: vertexAt(solidsOf('base'), [10, 10, 10]), radius: mm(6) }),
]);
batch.add('primitive', 'ellipsoid', [
  call('fEllipsoid', 'ellipsoid1', { center: vec3(0, 0, 0), radius: vec3(20, 12, 8) }),
]);

// ----- boolean -----

const twoBoxes = [box('a', [0, 0, 0], [30, 20, 10]), box('b', [15, 10, 5], [45, 35, 25])];
const tools = (...names) => `qUnion([${names.map(solidsOf).join(', ')}])`;
batch.add('boolean', 'union of two overlapping boxes', [
  twoBoxes,
  call('booleanBodies', 'boolean1', { operationType: 'BooleanOperationType.UNION', tools: tools('a', 'b') }),
]);
batch.add('boolean', 'union of two boxes that do not touch: both stay', [
  box('a', [0, 0, 0], [10, 10, 10]), box('b', [20, 0, 0], [30, 10, 10]),
  call('booleanBodies', 'boolean1', { operationType: 'BooleanOperationType.UNION', tools: tools('a', 'b') }),
]);
batch.add('boolean', 'union of three bodies', [
  twoBoxes, cylinder('c', [0, 0], 8, -5, 15),
  call('booleanBodies', 'boolean1', { operationType: 'BooleanOperationType.UNION', tools: tools('a', 'b', 'c') }),
]);
batch.add('boolean', 'subtract a cylinder from a box', [
  box('a', [-20, -20, 0], [20, 20, 15]), cylinder('b', [5, 0], 8, -5, 25),
  call('booleanBodies', 'boolean1', {
    operationType: 'BooleanOperationType.SUBTRACTION', targets: solidsOf('a'), tools: solidsOf('b'),
  }),
]);
batch.add('boolean', 'subtract keeping the tool', [
  box('a', [-20, -20, 0], [20, 20, 15]), cylinder('b', [20, 20], 10, -5, 25),
  call('booleanBodies', 'boolean1', {
    operationType: 'BooleanOperationType.SUBTRACTION', targets: solidsOf('a'), tools: solidsOf('b'), keepTools: 'true',
  }),
]);
batch.add('boolean', 'subtract one tool from two targets', [
  box('a', [0, 0, 0], [20, 20, 10]), box('b', [30, 0, 0], [50, 20, 10]), box('c', [15, 5, -5], [35, 15, 15]),
  call('booleanBodies', 'boolean1', {
    operationType: 'BooleanOperationType.SUBTRACTION', targets: tools('a', 'b'), tools: solidsOf('c'),
  }),
]);
batch.add('boolean', 'subtract that splits the target in two', [
  box('a', [0, 0, 0], [40, 10, 10]), box('b', [15, -5, -5], [25, 15, 15]),
  call('booleanBodies', 'boolean1', {
    operationType: 'BooleanOperationType.SUBTRACTION', targets: solidsOf('a'), tools: solidsOf('b'),
  }),
]);
batch.add('boolean', 'intersect a box and a cylinder', [
  box('a', [-20, -20, 0], [20, 20, 15]), cylinder('b', [10, 10], 18, -5, 25),
  call('booleanBodies', 'boolean1', { operationType: 'BooleanOperationType.INTERSECTION', tools: tools('a', 'b') }),
]);

// ----- transform -----

const block = box('base', [0, 0, 0], [20, 10, 5]);
const base = solidsOf('base');
// The transform feature has no default for makeCopy.
const transformCase = (description, fields, extra = []) =>
  batch.add('transform', description, [
    block, extra, call('transform', 'transform1', { entities: base, makeCopy: 'false', ...fields }),
  ]);

transformCase('translate by xyz', { transformType: 'TransformType.TRANSLATION_3D', dx: mm(5), dy: mm(-12), dz: mm(30) });
transformCase('translate by xyz, keeping a copy', {
  transformType: 'TransformType.TRANSLATION_3D', dx: mm(25), dy: mm(0), dz: mm(0), makeCopy: 'true',
});
transformCase('translate by distance along an edge', {
  transformType: 'TransformType.TRANSLATION_DISTANCE', transformDirection: edgeOf('dir1'), distance: mm(17),
}, axisLine('dir1', [0, 0, 0], [1, 2, 2]));
transformCase('translate by distance, opposite', {
  transformType: 'TransformType.TRANSLATION_DISTANCE', transformDirection: edgeOf('dir1'), distance: mm(17),
  oppositeDirection: 'true',
}, axisLine('dir1', [0, 0, 0], [1, 2, 2]));
transformCase('translate by a line', {
  transformType: 'TransformType.TRANSLATION_ENTITY', transformLine: edgeOf('dir1'), oppositeDirectionEntity: 'false',
}, axisLine('dir1', [5, 5, 5], [15, 25, 35]));
transformCase('rotate 30 degrees about z', {
  transformType: 'TransformType.ROTATION', transformAxis: edgeOf('axis1'), angle: deg(30),
}, zAxis('axis1'));
transformCase('rotate 135 degrees about a slanted axis, keeping a copy', {
  transformType: 'TransformType.ROTATION', transformAxis: edgeOf('axis1'), angle: deg(135), makeCopy: 'true',
}, axisLine('axis1', [30, 0, 0], [30, 10, 20]));
transformCase('rotate opposite', {
  transformType: 'TransformType.ROTATION', transformAxis: edgeOf('axis1'), angle: deg(30), oppositeDirection: 'true',
}, zAxis('axis1'));
transformCase('copy in place', { transformType: 'TransformType.COPY' });
transformCase('scale by 2 about a vertex', {
  transformType: 'TransformType.SCALE_UNIFORMLY', scale: '2', scalePoint: vertexAt(base, [20, 10, 5]),
});
transformCase('scale by 0.5 about a vertex', {
  transformType: 'TransformType.SCALE_UNIFORMLY', scale: '0.5', scalePoint: vertexAt(base, [0, 0, 0]),
});
batch.add('transform', 'copy part', [block, call('copyPart', 'copy1', { entities: base })]);

// ----- mirror -----

const mirrorPlane = planeBody('mirrorPlane', [30, 0, 0], [1, 0, 0], [0, 1, 0]);
batch.add('mirror', 'mirror a block across a plane', [
  block, mirrorPlane,
  call('mirror', 'mirror1', { entities: base, mirrorPlane: createdBy('mirrorPlane', 'FACE') }),
]);
batch.add('mirror', 'mirror a cylinder across a slanted plane', [
  cylinder('base', [0, 0], 6, 0, 20),
  planeBody('mirrorPlane', [10, 0, 0], [1, 0, 1], [0, 1, 0]),
  call('mirror', 'mirror1', { entities: base, mirrorPlane: createdBy('mirrorPlane', 'FACE') }),
]);
batch.add('mirror', 'mirror across a face of the part and add: one body', [
  block,
  call('mirror', 'mirror1', {
    entities: base, mirrorPlane: faceAt(base, [20, 5, 2.5]), operationType: 'NewBodyOperationType.ADD', ...scope('base'),
  }),
]);
batch.add('mirror', 'mirror two bodies', [
  box('a', [0, 0, 0], [10, 10, 10]), cylinder('b', [5, 20], 4, 0, 15), mirrorPlane,
  call('mirror', 'mirror1', { entities: tools('a', 'b'), mirrorPlane: createdBy('mirrorPlane', 'FACE') }),
]);

// ----- linear pattern -----

const xDir = axisLine('dirX', [0, 0, 0], [1, 0, 0]);
const yDir = axisLine('dirY', [0, 0, 0], [0, 1, 0]);
const linear = (description, fields, extra = []) =>
  batch.add('linear-pattern', description, [
    block, xDir, extra,
    call('linearPattern', 'pattern1', { entities: base, directionOne: edgeOf('dirX'), ...fields }),
  ]);
linear('3 instances 30 mm apart', { distance: mm(30), instanceCount: '3' });
linear('4 instances opposite', { distance: mm(25), instanceCount: '4', oppositeDirection: 'true' });
linear('5 instances centred', { distance: mm(25), instanceCount: '5', isCentered: 'true' });
linear('3 by 2 grid', {
  distance: mm(30), instanceCount: '3', hasSecondDir: 'true', directionTwo: edgeOf('dirY'), distanceTwo: mm(15),
  instanceCountTwo: '2',
}, yDir);
linear('overlapping instances added together', {
  distance: mm(12), instanceCount: '3', operationType: 'NewBodyOperationType.ADD', ...scope('base'),
});
linear('4 instances skipping the third', {
  distance: mm(30), instanceCount: '4', skipInstances: 'true', skippedInstances: '[{ "index1" : 2, "index2" : 0 }]',
});
linear('along a slanted edge', { distance: mm(22), instanceCount: '3', directionOne: edgeOf('dirS') },
  axisLine('dirS', [0, 0, 0], [1, 1, 1]));

// ----- circular pattern -----

const cyl = cylinder('base', [30, 0], 5, 0, 10);
const circular = (description, fields) =>
  batch.add('circular-pattern', description, [
    cyl, zAxis('axis1'),
    call('circularPattern', 'pattern1', { entities: base, axis: edgeOf('axis1'), ...fields }),
  ]);
circular('6 instances equally spaced over 360 degrees', { angle: deg(360), instanceCount: '6', equalSpace: 'true' });
circular('3 instances 40 degrees apart', { angle: deg(40), instanceCount: '3' });
circular('4 instances over 180 degrees, equally spaced', { angle: deg(180), instanceCount: '4', equalSpace: 'true' });
circular('5 instances centred, 30 degrees apart', { angle: deg(30), instanceCount: '5', isCentered: 'true' });
circular('3 instances opposite, 50 degrees apart', { angle: deg(50), instanceCount: '3', oppositeDirection: 'true' });
batch.add('circular-pattern', 'overlapping block instances added together', [
  box('base', [-5, 0, 0], [5, 30, 8]), zAxis('axis1'),
  call('circularPattern', 'pattern1', {
    entities: base, axis: edgeOf('axis1'), angle: deg(360), instanceCount: '8', equalSpace: 'true',
    operationType: 'NewBodyOperationType.ADD', ...scope('base'),
  }),
]);

// ----- split -----

const splitPlane = planeBody('cut', [8, 0, 0], [1, 0, 0], [0, 1, 0]);
const split = (description, fields, extra = [block, splitPlane]) =>
  batch.add('split', description, [
    extra,
    call('splitPart', 'split1', { targets: base, tool: createdBy('cut', 'FACE'), ...fields }),
  ]);
split('split a block by a plane, keeping both sides', {});
split('split keeping the front', { keepBothSides: 'false', keepFront: 'true' });
split('split keeping the back', { keepBothSides: 'false', keepFront: 'false' });
split('split a cylinder by a slanted plane', {}, [
  cylinder('base', [0, 0], 10, 0, 30), planeBody('cut', [0, 0, 15], [0.3, 0, 1], [1, 0, -0.3]),
]);
split('split by a part face, keeping the tool', { tool: faceAt(solidsOf('b'), [10, 5, 2.5]) }, [
  box('base', [0, 0, 0], [20, 10, 5]), box('b', [10, -5, -5], [30, 15, 10]),
]);

// ----- delete bodies -----

batch.add('delete', 'delete one of two bodies', [
  box('a', [0, 0, 0], [10, 10, 10]), box('b', [20, 0, 0], [30, 10, 10]),
  call('deleteBodies', 'delete1', { entities: solidsOf('a') }),
]);
batch.add('delete', 'delete every body a case made', [
  box('a', [0, 0, 0], [10, 10, 10]), cylinder('b', [20, 0], 5, 0, 10),
  call('deleteBodies', 'delete1', { entities: tools('a', 'b') }),
  'return { "deleted" : true };',
]);

// ----- construction planes -----

const cplane = (description, fields, extra = [block]) =>
  batch.add('cplane', description, [extra, call('cPlane', 'plane1', { width: mm(60), height: mm(40), ...fields })]);
cplane('offset from a face', { cplaneType: 'CPlaneType.OFFSET', entities: faceAt(base, [10, 5, 5]), offset: mm(12) });
cplane('offset from a side face, opposite', {
  cplaneType: 'CPlaneType.OFFSET', entities: faceAt(base, [20, 5, 2.5]), offset: mm(7), oppositeDirection: 'true',
});
cplane('through a vertex parallel to a face', {
  cplaneType: 'CPlaneType.PLANE_POINT', entities: `qUnion([${faceAt(base, [10, 5, 0])}, ${vertexAt(base, [20, 10, 5])}])`,
});
cplane('through an edge at 30 degrees to a face', {
  cplaneType: 'CPlaneType.LINE_ANGLE', entities: `qUnion([${edgeAt(base, [10, 0, 5])}, ${faceAt(base, [10, 5, 5])}])`,
  angle: deg(30),
});
cplane('through a line and a point', {
  cplaneType: 'CPlaneType.LINE_POINT', entities: `qUnion([${edgeAt(base, [10, 0, 0])}, ${vertexAt(base, [20, 10, 5])}])`,
});
cplane('through three vertices', {
  cplaneType: 'CPlaneType.THREE_POINT',
  entities: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [20, 0, 5])}, ${vertexAt(base, [0, 10, 5])}])`,
});
cplane('midway between two faces', {
  cplaneType: 'CPlaneType.MID_PLANE', entities: `qUnion([${faceAt(base, [0, 5, 2.5])}, ${faceAt(base, [20, 5, 2.5])}])`,
});
cplane('normal to an edge at its end', {
  cplaneType: 'CPlaneType.CURVE_POINT', entities: `qUnion([${edgeAt(base, [10, 0, 0])}, ${vertexAt(base, [20, 0, 0])}])`,
});

// ----- construction points -----

batch.add('cpoint', 'point at the middle of an edge', [
  block, call('cPoint', 'point1', { pointType: 'PointType.EDGE_POINT', entities: edgeAt(base, [10, 0, 0]), parameter: '0.5' }),
]);
batch.add('cpoint', 'point a quarter of the way along a slanted edge', [
  axisLine('line1', [0, 0, 0], [10, 20, 30]),
  call('cPoint', 'point1', { pointType: 'PointType.EDGE_POINT', entities: edgeOf('line1'), parameter: '0.25' }),
]);

// ----- variables -----

batch.add('variable', 'assign a length and read it back', [
  call('assignVariable', 'variable1', { name: '"width"', variableType: 'VariableType.LENGTH', lengthValue: mm(42) }),
  'return { "width" : getVariable(context, "width") / millimeter };',
]);
batch.add('variable', 'assign a number and an angle', [
  call('assignVariable', 'variable1', { name: '"count"', variableType: 'VariableType.NUMBER', numberValue: '7' }),
  call('assignVariable', 'variable2', { name: '"tilt"', variableType: 'VariableType.ANGLE', angleValue: deg(15) }),
  'return { "count" : getVariable(context, "count"), "tilt" : getVariable(context, "tilt") / degree };',
]);
batch.add('variable', 'measure the distance between two vertices', [
  block,
  call('assignVariable', 'variable1', {
    name: '"gap"', mode: 'VariableMode.MEASURED', measurementMode: 'VariableMeasurementMode.DISTANCE',
    entityCouple: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [20, 10, 5])}])`,
  }),
  'return { "gap" : getVariable(context, "gap") / millimeter };',
]);
batch.add('variable', 'measure the length of an edge', [
  block,
  call('assignVariable', 'variable1', {
    name: '"len"', mode: 'VariableMode.MEASURED', measurementMode: 'VariableMeasurementMode.LENGTH',
    lengthEntities: edgeAt(base, [10, 0, 0]),
  }),
  'return { "len" : getVariable(context, "len") / millimeter };',
]);
batch.add('variable', 'a variable drives a box', [
  call('assignVariable', 'variable1', { name: '"depth"', variableType: 'VariableType.LENGTH', lengthValue: mm(13) }),
  sketch('sketch1', PLANES.xy), rect('sketch1', 'rect1', [0, 0], [10, 10]), solve('sketch1'),
  call('extrude', 'extrude1', { entities: regions('sketch1'), endBound: 'BoundingType.BLIND', depth: 'getVariable(context, "depth")' }),
]);

batch.write('tools/fixtures/gen-batch-002.mjs');
