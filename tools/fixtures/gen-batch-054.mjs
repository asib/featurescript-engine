// Generates fixture batch 054: face blends beyond batch 010's circular one between two sheets at right angles:
// conic and curvature sections, uneven radii (either way round), width control (even and uneven), untrimmed and
// detached blends, sheets apart, and sheets at 60° and 120°.
//
// Usage: node tools/fixtures/gen-batch-054.mjs
//
// Writes fixtures/batch-054/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, line, mm, planeAt, sketch, solve, PLANES } from './lib.mjs';

const batch = new Batch({ number: 54, title: 'face blends: sections, uneven radii, width, trims, sheets apart and at angles' });

const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
// The sheets of batch 010: "floor" is the upright one, y = 0, x 0..30, z 0..20 (a line along x extruded up), and
// "wall" the level one, z = 0, x 0..30, y 0..20 (a line along x extruded along y); they meet along the x axis, and
// the blend goes in the corner between them (y and z positive).
const upright = (z0, d = [0, 0, 1]) => [
  sketch('s1', planeAt([0, 0, z0], d, [1, 0, 0])), line('s1', 'l1', [0, 0], [30, 0]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
];
const level = [
  sketch('s2', PLANES.xz), line('s2', 'l1', [0, 0], [30, 0]), solve('s2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const blend = (fields) =>
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('floor')}, EntityType.FACE)`, flipSide1: 'true', side2: `qOwnedByBody(${sheetsOf('wall')}, EntityType.FACE)`,
    flipSide2: 'false', propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS',
    radius: mm(5), asymmetric: 'false', crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false',
    conicHoldLines: 'false', hasCliffEdges: 'false', hasCaps: 'false', hasLimits: 'false', hasHelpPoint: 'false',
    trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false', showIsocurves: 'false', ...fields,
  });
const corner = [upright(0), level];
const conic = { crossSectionShape: 'FaceBlendCrossSectionShape.CONIC', rho: '0.7' };

batch.add('face-blend', 'two sheets at right angles, conic, 5 mm, rho 0.7', [corner, blend(conic)]);
batch.add('face-blend', 'two sheets at right angles, curvature, 5 mm, magnitude 0.5', [corner, blend({ crossSectionShape: 'FaceBlendCrossSectionShape.CURVATURE', magnitude: '0.5' })]);
batch.add('face-blend', 'two sheets at right angles, circular, radii 5 and 3 mm', [corner, blend({ asymmetric: 'true', secondRadius: mm(3), switchAsymmetric: 'false' })]);
batch.add('face-blend', 'the same, switched', [corner, blend({ asymmetric: 'true', secondRadius: mm(3), switchAsymmetric: 'true' })]);
batch.add('face-blend', 'two sheets at right angles, circular, 6 mm wide', [corner, blend({ blendControlType: 'BlendControlType.WIDTH', width: mm(6) })]);
batch.add('face-blend', 'two sheets at right angles, circular, 6 mm wide, ratio 2', [
  corner, blend({ blendControlType: 'BlendControlType.WIDTH', width: mm(6), asymmetric: 'true', widthRatio: '2', switchAsymmetric: 'false' }),
]);
batch.add('face-blend', 'two sheets at right angles, circular, 5 mm, untrimmed', [corner, blend({ trim: 'FaceBlendTrimType.NO_TRIM' })]);
batch.add('face-blend', 'two sheets at right angles, circular, 5 mm, detached', [corner, blend({ detach: 'true' })]);
// The upright sheet raised to z 2..22: the sheets don't meet.
batch.add('face-blend', 'an upright sheet 2 mm above a level one, circular, 5 mm', [upright(2), level, blend({})]);
batch.add('face-blend', 'the same, conic, rho 0.7', [upright(2), level, blend(conic)]);
// The upright sheet leaning over the level one at 60° to it, and back at 120°.
for (const deg of [60, 120]) {
  const r = (deg * Math.PI) / 180;
  batch.add('face-blend', `an upright sheet at ${deg}° to a level one, circular, 5 mm`, [upright(0, [0, Math.cos(r), Math.sin(r)]), level, blend({})]);
}

batch.write('tools/fixtures/gen-batch-054.mjs');
