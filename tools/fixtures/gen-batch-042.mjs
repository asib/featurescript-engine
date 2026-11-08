// Generates fixture batch 042: thin solids recognized as sheet metal that batch 031 left out: an L whose bend is
// already rounded, a rolled wall (half a tube), and an L whose legs differ in thickness.
//
// Usage: node tools/fixtures/gen-batch-042.mjs
//
// Writes fixtures/batch-042/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm, planeAt, sketch, solidsOf, solve } from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({ number: 42, title: 'sheet metal recognized: rounded bends, rolled walls, several thicknesses', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const v2 = (x, y) => `vector(${x}, ${y}) * millimeter`;
const seg = (name, a, b) => `skLineSegment(s1, "${name}", { "start" : ${v2(...a)}, "end" : ${v2(...b)} });`;
const arc3 = (name, a, m, b) => `skArc(s1, "${name}", { "start" : ${v2(...a)}, "mid" : ${v2(...m)}, "end" : ${v2(...b)} });`;
const recognize = (radius) =>
  call('sheetMetalRecognize', 'recognize1', { bodies: base, radius: mm(radius), changeThickness: 'false', keepInputParts: 'false', kFactor: '0.45', minimalClearance: mm(0.02) });
const extruded = (depth) => call('extrude', 'base', { entities: `qSketchRegion(id + "s1")`, endBound: 'BoundingType.BLIND', depth: mm(depth) });
const s = Math.SQRT1_2;

// An L 2 mm thick, its bend rounded 2 mm inside and 4 mm outside about (36, 4).
batch.add('sheet-metal-recognize', 'an L bracket 2 mm thick with its bend rounded 2 mm inside, extruded 30 mm, recognized', [
  sketch('s1', xy),
  seg('l1', [0, 0], [36, 0]), arc3('a1', [36, 0], [36 + 4 * s, 4 - 4 * s], [40, 4]), seg('l2', [40, 4], [40, 25]), seg('l3', [40, 25], [38, 25]),
  seg('l4', [38, 25], [38, 4]), arc3('a2', [38, 4], [36 + 2 * s, 4 - 2 * s], [36, 2]), seg('l5', [36, 2], [0, 2]), seg('l6', [0, 2], [0, 0]),
  solve('s1'), extruded(30), recognize(1), returns({ counts: 'bodyCounts(context)' }),
]);
// Half a tube: radii 10 and 12 about the origin, from (−12, 0) round through y > 0 to (12, 0).
batch.add('sheet-metal-recognize', 'half a tube 2 mm thick, extruded 30 mm, recognized as a rolled wall', [
  sketch('s1', xy),
  arc3('outer', [12, 0], [0, 12], [-12, 0]), seg('l1', [-12, 0], [-10, 0]), arc3('inner', [-10, 0], [0, 10], [10, 0]), seg('l2', [10, 0], [12, 0]),
  solve('s1'), extruded(30), recognize(1), returns({ counts: 'bodyCounts(context)' }),
]);
// An L whose base is 2 mm thick and upright 3 mm.
batch.add('sheet-metal-recognize', 'an L bracket 2 mm thick at its base and 3 mm at its upright, recognized', [
  sketch('s1', xy),
  `skPolyline(s1, "l", { "points" : [${v2(0, 0)}, ${v2(40, 0)}, ${v2(40, 25)}, ${v2(37, 25)}, ${v2(37, 2)}, ${v2(0, 2)}, ${v2(0, 0)}] });`,
  solve('s1'), extruded(30), recognize(1), returns({ counts: 'bodyCounts(context)' }),
]);

batch.write('tools/fixtures/gen-batch-042.mjs');
