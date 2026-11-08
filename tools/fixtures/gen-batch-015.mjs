// Generates fixture batch 015: sheet metal corner breaks, unfolding and refolding, and partial flanges (with the
// bend reliefs they need).
//
// Usage: node tools/fixtures/gen-batch-015.mjs
//
// Writes fixtures/batch-015/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, deg, line, mm, planeAt, rect, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

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

const batch = new Batch({ number: 15, title: 'sheet metal corner breaks, unfold and refold, partial flanges', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const nearest = (type, p) => `qClosestTo(qOwnedByBody(${part}, EntityType.${type}), ${vec3(...p)})`;
const counts = returns({ counts: 'bodyCounts(context)' });

const start = (fields) =>
  call('sheetMetalStart', 'sm1', { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', ...fields });
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  start({ process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()' }),
];
const lSection = [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [30, 20]), solve('s1'),
  start({
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
];
const flange = (edges, fields = {}) =>
  call('sheetMetalFlange', 'flange1', {
    edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', ...fields,
  });

// ----- corner breaks -----

batch.add('sheet-metal-corner-break', 'a plate corner rounded to 5 mm', [
  plate,
  call('sheetMetalCornerBreak', 'break1', {
    blendType: 'EdgeBlendType.FILLET', entities: nearest('VERTEX', [40, 30, 2]), blendControlType: 'BlendControlType.RADIUS',
    crossSection: 'FilletCrossSection.CIRCULAR', radius: mm(5),
  }),
  counts,
]);
batch.add('sheet-metal-corner-break', 'two plate corners chamfered 4 mm', [
  plate,
  call('sheetMetalCornerBreak', 'break1', {
    blendType: 'EdgeBlendType.CHAMFER', entities: `qUnion([${nearest('VERTEX', [0, 0, 2])}, ${nearest('VERTEX', [40, 0, 2])}])`,
    chamferMethod: 'ChamferMethod.FACE_OFFSET', chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(4),
  }),
  counts,
]);
batch.add('sheet-metal-corner-break', 'the top corner of a flange rounded to 3 mm', [
  plate, flange(at('EDGE', 1, 0.5, 1)),
  call('sheetMetalCornerBreak', 'break1', {
    blendType: 'EdgeBlendType.FILLET', entities: nearest('VERTEX', [42, 30, 15]), blendControlType: 'BlendControlType.RADIUS',
    crossSection: 'FilletCrossSection.CIRCULAR', radius: mm(3),
  }),
  counts,
]);

// ----- unfold and refold -----

const bendFace = `qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0)`;
const unfold = call('sheetMetalUnfold', 'unfold1', {
  unfoldAll: 'false', partToUnfold: 'qNothing()', entitiesToUnfold: bendFace, holdEntity: at('FACE', 0.5, 0, 0.5),
});
batch.add('sheet-metal-unfold', 'the bend of an L unfolded, holding the long wall', [lSection, unfold, counts]);
batch.add('sheet-metal-unfold', 'the L unfolded and folded again', [
  lSection, unfold,
  call('sheetMetalRefold', 'refold1', { toRefold: part }),
  counts,
]);

// ----- partial flanges -----

const partial = (fields) => ({
  isPartialFlange: 'true', chainType: 'SMPartialFlangeChainType.PER_EDGE', flipFlangeBounds: 'false', holdAdjacentEdges: 'false',
  firstBoundType: 'SMFlangeBoundingType.BLIND', ...fields,
});
batch.add('sheet-metal-flange', 'a flange on the middle 20 mm of an edge', [
  plate, flange(at('EDGE', 1, 0.5, 1), partial({ firstBoundOffset: mm(5), hasSecondBound: 'true', secondBoundType: 'SMFlangeBoundingType.BLIND', secondBoundOffset: mm(5) })),
  counts,
]);
batch.add('sheet-metal-flange', 'a flange stopping 10 mm short of one end of an edge', [
  plate, flange(at('EDGE', 1, 0.5, 1), partial({ firstBoundOffset: mm(10), hasSecondBound: 'false' })), counts,
]);

batch.write('tools/fixtures/gen-batch-015.mjs');
