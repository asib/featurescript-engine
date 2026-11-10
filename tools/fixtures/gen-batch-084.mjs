// Generates fixture batch 084: batch 083's hold lines and cliff edge inside a split upright, with the upright's flip
// that points it towards the blend; a conic fillet of a cone frustum's top edge; a curvature fillet of the concave
// edge round a boss on a block; a conic face blend between a floor and an upright cylinder.
//
// Usage: node tools/fixtures/gen-batch-084.mjs
//
// Writes fixtures/batch-084/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, edgeAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

const helpers = `
// Each face of a body: its area and its surface's type.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType;
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 84, title: 'hold lines and a cliff edge inside a side, fillets of circular edges on a cone and round a boss, a conic blend with a cylinder', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const facesOf = (name) => `qOwnedByBody(${sheetsOf(name)}, EntityType.FACE)`;
const blendFields = {
  propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS', asymmetric: 'false',
  crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false', conicHoldLines: 'false', hasCliffEdges: 'false',
  hasCaps: 'false', hasLimits: 'false', hasHelpPoint: 'false', trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false',
  showIsocurves: 'false',
};

// ----- hold lines and a cliff edge inside a side -----

// Batch 083's upright at y = 0 of two faces, z 0..4 and 4..20 (x 0..30), and a level wall z = 0 (x 0..30, y 0..20);
// the upright not flipped, so its sense points towards the wall.
const corner = [
  sketch('s1', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), line('s1', 'l1', [0, 0], [0, 4]), line('s1', 'l2', [0, 4], [0, 20]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'false' }),
  sketch('s2', PLANES.xz), line('s2', 'l1', [0, 0], [30, 0]), solve('s2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const innerEdge = `qContainsPoint(qOwnedByBody(${sheetsOf('floor')}, EntityType.EDGE), ${vec3(15, 0, 4)})`;
const cornerBlend = (fields) =>
  call('faceBlend', 'blend1', {
    side1: facesOf('floor'), flipSide1: 'false', side2: facesOf('wall'), flipSide2: 'false', radius: mm(5), ...blendFields, ...fields,
  });
const cornerProbe = returns({ floor: `faceProbe(context, ${sheetsOf('floor')})` });
batch.add('face-blend', 'an upright of two faces (split 4 mm up) and a level wall, blended 5 mm, the split a tangent hold line', [
  corner, cornerBlend({ tangentHoldLines: 'true', tangentEdges: innerEdge, inverseTangentEdges: 'qNothing()' }), cornerProbe,
]);
batch.add('face-blend', 'the same, the split an inverse tangent hold line', [
  corner, cornerBlend({ tangentHoldLines: 'true', tangentEdges: 'qNothing()', inverseTangentEdges: innerEdge }), cornerProbe,
]);
batch.add('face-blend', 'the same, the split a conic hold line', [
  corner, cornerBlend({ conicHoldLines: 'true', conicEdges: innerEdge, inverseConicEdges: 'qNothing()' }), cornerProbe,
]);
batch.add('face-blend', 'the same, the split a cliff edge', [corner, cornerBlend({ hasCliffEdges: 'true', cliffEdges: innerEdge }), cornerProbe]);

// ----- fillets of circular edges -----

const base = solidsOf('base');
const filletProbe = returns({ faces: `faceProbe(context, ${base})` });
// A cone frustum: radius 10 at z = 0 to 6 at z = 20 (a revolved trapezoid).
batch.add('fillet', 'a cone frustum (radius 10 at the bottom, 6 at the top, 20 tall): its top edge filleted with a conic section (rho 0.7), 2 mm', [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [6, 20]), line('s1', 'l3', [6, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1'),
  call('revolve', 'base', { entities: regions('s1'), axis: `qContainsPoint(qCreatedBy(id + "s1", EntityType.EDGE), ${vec3(0, 0, 10)})`, fullRevolve: 'true' }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [6, 0, 20]), radius: mm(2), crossSection: 'FilletCrossSection.CONIC', nonCircularRadius: mm(2), rho: '0.7' }),
  filletProbe,
]);
// A 40 × 40 × 10 block with a boss of radius 8 on its top, 10 tall; the concave edge round the boss's foot.
batch.add('fillet', 'a 40 × 40 × 10 block with a boss of radius 8, 10 tall, on its top: the edge round the boss\'s foot filleted with a curvature section (magnitude 0.5), 3 mm', [
  box('base', [-20, -20, 0], [20, 20, 10]),
  sketch('s2', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), circle('s2', 'c1', [0, 0], 8), solve('s2'),
  call('extrude', 'boss', { entities: regions('s2'), endBound: 'BoundingType.BLIND', depth: mm(10), operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [8, 0, 10]), radius: mm(3), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(3), magnitude: '0.5' }),
  filletProbe,
]);

// ----- a conic blend with a cylinder -----

// A floor sheet (z = 0, 40 × 40 about the origin) and an upright cylinder sheet of radius 10 about z, 20 tall.
batch.add('face-blend', 'a floor sheet and an upright cylinder sheet of radius 10 on it, blended 3 mm with a conic section (rho 0.7)', [
  sketch('s1', PLANES.xy), rect('s1', 'r1', [-20, -20], [20, 20]), solve('s1'),
  sketch('s2', PLANES.xy), circle('s2', 'c1', [0, 0], 10), solve('s2'),
  call('extrude', 'cyl', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
  call('faceBlend', 'blend1', {
    side1: facesOf('s1'), flipSide1: 'false', side2: facesOf('cyl'), flipSide2: 'false', radius: mm(3), ...blendFields,
    crossSectionShape: 'FaceBlendCrossSectionShape.CONIC', rho: '0.7',
  }),
  returns({ floor: `faceProbe(context, ${sheetsOf('s1')})` }),
]);

batch.write('tools/fixtures/gen-batch-084.mjs');
