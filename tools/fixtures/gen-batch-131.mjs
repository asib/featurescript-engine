// Generates fixture batch 131: everyday sheet metal past batch 013's one of each. A plate bent along a line with
// each of the other bend alignments, along a slanted line, and with its own radius; jogs anchored inside and on
// the nominal line, a jog the plate's thickness times two, a jog keeping its material; corners where two flanges
// meet relieved by each of the other styles; an L's bend made a rip with an edge joint and a butt joint.
//
// Usage: node tools/fixtures/gen-batch-131.mjs
//
// Writes fixtures/batch-131/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 131, title: 'sheet metal: bend alignments, slanted bends, jogs, corner reliefs, rip joints', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const counts = returns({ counts: 'bodyCounts(context)' });

const start = (fields) =>
  call('sheetMetalStart', 'sm1', { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', ...fields });
// A 40 × 30 plate, 2 thick, bends of radius 1.
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  start({ process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()' }),
];
// An L of legs 30 and 20, 25 deep.
const lSection = [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [30, 20]), solve('s1'),
  start({
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
];
// A line across the plate's top, at x = 25 (or from (15, -5) to (35, 35), slanted).
const bendLine = (from = [25, -5], to = [25, 35]) => [sketch('bl', planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])), line('bl', 'l1', from, to), solve('bl')];
const bendFields = {
  bendReference: wireEdges('bl'), holdOtherSide: 'false', face: at('FACE', 0.5, 0.5, 1), bendAlignment: 'BendAlignment.BEND_LINE',
  angleControlType: 'BendAngleControlType.BEND_ANGLE', oppositeAngle: 'false', useDefaultRadius: 'true', useDefaultKFactor: 'true',
};
const jogFields = {
  ...bendFields, bendAngleSetProgrammatically: 'false', bendAngle: deg(90), offsetType: 'JogOffsetBoundingType.BLIND',
  bendOffset: mm(5), bendOffsetAnchor: 'JogOffsetAnchor.OUTSIDE', preserveMaterial: 'false',
};
const flange = (name, edges, fields = {}) =>
  call('sheetMetalFlange', name, {
    edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', ...fields,
  });
const bendFace = `qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0)`;

// ----- bends -----

for (const [alignment, name] of [['HELD_EDGE', 'hold line'], ['BENT_EDGE', 'hold other line'], ['BENT_INSIDE', 'inner'], ['BENT_OUTSIDE', 'outer'], ['BENT_MIDPLANE', 'middle']]) {
  batch.add('sheet-metal-bend', `a plate bent up 90 degrees along a line across it, aligned to the ${name}`, [
    plate, bendLine(), call('sheetMetalBend', 'bend1', { ...bendFields, bendAlignment: `BendAlignment.${alignment}`, bendAngle: deg(90) }), counts,
  ]);
}
batch.add('sheet-metal-bend', 'a plate bent up 45 degrees along a slanted line across it', [
  plate, bendLine([15, -5], [35, 35]), call('sheetMetalBend', 'bend1', { ...bendFields, bendAngle: deg(45) }), counts,
]);
batch.add('sheet-metal-bend', 'a plate bent up 90 degrees along a line across it, the bend of radius 4', [
  plate, bendLine(), call('sheetMetalBend', 'bend1', { ...bendFields, useDefaultRadius: 'false', bendRadius: mm(4), bendAngle: deg(90) }), counts,
]);

// ----- jogs -----

batch.add('sheet-metal-jog', 'a 5 mm jog along a line across a plate, anchored inside', [
  plate, bendLine(), call('sheetMetalJog', 'jog1', { ...jogFields, bendOffsetAnchor: 'JogOffsetAnchor.INSIDE' }), counts,
]);
batch.add('sheet-metal-jog', 'an 8 mm jog along a line across a plate, anchored at the nominal line', [
  plate, bendLine(), call('sheetMetalJog', 'jog1', { ...jogFields, bendOffset: mm(8), bendOffsetAnchor: 'JogOffsetAnchor.NOMINAL' }), counts,
]);
batch.add('sheet-metal-jog', 'a jog of twice the thickness along a line across a plate', [
  plate, bendLine(), call('sheetMetalJog', 'jog1', { ...jogFields, offsetType: 'JogOffsetBoundingType.THICKNESS', thicknessFactor: '2' }), counts,
]);
batch.add('sheet-metal-jog', 'a 5 mm jog along a line across a plate, keeping its material', [
  plate, bendLine(), call('sheetMetalJog', 'jog1', { ...jogFields, preserveMaterial: 'true' }), counts,
]);

// ----- corners -----

const corner = (fields) => [
  plate,
  flange('flange1', `qUnion([${at('EDGE', 1, 0.5, 1)}, ${at('EDGE', 0.5, 1, 1)}])`),
  call('sheetMetalCorner', 'corner1', { corner: `qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), vector(40, 30, 2) * millimeter)`, ...fields }),
  counts,
];
batch.add('sheet-metal-corner', 'a square relief 4 wide where two flanges meet', [
  corner({ cornerStyle: 'SMCornerReliefStyle.SIZED_RECTANGLE', squareReliefWidth: mm(4) }),
]);
batch.add('sheet-metal-corner', 'a rectangular relief scaled 2 where two flanges meet', [
  corner({ cornerStyle: 'SMCornerReliefStyle.RECTANGLE', cornerReliefScale: '2' }),
]);
batch.add('sheet-metal-corner', 'a round relief 5 across where two flanges meet', [
  corner({ cornerStyle: 'SMCornerReliefStyle.SIZED_ROUND', roundReliefDiameter: mm(5) }),
]);
batch.add('sheet-metal-corner', 'a closed corner where two flanges meet', [corner({ cornerStyle: 'SMCornerReliefStyle.CLOSED' })]);
batch.add('sheet-metal-corner', 'a simple corner where two flanges meet', [corner({ cornerStyle: 'SMCornerReliefStyle.SIMPLE' })]);

// ----- rip joints -----

for (const [style, name] of [['EDGE', 'an edge joint'], ['BUTT', 'a butt joint one way'], ['BUTT2', 'a butt joint the other way']]) {
  batch.add('sheet-metal-joint', `the bend of an L made a rip, ${name}`, [
    lSection,
    call('sheetMetalJoint', 'joint1', { entity: bendFace, jointType: 'SMJointType.RIP', hasStyle: 'true', jointStyle: `SMJointStyle.${style}` }),
    counts,
  ]);
}

batch.write('tools/fixtures/gen-batch-131.mjs');
