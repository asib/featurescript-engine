// Generates fixture batch 083: face blends between a floor and the two walls of one Π-shaped sheet (two solutions),
// with a help point picking one and without; and between a floor and an upright of two faces, its inner edge 4 mm up
// a tangent hold line, an inverse tangent hold line, a conic hold line and a cliff edge.
//
// Usage: node tools/fixtures/gen-batch-083.mjs
//
// Writes fixtures/batch-083/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, line, mm, planeAt, rect, sketch, solve, vec3, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 83, title: 'face blends with two solutions and a help point, hold lines and a cliff edge inside a side', helpers });

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

// ----- two solutions -----

// A floor sheet z = 0 (x 0..30, y -5..25); a Π-shaped sheet standing on it: walls at y = 0 and y = 20 (15 tall)
// joined by a roof at z = 15, extruded 30 along x from the yz plane. The blend between the floor and the two walls
// (their inner sides) can go at either wall.
const channel = [
  sketch('s1', PLANES.xy), rect('s1', 'r1', [0, -5], [30, 25]), solve('s1'),
  sketch('s2', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])),
  line('s2', 'l1', [0, 0], [0, 15]), line('s2', 'l2', [0, 15], [20, 15]), line('s2', 'l3', [20, 15], [20, 0]), solve('s2'),
  call('extrude', 'walls', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'false' }),
];
const wallFaces = `qUnion([qContainsPoint(${facesOf('walls')}, ${vec3(15, 0, 7)}), qContainsPoint(${facesOf('walls')}, ${vec3(15, 20, 7)})])`;
const channelBlend = (fields) =>
  call('faceBlend', 'blend1', {
    side1: facesOf('s1'), flipSide1: 'false', side2: wallFaces, flipSide2: 'false', radius: mm(3), ...blendFields, ...fields,
  });
const channelProbe = returns({ floor: `faceProbe(context, ${sheetsOf('s1')})` });
batch.add('face-blend', 'a floor sheet and a Π-shaped sheet\'s two walls standing on it 20 mm apart, blended 3 mm, a help point at (15, 18, 1) picking one', [
  channel, `opPoint(context, id + "help", { "point" : ${vec3(15, 18, 1)} });`,
  channelBlend({ hasHelpPoint: 'true', helpPoint: `qCreatedBy(id + "help", EntityType.VERTEX)` }), channelProbe,
]);
batch.add('face-blend', 'the same, no help point', [channel, channelBlend({}), channelProbe]);

// ----- hold lines and a cliff edge inside a side -----

// An upright sheet at y = 0 of two faces, z 0..4 and 4..20 (x 0..30), and a level wall z = 0 (x 0..30, y 0..20).
const corner = [
  sketch('s1', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), line('s1', 'l1', [0, 0], [0, 4]), line('s1', 'l2', [0, 4], [0, 20]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'false' }),
  sketch('s2', PLANES.xz), line('s2', 'l1', [0, 0], [30, 0]), solve('s2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const innerEdge = `qContainsPoint(qOwnedByBody(${sheetsOf('floor')}, EntityType.EDGE), ${vec3(15, 0, 4)})`;
const cornerBlend = (fields) =>
  call('faceBlend', 'blend1', {
    side1: facesOf('floor'), flipSide1: 'true', side2: facesOf('wall'), flipSide2: 'false', radius: mm(5), ...blendFields, ...fields,
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

batch.write('tools/fixtures/gen-batch-083.mjs');
