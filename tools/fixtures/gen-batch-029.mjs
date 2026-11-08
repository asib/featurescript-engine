// Generates fixture batch 029: sheet metal tear drop hems, whose profile std makes with a constrained sketch.
//
// Usage: node tools/fixtures/gen-batch-029.mjs
//
// Writes fixtures/batch-029/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm, planeAt, rect, sketch, solve } from './lib.mjs';

const helpers = `
// The point at fractions (fx, fy, fz) of a part's bounding box.
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

// An edge, face or vertex of a part through a point at fractions of its bounding box.
function atFraction(context is Context, part is Query, entityType is EntityType, fx is number, fy is number, fz is number) returns Query
{
    return qContainsPoint(qOwnedByBody(part, entityType), pointAtFraction(context, part, fx, fy, fz));
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;


const batch = new Batch({ number: 29, title: 'sheet metal tear drop hems', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const counts = returns({ counts: 'bodyCounts(context)' });
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
const hem = (fields) =>
  call('sheetMetalHem', 'hem1', {
    edges: at('EDGE', 1, 0.5, 1), oppositeDirection: 'false', hemAlignment: 'SMHemAlignment.IN_PLACE', hemType: 'SMHemType.TEAR_DROP',
    hemCornerType: 'SMHemCornerType.SIMPLE', ...fields,
  });

batch.add('sheet-metal-hem', 'a tear drop hem 10 mm wide round a 1 mm radius, closed to the minimal gap', [
  plate, hem({ innerRadius: mm(1), useMinimalGapForTipGap: 'true', length: mm(10) }), counts,
]);
batch.add('sheet-metal-hem', 'a tear drop hem 12 mm wide round a 2 mm radius, its tip 1 mm off the plate', [
  plate, hem({ innerRadius: mm(2), useMinimalGapForTipGap: 'false', tipGap: mm(1), length: mm(12) }), counts,
]);
batch.add('sheet-metal-hem', 'a tear drop hem 10 mm wide, aligned outside', [
  plate, hem({ innerRadius: mm(1), useMinimalGapForTipGap: 'true', length: mm(10), hemAlignment: 'SMHemAlignment.OUTER' }), counts,
]);

batch.write('tools/fixtures/gen-batch-029.mjs');
