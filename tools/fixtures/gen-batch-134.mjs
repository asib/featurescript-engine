// Generates fixture batch 134: everyday parts, several features each. A flange (a disc with a boss, a bore and six
// bolt holes patterned round it, the boss's edges filleted); batch 006's L with a rib, two holes in its plate, the
// plate's edge chamfered and the wall's filleted; a shaft with a keyway, its ends chamfered; a plate with an obround
// slot through it, the slot's top edge chamfered all round; a plate with a pocket, its corners filleted and then its
// floor; a cup (a cylinder, its foot filleted, shelled open at its top).
//
// Usage: node tools/fixtures/gen-batch-134.mjs
//
// Writes fixtures/batch-134/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, circle, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

const union = (...qs) => `qUnion([${qs.join(', ')}])`;

const batch = new Batch({ number: 134, title: 'everyday parts: a flange, a ribbed bracket, a keyed shaft, a slotted plate, a pocket, a cup', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const added = (name, depth) => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(depth), operationType: 'NewBodyOperationType.ADD', ...scoped });
const through = (name) => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped });
const cut = (name, depth) => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(depth), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped });
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const featurePattern = (fn, feature, body, fields) =>
  `${fn}(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "${feature}") : function(instanceId) { ${body} } } as FeatureList, ${Object.entries(fields).map(([k, v]) => `"${k}" : ${v}`).join(', ')} });`;
const fillet = (name, edges, r, more = {}) => call('fillet', name, { entities: edges, radius: mm(r), ...more });
const chamfer = (name, edges, w, more = {}) => call('chamfer', name, { entities: edges, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(w), ...more });

batch.add('circular-pattern', 'a flange: a disc (radius 30, 8 thick) with a boss (radius 12) to 23, a bore (radius 6) through, a bolt hole (radius 3) at (22, 0) patterned 6 times round; the boss\'s top edge filleted 2 mm and its foot 1 mm', [
  cylinder('base', [0, 0], 30, 0, 8),
  sketch('bossSketch', top(8)), circle('bossSketch', 'circle1', [0, 0], 12), solve('bossSketch'), added('boss', 15),
  sketch('boreSketch', top(23)), circle('boreSketch', 'circle1', [0, 0], 6), solve('boreSketch'), through('bore'),
  sketch('boltSketch', top(8)), circle('boltSketch', 'circle1', [22, 0], 3), solve('boltSketch'), through('bolt'),
  sketch('axis1', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis1', 'line1', [0, 0], [10, 0]), solve('axis1'),
  featurePattern('circularPattern', 'bolt',
    `extrude(context, instanceId, { "entities" : ${regions('boltSketch')}, "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${base} });`,
    { axis: `qCreatedBy(id + "axis1", EntityType.EDGE)`, angle: deg(360), instanceCount: 6, equalSpace: 'true', fullFeaturePattern: 'false' }),
  fillet('fillet1', edgeAt(base, [12, 0, 23]), 2),
  fillet('fillet2', edgeAt(base, [12, 0, 8]), 1),
]);

const walls = [
  box('base', [0, 0, 0], [40, 30, 4]),
  box('wall', [0, 0, 4], [4, 30, 24]),
  call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: union(base, solidsOf('wall')) }),
];
batch.add('rib', 'batch 006\'s L (a 40 × 30 × 4 plate, a wall 4 thick and 24 tall at x = 0) with a rib 3 thick across its middle, two holes (radius 2.5) through the plate, the plate\'s far top edge chamfered 1 mm and the wall\'s outer top edge filleted 2 mm', [
  walls,
  sketch('profile', planeAt([0, 15, 0], [0, -1, 0], [1, 0, 0])), line('profile', 'line1', [4, 20], [30, 4]), solve('profile'),
  call('rib', 'rib1', { profiles: wireEdges('profile'), parts: base, thickness: mm(3), ribExtrusionDirection: 'RibExtrusionDirection.PARALLEL_TO_SKETCH_PLANE', oppositeDirection: 'true' }),
  sketch('holesSketch', top(4)), circle('holesSketch', 'circle1', [25, 7], 2.5), circle('holesSketch', 'circle2', [25, 23], 2.5), solve('holesSketch'), through('holes'),
  chamfer('chamfer1', edgeAt(base, [40, 15, 4]), 1),
  fillet('fillet1', edgeAt(base, [0, 15, 24]), 2),
]);

batch.add('extrude', 'a shaft (radius 10, 60 long, along x) with a keyway 20 long, 6 wide and 4 deep cut in its top, both ends chamfered 1 mm', [
  sketch('baseSketch', planeAt([-30, 0, 0], [1, 0, 0], [0, 1, 0])), circle('baseSketch', 'circle1', [0, 0], 10), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(60) }),
  sketch('keySketch', top(10)), rect('keySketch', 'rect1', [-10, -3], [10, 3]), solve('keySketch'), cut('key', 4),
  chamfer('chamfer1', `qUnion([${edgeAt(base, [-30, -10, 0])}, ${edgeAt(base, [30, -10, 0])}])`, 1),
]);

batch.add('chamfer', 'a 60 × 30 × 6 plate with an obround slot (40 long, 10 wide) through its middle, the slot\'s top edge chamfered 0.5 mm all round', [
  box('base', [0, 0, 0], [60, 30, 6]),
  sketch('slotSketch', top(6)),
  line('slotSketch', 'line1', [15, 10], [45, 10]), arc('slotSketch', 'arc1', [45, 10], [50, 15], [45, 20]),
  line('slotSketch', 'line2', [45, 20], [15, 20]), arc('slotSketch', 'arc2', [15, 20], [10, 15], [15, 10]),
  solve('slotSketch'), through('slot'),
  chamfer('chamfer1', edgeAt(base, [30, 10, 6]), 0.5, { tangentPropagation: 'true' }),
]);

batch.add('fillet', 'a 60 × 40 × 10 plate with a 40 × 20 pocket 6 deep: the pocket\'s corners filleted 4 mm, then its floor\'s edge 1 mm all round', [
  box('base', [0, 0, 0], [60, 40, 10]),
  sketch('pocketSketch', top(10)), rect('pocketSketch', 'rect1', [10, 10], [50, 30]), solve('pocketSketch'), cut('pocket', 6),
  fillet('fillet1', `qUnion([${[[10, 10], [50, 10], [50, 30], [10, 30]].map(([x, y]) => edgeAt(base, [x, y, 7])).join(', ')}])`, 4),
  fillet('fillet2', edgeAt(base, [30, 10, 4]), 1, { tangentPropagation: 'true' }),
]);

batch.add('shell', 'a cup: a cylinder (radius 20, 30 tall), its foot filleted 3 mm, shelled 2 mm open at its top', [
  cylinder('base', [0, 0], 20, 0, 30),
  fillet('fillet1', edgeAt(base, [20, 0, 0]), 3),
  call('shell', 'shell1', { entities: faceAt(base, [0, 0, 30]), thickness: mm(2) }),
]);

batch.write('tools/fixtures/gen-batch-134.mjs');
