// Generates fixture batch 014: sheet metal hems, rips, and more flange options.
//
// Usage: node tools/fixtures/gen-batch-014.mjs
//
// Writes fixtures/batch-014/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, deg, faceAt, mm, planeAt, planeBody, rect, sketch, solidsOf, solve, vec3 } from './lib.mjs';

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

const batch = new Batch({ number: 14, title: 'sheet metal hems, rips, flange options', helpers });

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
    edges: at('EDGE', 1, 0.5, 1), oppositeDirection: 'false', hemAlignment: 'SMHemAlignment.IN_PLACE',
    hemCornerType: 'SMHemCornerType.SIMPLE', ...fields,
  });
const flange = (edges, fields = {}) =>
  call('sheetMetalFlange', 'flange1', {
    edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', ...fields,
  });

// ----- hems -----

batch.add('sheet-metal-hem', 'a straight hem, 10 mm long with a 1 mm inner radius', [
  plate, hem({ hemType: 'SMHemType.STRAIGHT', useMinimalGapForStraight: 'false', innerRadius: mm(1), length: mm(10) }), counts,
]);
batch.add('sheet-metal-hem', 'a straight hem closed down to the minimal gap, aligned outside', [
  plate,
  hem({ hemType: 'SMHemType.STRAIGHT', useMinimalGapForStraight: 'true', length: mm(8), hemAlignment: 'SMHemAlignment.OUTER' }),
  counts,
]);
batch.add('sheet-metal-hem', 'a rolled hem, 270 degrees round a 3 mm radius', [
  plate, hem({ hemType: 'SMHemType.ROLLED', innerRadius: mm(3), angle: deg(270) }), counts,
]);

// ----- rips -----

batch.add('sheet-metal-rip', 'a plate ripped corner to corner', [
  plate,
  call('sheetMetalRip', 'rip1', {
    vertices: `qUnion([qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), ${vec3(0, 0, 2)}), qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), ${vec3(40, 30, 2)})])`,
    useDefaultGap: 'false', minimalClearance: mm(0.5),
  }),
  counts,
]);

// ----- flange options -----

batch.add('sheet-metal-flange', 'a flange up to a plane 12 mm above the plate', [
  plate, planeBody('limit', [0, 0, 12], [0, 0, 1], [1, 0, 0]),
  flange(at('EDGE', 1, 0.5, 1), { limitType: 'SMFlangeBoundingType.UP_TO_ENTITY', limitEntity: `qCreatedBy(id + "limit", EntityType.FACE)` }),
  counts,
]);
batch.add('sheet-metal-flange', 'flanges on two edges at a corner, mitred at 30 degrees', [
  plate, flange(`qUnion([${at('EDGE', 1, 0.5, 1)}, ${at('EDGE', 0.5, 1, 1)}])`, { autoMiter: 'false', miterAngle: deg(30) }), counts,
]);
batch.add('sheet-metal-flange', 'a flange on an edge of a converted box', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(1), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.CONVERT',
    partToConvert: solidsOf('base'), facesToExclude: faceAt(solidsOf('base'), [20, 15, 20]), bends: 'qNothing()', clearance: mm(0),
    keepInputParts: 'false',
  }),
  flange(at('EDGE', 0.5, 0, 1), { distance: mm(10) }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-014.mjs');
