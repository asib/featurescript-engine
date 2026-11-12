// Generates fixture batch 137: sweeps locked to curved faces along paths other than circles round their axes and
// their rulings (which batches 058, 070, 076 and 119 cover): a square and a thread's triangle along helices round a
// cylinder, a square along a helix round a cone frustum, a square round the closed curve a circle beside a cylinder
// splits its side along, a square round a small circle on a sphere tilted to its axis. Each profile is sketched on
// the plane square to the path at its start, its x along the locked face's outward normal there.
//
// Usage: node tools/fixtures/gen-batch-137.mjs
//
// Writes fixtures/batch-137/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, call, circle, cylinder, deg, faceAt, line, mm, planeAt, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
// The plane square to an edge at its start, its x along the face's outward normal there.
function startPlane(context is Context, edge is Query, face is Query) returns Plane
{
    const t = evEdgeTangentLine(context, { "edge" : edge, "parameter" : 0 });
    const d = evDistance(context, { "side0" : t.origin, "side1" : face });
    const n = evFaceTangentPlane(context, { "face" : face, "parameter" : d.sides[1].parameter }).normal;
    return plane(t.origin, t.direction, n);
}
`;

const batch = new Batch({ number: 137, title: 'sweeps locked to curved faces along other paths', helpers });
const base = solidsOf('base');
const wires = (name) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const profileOn = (edge, face) => `const profile = newSketchOnPlane(context, id + "profile", { "sketchPlane" : startPlane(context, ${edge}, ${face}) });`;
const square = `skRectangle(profile, "rect1", { "firstCorner" : vector(-0.5, -0.5) * millimeter, "secondCorner" : vector(0.5, 0.5) * millimeter });`;
const thread = [
  `skLineSegment(profile, "l1", { "start" : vector(0, -0.8) * millimeter, "end" : vector(1, 0) * millimeter });`,
  `skLineSegment(profile, "l2", { "start" : vector(1, 0) * millimeter, "end" : vector(0, 0.8) * millimeter });`,
  `skLineSegment(profile, "l3", { "start" : vector(0, 0.8) * millimeter, "end" : vector(0, -0.8) * millimeter });`,
];
const locked = (path, face, draw) => [
  profileOn(path, face), draw, solve('profile'),
  call('sweep', 'swept', { profiles: regions('profile'), path, profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: face }),
];
// A helix of radius 10 about z from (10, 0, 0), 2 turns 20 high.
const helix = (hand) => [
  sketch('axis1', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis1', 'line1', [0, 0], [30, 0]), solve('axis1'),
  call('opPoint', 'start', { point: vec3(10, 0, 0) }),
  call('helix', 'helix1', {
    startType: 'StartType.START_POINT', startAngle: deg(0), handedness: `Direction.${hand}`, oppositeDirection: 'false',
    axisType: 'AxisType.AXIS', axis: `qCreatedBy(id + "axis1", EntityType.EDGE)`, pathType: 'PathType.TURNS',
    revolutions: 2, startPoint: `qCreatedBy(id + "start", EntityType.VERTEX)`, endType: 'EndType.HEIGHT', height: mm(20),
  }),
];
const side = faceAt(base, [10, 0, 15]);

batch.add('sweep', 'a cylinder (radius 10, 30 tall): a 1 × 1 square across its side swept along a left-handed helix on it (2 turns, 20 high), locked to the side', [
  cylinder('base', [0, 0], 10, 0, 30), helix('CCW'), locked(wires('helix1'), side, square),
]);
batch.add('sweep', 'a cylinder (radius 10, 30 tall): a thread\'s triangle (1 out, 1.6 along) on its side swept along a right-handed helix on it, locked to the side', [
  cylinder('base', [0, 0], 10, 0, 30), helix('CW'), locked(wires('helix1'), side, thread),
]);
batch.add('sweep', 'a cone frustum (radius 10 to 6, 20 tall): a 1 × 1 square across its side swept along a helix round it (3 turns), locked to the side', [
  sketch('f1', PLANES.xz), line('f1', 'l1', [0, 0], [10, 0]), line('f1', 'l2', [10, 0], [6, 20]), line('f1', 'l3', [6, 20], [0, 20]), line('f1', 'l4', [0, 20], [0, 0]), solve('f1'),
  sketch('faxis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('faxis', 'line1', [-50, 0], [50, 0]), solve('faxis'),
  call('revolve', 'base', { entities: regions('f1'), axis: `qCreatedBy(id + "faxis", EntityType.EDGE)`, fullRevolve: 'true' }),
  call('helix', 'helix1', {
    startType: 'StartType.START_ANGLE', startAngle: deg(0), handedness: 'Direction.CCW', oppositeDirection: 'false',
    axisType: 'AxisType.SURFACE', entities: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`, pathType: 'PathType.TURNS', revolutions: '3',
  }),
  locked(wires('helix1'), `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`, square),
]);
batch.add('sweep', 'a cylinder (radius 10, 30 tall): its side split by a circle of radius 4 on the plane x = 15 projected normal to it; a 1 × 1 square swept round the split edge, locked to the side', [
  cylinder('base', [0, 0], 10, 0, 30),
  sketch('cut', planeAt([15, 0, 0], [1, 0, 0], [0, 1, 0])), circle('cut', 'circle1', [0, 15], 4), solve('cut'),
  call('splitPart', 'split1', { splitType: 'SplitType.FACE', faceTargets: side, faceTools: wires('cut'), projectionType: 'ProjectionType.NORMAL_TO_TARGET' }),
  locked(`qCreatedBy(id + "split1", EntityType.EDGE)`, faceAt(base, [-10, 0, 15]), square),
]);
batch.add('sweep', 'a sphere of radius 10: its face split by a plane 5 from its centre, tilted 30° from its axis; a 1 × 1 square swept round the split edge, locked to the sphere', [
  call('fSphere', 'base', { radius: mm(10) }),
  call('opPlane', 'tilted', { plane: `plane(${vec3(0, -2.5, 5 * Math.cos(Math.PI / 6))}, vector(0, ${-Math.sin(Math.PI / 6)}, ${Math.cos(Math.PI / 6)}))`, width: mm(40), height: mm(40) }),
  call('opSplitFace', 'split1', { faceTargets: `qOwnedByBody(${base}, EntityType.FACE)`, faceTools: `qCreatedBy(id + "tilted", EntityType.FACE)` }),
  locked(`qCreatedBy(id + "split1", EntityType.EDGE)`, faceAt(base, [0, 0, -10]), square),
]);

batch.write('tools/fixtures/gen-batch-137.mjs');
