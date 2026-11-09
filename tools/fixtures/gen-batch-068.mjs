// Generates fixture batch 068: sheet metal still to check: a rolled wall between two flat walls unfolded (holding a
// flat wall) and refolded, and an open box shelled from a block recognized as sheet metal (rips up its corners).
//
// Usage: node tools/fixtures/gen-batch-068.mjs
//
// Writes fixtures/batch-068/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, faceAt, line, mm, planeAt, sketch, solidsOf, solve } from './lib.mjs';

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

// The part's faces' areas and kinds, to compare.
function faceAreas(context is Context, part is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(part, EntityType.FACE)))
    {
        const s = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true });
        out = append(out, { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6), "type" : s.surfaceType });
    }
    return out;
}
`;

const batch = new Batch({ number: 68, title: 'rolled walls between flat walls unfolded and refolded, a shelled box recognized with rips', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const extruded = (s) =>
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges(s), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  });
const probe = returns({ counts: 'bodyCounts(context)', faces: `faceAreas(context, ${part})` });
// A flat wall from (-30, 0) to (0, 0), a quarter circle of radius 20 about (0, 20) up to (20, 20), and a flat wall on
// up to (20, 40): tangent all along.
const sCurve = [
  sketch('s1', xy), line('s1', 'l1', [-30, 0], [0, 0]), arc('s1', 'a1', [0, 0], [14.142135623730951, 5.857864376269049], [20, 20]),
  line('s1', 'l2', [20, 20], [20, 40]), solve('s1'), extruded('s1'),
];
const unfoldRolled = call('sheetMetalUnfold', 'unfold1', {
  unfoldAll: 'false', partToUnfold: 'qNothing()',
  entitiesToUnfold: `qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0)`,
  holdEntity: `qClosestTo(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.PLANE), vector(-15, 0, 12.5) * millimeter)`,
});
batch.add('sheet-metal-unfold', 'a rolled wall between two flat walls, unfolded holding the first flat wall', [sCurve, unfoldRolled, probe]);
batch.add('sheet-metal-refold', 'the same, refolded', [sCurve, unfoldRolled, call('sheetMetalRefold', 'refold1', { toRefold: part }), probe]);

const base = solidsOf('base');
batch.add('sheet-metal-recognize', 'a 40 x 30 x 20 block shelled 2 mm from its top, recognized with 1 mm bends', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('shell', 'shell1', { entities: faceAt(base, [20, 15, 20]), thickness: mm(2) }),
  call('sheetMetalRecognize', 'recognize1', { bodies: base, radius: mm(1), changeThickness: 'false', keepInputParts: 'false', kFactor: '0.45', minimalClearance: mm(0.02) }),
  returns({ counts: 'bodyCounts(context)', faces: `faceAreas(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID))` }),
]);

batch.write('tools/fixtures/gen-batch-068.mjs');
