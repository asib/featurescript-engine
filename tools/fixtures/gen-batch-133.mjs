// Generates fixture batch 133: faces split by sketches projected normal to the target, onto curved faces (the
// engine projected along the plane's normal only). A cylinder's side split by a circle and by a rectangle taller
// than it on a plane beside it; a sphere split by a circle above it; a cone's side split by a circle beside it; a
// block's top, as before, for reference.
//
// Usage: node tools/fixtures/gen-batch-133.mjs
//
// Writes fixtures/batch-133/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, cylinder, faceAt, mm, planeAt, rect, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 133, title: 'faces split by sketches projected normal to curved targets', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const split = (target) =>
  call('splitPart', 'split1', { splitType: 'SplitType.FACE', faceTargets: target, faceTools: wireEdges('cut'), projectionType: 'ProjectionType.NORMAL_TO_TARGET' });
// A sketch on the plane x = 15, its x along y and its y along z.
const beside = planeAt([15, 0, 0], [1, 0, 0], [0, 1, 0]);

batch.add('split-face', 'a cylinder of radius 10, 30 tall: its side split by a circle of radius 4 on the plane x = 15, projected normal to it', [
  cylinder('base', [0, 0], 10, 0, 30), sketch('cut', beside), circle('cut', 'circle1', [0, 15], 4), solve('cut'),
  split(faceAt(base, [10, 0, 15])),
]);
batch.add('split-face', 'a cylinder of radius 10, 30 tall: its side split by a rectangle 8 wide and taller than it on the plane x = 15, projected normal to it', [
  cylinder('base', [0, 0], 10, 0, 30), sketch('cut', beside), rect('cut', 'rect1', [-4, -5], [4, 35]), solve('cut'),
  split(faceAt(base, [10, 0, 15])),
]);
batch.add('split-face', 'a sphere of radius 10: split by a circle of radius 5 on the plane z = 15, projected normal to it', [
  call('opSphere', 'base', { center: vec3(0, 0, 0), radius: mm(10) }),
  sketch('cut', planeAt([0, 0, 15], [0, 0, 1], [1, 0, 0])), circle('cut', 'circle1', [0, 0], 5), solve('cut'),
  split(faceAt(base, [0, 0, 10])),
]);
batch.add('split-face', 'a cone frustum (radius 10 to 5, 20 tall): its side split by a circle of radius 3 on the plane x = 15, projected normal to it', [
  call('fCone', 'base', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 20), bottomRadius: mm(10), topRadius: mm(5) }),
  sketch('cut', beside), circle('cut', 'circle1', [0, 10], 3), solve('cut'),
  split(faceAt(base, [7.5, 0, 10])),
]);
batch.add('split-face', 'a 40 × 30 × 12 block: its top split by a circle of radius 5 above it, projected normal to it', [
  box('base', [0, 0, 0], [40, 30, 12]),
  sketch('cut', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('cut', 'circle1', [20, 15], 5), solve('cut'),
  split(faceAt(base, [10, 10, 12])),
]);

batch.write('tools/fixtures/gen-batch-133.mjs');
