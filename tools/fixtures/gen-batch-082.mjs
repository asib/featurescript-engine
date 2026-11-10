// Generates fixture batch 082: constant conic, curvature and asymmetric fillets of a cylinder's top edge; a face
// blend between a floor and two parallel uprights standing on it, with a help point picking one blend and without;
// two circles lofted with a connection between points along their edges.
//
// Usage: node tools/fixtures/gen-batch-082.mjs
//
// Writes fixtures/batch-082/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, cylinder, edgeAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

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

// Each edge of a body that isn't straight: three points along it.
function curvedEdges(context is Context, body is Query) returns array
{
    var out = [];
    const edges = qOwnedByBody(body, EntityType.EDGE);
    for (var edge in evaluateQuery(context, qSubtraction(edges, qGeometry(edges, GeometryType.LINE))))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, mapArray(lines, function(l) { return round6(l.origin); }));
    }
    return out;
}
`;

const batch = new Batch({
  number: 82,
  title: 'conic, curvature and asymmetric fillets of a circular edge, a face blend with two solutions and a help point, circles lofted with a connection',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;

// ----- fillets of a circular edge -----

const base = solidsOf('base');
const topEdge = edgeAt(base, [10, 0, 20]);
const filletProbe = returns({ faces: `faceProbe(context, ${base})` });
batch.add('fillet', 'a cylinder of radius 10, 20 tall: its top edge filleted with a conic section (rho 0.7), 3 mm', [
  cylinder('base', [0, 0], 10, 0, 20),
  call('fillet', 'fillet1', { entities: topEdge, radius: mm(3), crossSection: 'FilletCrossSection.CONIC', nonCircularRadius: mm(3), rho: '0.7' }),
  filletProbe,
]);
batch.add('fillet', 'the same, a curvature section (magnitude 0.5)', [
  cylinder('base', [0, 0], 10, 0, 20),
  call('fillet', 'fillet1', { entities: topEdge, radius: mm(3), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(3), magnitude: '0.5' }),
  filletProbe,
]);
batch.add('fillet', 'the same, asymmetric: 3 mm and 2 mm', [
  cylinder('base', [0, 0], 10, 0, 20),
  call('fillet', 'fillet1', { entities: topEdge, radius: mm(3), isAsymmetric: 'true', otherRadius: mm(2), flipAsymmetric: 'false' }),
  filletProbe,
]);

// ----- a face blend with two solutions -----

// A floor sheet z = 0 (x 0..30, y -5..25) and two uprights standing on it at y = 0 and y = 20 (x 0..30, 15 tall), one
// sheet of two faces; blended 3 mm between the floor and the uprights, on their inner sides.
const channel = [
  sketch('s1', PLANES.xy), rect('s1', 'r1', [0, -5], [30, 25]), solve('s1'),
  sketch('s2', PLANES.xy), line('s2', 'l1', [0, 0], [30, 0]), line('s2', 'l2', [0, 20], [30, 20]), solve('s2'),
  call('extrude', 'walls', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(15), oppositeDirection: 'false' }),
];
const blendFields = {
  propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS', asymmetric: 'false',
  crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false', conicHoldLines: 'false', hasCliffEdges: 'false',
  hasCaps: 'false', hasLimits: 'false', trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false', showIsocurves: 'false',
};
const channelBlend = (help) =>
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('s1')}, EntityType.FACE)`, flipSide1: 'false', side2: `qOwnedByBody(${sheetsOf('walls')}, EntityType.FACE)`,
    flipSide2: 'false', radius: mm(3), ...blendFields,
    ...(help ? { hasHelpPoint: 'true', helpPoint: `qCreatedBy(id + "help", EntityType.VERTEX)` } : { hasHelpPoint: 'false' }),
  });
const blendProbe = returns({ floor: `faceProbe(context, ${sheetsOf('s1')})`, walls: `faceProbe(context, ${sheetsOf('walls')})` });
batch.add('face-blend', 'a floor sheet and two parallel upright sheets standing on it 20 mm apart, blended 3 mm, a help point at (15, 18, 1) picking one', [
  channel, `opPoint(context, id + "help", { "point" : ${vec3(15, 18, 1)} });`, channelBlend(true), blendProbe,
]);
batch.add('face-blend', 'the same, no help point', [channel, channelBlend(false), blendProbe]);

// ----- circles lofted with a connection -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const circleEdge = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.SHEET), EntityType.EDGE)`;
batch.add('loft', 'circles of radius 10 at z = 0 and z = 20, connected from a quarter along the first to half along the second', [
  sketch('s1', at(0)), circle('s1', 'c1', [0, 0], 10), solve('s1'), sketch('s2', at(20)), circle('s2', 'c1', [0, 0], 10), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }]`, matchConnections: 'true',
    connections: `[{ "connectionEntities" : qUnion([${circleEdge('s1')}, ${circleEdge('s2')}]), "connectionEdgeQueries" : qUnion([${circleEdge('s1')}, ${circleEdge('s2')}]), "connectionEdgeParameters" : [0.25, 0.5] }]`,
  }),
  returns({ faces: `faceProbe(context, ${solidsOf('loft1')})`, curves: `curvedEdges(context, ${solidsOf('loft1')})` }),
]);

batch.write('tools/fixtures/gen-batch-082.mjs');
