// Generates fixture batch 069: a rolled wall with a flange beyond it (one part, joined by a bend): the rolled wall
// unfolded holding the flange, refolded, and the whole part unfolded.
//
// Usage: node tools/fixtures/gen-batch-069.mjs
//
// Writes fixtures/batch-069/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, deg, mm, planeAt, sketch, solve } from './lib.mjs';

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

const batch = new Batch({ number: 69, title: 'a rolled wall with a flange beyond it, unfolded and refolded', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const probe = returns({ counts: 'bodyCounts(context)', faces: `faceAreas(context, ${part})` });
// A half tube of radius 20 about the origin, from (20, 0) round through y > 0 to (-20, 0), 25 mm tall, with a 10 mm
// flange from its straight edge at (20, 0).
const rolled = [
  sketch('s1', xy), arc('s1', 'a1', [20, 0], [0, 20], [-20, 0]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  call('sheetMetalFlange', 'flange1', {
    edges: `qClosestTo(qOwnedByBody(${part}, EntityType.EDGE), vector(20, 0, 12.5) * millimeter)`,
    flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(10),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
  }),
];
// The flange runs in from (20, 0) towards the axis: its inner face's middle is near (15, 0, 12.5).
const hold = `qClosestTo(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.PLANE), vector(15, 0, 12.5) * millimeter)`;
const unfoldRolled = call('sheetMetalUnfold', 'unfold1', {
  unfoldAll: 'false', partToUnfold: 'qNothing()',
  entitiesToUnfold: `qLargest(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER))`,
  holdEntity: hold,
});
batch.add('sheet-metal-flange', 'a half tube of radius 20 with a 10 mm flange from its straight edge at (20, 0)', [rolled, probe]);
batch.add('sheet-metal-unfold', 'the same, the rolled wall unfolded holding the flange', [rolled, unfoldRolled, probe]);
batch.add('sheet-metal-refold', 'the same, refolded', [rolled, unfoldRolled, call('sheetMetalRefold', 'refold1', { toRefold: part }), probe]);
batch.add('sheet-metal-unfold', 'the same, every bend and rolled wall unfolded holding the flange', [
  rolled, call('sheetMetalUnfold', 'unfold1', { unfoldAll: 'true', partToUnfold: part, entitiesToUnfold: 'qNothing()', holdEntity: hold }), probe,
]);

batch.write('tools/fixtures/gen-batch-069.mjs');
