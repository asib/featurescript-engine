// Generates fixture batch 093: `opSplitBySelfShadow`, which no batch has run: blocks, a cylinder, a sphere and a
// cone frustum split along their silhouettes as seen straight on and at a slant; the shadows a tower casts on its
// step, a boss and a dome on their plates; and whether one body shadows another given with it, or not given.
//
// Usage: node tools/fixtures/gen-batch-093.mjs
//
// Writes fixtures/batch-093/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, circle, cylinder, dir3, line, mm, planeAt, regions, sketch, solidsOf, solve } from './lib.mjs';

const helpers = `
// Faces' areas, smallest first.
function areaList(context is Context, faces is array) returns array
{
    return sort(mapArray(faces, function(f) { return roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 6); }),
        function(a, b) { return a - b; });
}

// What a split by self shadow made: the visible and invisible faces' areas, the new edges' lengths, and the split
// faces in front of the split (visible) and behind it.
function shadowProbe(context is Context, splitId is Id, result is map) returns map
{
    return { "visible" : areaList(context, result.visibleFaces),
             "invisible" : areaList(context, result.invisibleFaces),
             "edges" : sort(mapArray(evaluateQuery(context, qCreatedBy(splitId, EntityType.EDGE)), function(e) { return roundToPrecision(evLength(context, { "entities" : e }) / millimeter, 6); }),
                 function(a, b) { return a - b; }),
             "front" : areaList(context, evaluateQuery(context, qSplitBy(splitId, EntityType.FACE, false))),
             "back" : areaList(context, evaluateQuery(context, qSplitBy(splitId, EntityType.FACE, true))) };
}
`;

const batch = new Batch({ number: 93, title: 'splits by self shadow', helpers });

const shadow = (bodies, [x, y, z]) => [
  `const result = opSplitBySelfShadow(context, id + "shadow1", { "bodies" : ${bodies}, "viewDirection" : normalize(${dir3(x, y, z)}) });`,
  `return shadowProbe(context, id + "shadow1", result);`,
];
const base = solidsOf('base');
const add = (scope) => ({ operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: scope });

// ----- silhouettes -----

batch.add('shadow', 'a 40 × 30 × 20 block seen from straight above', [box('base', [0, 0, 0], [40, 30, 20]), shadow(base, [0, 0, -1])]);
batch.add('shadow', 'the block seen from above at a slant, along (1, 1, -1)', [box('base', [0, 0, 0], [40, 30, 20]), shadow(base, [1, 1, -1])]);
batch.add('shadow', 'a cylinder of radius 10, z 0 to 30, seen from the side along x', [cylinder('base', [0, 0], 10, 0, 30), shadow(base, [1, 0, 0])]);
batch.add('shadow', 'the cylinder seen from above at a slant, along (1, 0, -1)', [cylinder('base', [0, 0], 10, 0, 30), shadow(base, [1, 0, -1])]);
batch.add('shadow', 'a sphere of radius 10 seen from straight above', [call('fSphere', 'base', { radius: mm(10) }), shadow(base, [0, 0, -1])]);
// A frustum, radius 10 at z = 0 to 5 at z = 20, revolved about its own edge on z (its sketch entity).
const frustum = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [5, 20]), line('s1', 'l3', [5, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1'),
  call('revolve', 'base', { entities: regions('s1'), axis: `sketchEntityQuery(id + "s1", EntityType.EDGE, "l4")`, fullRevolve: 'true' }),
];
batch.add('shadow', 'a cone frustum (radius 10 at z = 0 to 5 at z = 20) seen from the side along x', [frustum, shadow(base, [1, 0, 0])]);
batch.add('shadow', 'the frustum seen from above at a slant, along (1, 0, -1)', [frustum, shadow(base, [1, 0, -1])]);

// ----- shadows -----

batch.add('shadow', 'a 40 × 30 × 10 step with a tower x 0 to 10 up to z = 30, seen along (1, 0, -1): the tower shadows the step to x = 30', [
  box('base', [0, 0, 0], [40, 30, 10]),
  sketch('t1', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), `skRectangle(t1, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(10, 30) * millimeter });`, solve('t1'),
  call('extrude', 'tower', { entities: regions('t1'), endBound: 'BoundingType.BLIND', depth: mm(20), ...add(base) }),
  shadow(base, [1, 0, -1]),
]);
batch.add('shadow', 'a boss of radius 5 at (20, 20), z 5 to 20, on a 50 × 40 × 5 plate, seen along (1, 0, -1)', [
  box('base', [0, 0, 0], [50, 40, 5]),
  sketch('b1', planeAt([0, 0, 5], [0, 0, 1], [1, 0, 0])), circle('b1', 'c1', [20, 20], 5), solve('b1'),
  call('extrude', 'boss', { entities: regions('b1'), endBound: 'BoundingType.BLIND', depth: mm(15), ...add(base) }),
  shadow(base, [1, 0, -1]),
]);
batch.add('shadow', 'a dome of radius 10 at (20, 20) on a 50 × 40 × 5 plate, seen along (1, 0, -1)', [
  box('base', [0, 0, 0], [50, 40, 5]),
  sketch('d1', planeAt([20, 20, 5], [0, -1, 0], [1, 0, 0])), line('d1', 'l1', [0, 10], [0, 0]), line('d1', 'l2', [0, 0], [10, 0]), arc('d1', 'a1', [10, 0], [7.0710678, 7.0710678], [0, 10]), solve('d1'),
  call('revolve', 'dome', { entities: regions('d1'), axis: `sketchEntityQuery(id + "d1", EntityType.EDGE, "l1")`, fullRevolve: 'true', ...add(base) }),
  shadow(base, [1, 0, -1]),
]);

// ----- other bodies -----

const pair = [box('base', [0, 0, 0], [40, 30, 10]), box('top', [15, 10, 20], [25, 20, 25])];
batch.add('shadow', 'a 40 × 30 × 10 block and a 10 × 10 × 5 block above it (z 20 to 25), both given, seen from straight above', [
  pair, shadow(`qUnion([${base}, ${solidsOf('top')}])`, [0, 0, -1]),
]);
batch.add('shadow', 'the same two blocks, only the lower one given', [pair, shadow(base, [0, 0, -1])]);

batch.write('tools/fixtures/gen-batch-093.mjs');
