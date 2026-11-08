// Generates fixture batch 013: more sheet metal. Bending a plate along a line, jogs, changing joints, a corner
// relief, ending a model, tabs, and the other flange alignments.
//
// Usage: node tools/fixtures/gen-batch-013.mjs
//
// Writes fixtures/batch-013/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, deg, line, mm, planeAt, rect, sketch, solve } from './lib.mjs';

const helpers = `
// The point at fractions (fx, fy, fz) of a part's bounding box.
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

// An edge, face or vertex of a part through a point at fractions of its bounding box: where sheet metal puts its
// material depends on the model, so entities are found from the part itself.
function atFraction(context is Context, part is Query, entityType is EntityType, fx is number, fy is number, fz is number) returns Query
{
    return qContainsPoint(qOwnedByBody(part, entityType), pointAtFraction(context, part, fx, fy, fz));
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "sheets" : evaluateQueryCount(context, qBodyType(all, BodyType.SHEET)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({ number: 13, title: 'sheet metal: bends, jogs, joints, corners, end, tabs, flange alignments', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const smParts = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const part = smParts('sm1');
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
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
// A line across the plate's top, at x = 25.
const bendLine = [sketch('bl', planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])), line('bl', 'l1', [25, -5], [25, 35]), solve('bl')];
const bendFields = {
  bendReference: wireEdges('bl'), holdOtherSide: 'false', face: at('FACE', 0.5, 0.5, 1), bendAlignment: 'BendAlignment.BEND_LINE',
  angleControlType: 'BendAngleControlType.BEND_ANGLE', oppositeAngle: 'false', useDefaultRadius: 'true', useDefaultKFactor: 'true',
};
const flange = (name, edges, fields = {}) =>
  call('sheetMetalFlange', name, {
    edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', ...fields,
  });

// ----- bends and jogs -----

batch.add('sheet-metal-bend', 'a plate bent up 90 degrees along a line across it', [
  plate, bendLine, call('sheetMetalBend', 'bend1', { ...bendFields, bendAngle: deg(90) }), counts,
]);
batch.add('sheet-metal-bend', 'a plate bent 60 degrees the other way, holding the other side', [
  plate, bendLine,
  call('sheetMetalBend', 'bend1', { ...bendFields, bendAngle: deg(60), oppositeAngle: 'true', holdOtherSide: 'true' }), counts,
]);
batch.add('sheet-metal-jog', 'a 5 mm jog along a line across a plate', [
  plate, bendLine,
  call('sheetMetalJog', 'jog1', {
    ...bendFields, bendAngleSetProgrammatically: 'false', bendAngle: deg(90), offsetType: 'JogOffsetBoundingType.BLIND',
    bendOffset: mm(5), bendOffsetAnchor: 'JogOffsetAnchor.OUTSIDE', preserveMaterial: 'false',
  }),
  counts,
]);

// ----- joints, corners, end -----

batch.add('sheet-metal-joint', 'the bend of an L made a rip', [
  lSection,
  call('sheetMetalJoint', 'joint1', { entity: `qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0)`, jointType: 'SMJointType.RIP', hasStyle: 'false' }),
  counts,
]);
batch.add('sheet-metal-joint', 'the bend of an L given a 5 mm radius', [
  lSection,
  call('sheetMetalJoint', 'joint1', {
    entity: `qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0)`, jointType: 'SMJointType.BEND',
    useDefaultRadius: 'false', radius: mm(5), useDefaultKFactor: 'true',
  }),
  counts,
]);
batch.add('sheet-metal-corner', 'a round relief where two flanges meet', [
  plate,
  flange('flange1', `qUnion([${at('EDGE', 1, 0.5, 1)}, ${at('EDGE', 0.5, 1, 1)}])`),
  call('sheetMetalCorner', 'corner1', {
    corner: `qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), vector(40, 30, 2) * millimeter)`, cornerStyle: 'SMCornerReliefStyle.ROUND',
    cornerReliefScale: '1.5',
  }),
  counts,
]);
batch.add('sheet-metal-end', 'a plate with a flange, its model ended', [
  plate, flange('flange1', at('EDGE', 1, 0.5, 1)), call('sheetMetalEnd', 'end1', { sheetMetalParts: part }), counts,
]);

// ----- tabs and flange alignments -----

batch.add('sheet-metal-tab', 'a tab on the edge of a plate', [
  plate,
  sketch('t1', planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])), rect('t1', 'rect1', [40, 10], [50, 20]), solve('t1'),
  call('sheetMetalTab', 'tab1', { tabFaces: `qSketchRegion(id + "t1")`, booleanUnionScope: at('FACE', 0.5, 0.5, 1), booleanOffset: mm(0) }),
  counts,
]);
for (const [alignment, description] of [['OUTER', 'outer'], ['MIDDLE', 'middle'], ['BEND', 'hold line']]) {
  batch.add('sheet-metal-flange', `a 15 mm flange aligned to the ${description}`, [
    plate, flange('flange1', at('EDGE', 1, 0.5, 1), { flangeAlignment: `SMFlangeAlignment.${alignment}` }), counts,
  ]);
}

batch.write('tools/fixtures/gen-batch-013.mjs');
