// Generates fixture batch 085: a cone frustum's side replaced by a coaxial cylinder; isoparametric curves on a full
// torus about z both ways and on one about x, with the tori's surface definitions (Onshape's torus parameters, for
// batch 073's gap); a variable conic fillet of two edges meeting at a block's corner.
//
// Usage: node tools/fixtures/gen-batch-085.mjs
//
// Writes fixtures/batch-085/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, circle, edgeAt, edgeOf, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, vertexAt, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

function dir6(v) returns array
{
    return [roundToPrecision(v[0], 6), roundToPrecision(v[1], 6), roundToPrecision(v[2], 6)];
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

// Three points along each edge of a body (its ends and middle, in its own parameter).
function edgePoints(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, mapArray(lines, function(l) { return round6(l.origin); }));
    }
    return out;
}

// A torus face's coordinate system and its points at the corners and middle of its parameter range.
function torusProbe(context is Context, face is Query) returns map
{
    const s = evSurfaceDefinition(context, { "face" : face });
    const cs = s.coordSystem;
    const planes = evFaceTangentPlanes(context, { "face" : face, "parameters" : [vector(0, 0), vector(0.25, 0), vector(0, 0.25), vector(0.5, 0.5)] });
    return { "origin" : round6(cs.origin), "xAxis" : dir6(cs.xAxis), "zAxis" : dir6(cs.zAxis),
             "points" : mapArray(planes, function(p) { return round6(p.origin); }) };
}
`;

const batch = new Batch({ number: 85, title: 'a cone side replaced by a cylinder, isoparametric curves and parameters of full tori, a variable fillet at a corner', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const base = solidsOf('base');

// ----- a cone side replaced by a cylinder -----

// A cone frustum revolved from a trapezoid (radius 10 at z = 0 to 6 at z = 20) about a separate axis line.
batch.add('replace-face', 'a cone frustum (radius 10 to 6, 20 tall): its side replaced by a coaxial cylinder of radius 8', [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [6, 20]), line('s1', 'l3', [6, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('s1'), axis: edgeOf('axis'), fullRevolve: 'true' }),
  sketch('s2', PLANES.xy), circle('s2', 'c1', [0, 0], 8), solve('s2'),
  call('extrude', 'tool', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'false' }),
  call('replaceFace', 'replace1', {
    replaceFaces: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`, templateFace: `qOwnedByBody(${sheetsOf('tool')}, EntityType.FACE)`,
    oppositeSense: 'false', offset: mm(0),
  }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

// ----- full tori -----

// The circle of radius 5 about (20, 0) in the xz plane revolved all the way about z; and the circle of radius 5
// about (0, 20) in the xy plane revolved all the way about x.
const torusZ = [
  sketch('profile', PLANES.xz), circle('profile', 'c1', [20, 0], 5), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'true' }),
];
const torusX = [
  sketch('profile', PLANES.xy), circle('profile', 'c1', [0, 20], 5), solve('profile'), axisLine('axis', [-50, 0, 0], [50, 0, 0]),
  call('revolve', 'base', { entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'true' }),
];
const torusFace = `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.TORUS)`;
const iso = (direction) => [
  call('isoparametricCurve', 'curve1', { face: torusFace, directionType: `DirectionType.${direction}`, equalSpacing: 'true', nCurves: '4' }),
  returns({
    torus: `torusProbe(context, ${torusFace})`,
    periodicity: `[evFacePeriodicity(context, { "face" : ${torusFace} }), evFacePeriodicity(context, { "face" : ${torusFace}, "trimmed" : false })]`,
    curves: `edgePoints(context, qCreatedBy(id + "curve1", EntityType.BODY))`,
  }),
];
batch.add('isoparametric-curve', 'four isoparametric curves in the U direction on a full torus about z, and its parameters', [torusZ, iso('U_DIRECTION')]);
batch.add('isoparametric-curve', 'four in the V direction on the same torus', [torusZ, iso('V_DIRECTION')]);
batch.add('isoparametric-curve', 'four in the U direction on a full torus about x (the circle about (0, 20) in the xy plane revolved)', [torusX, iso('U_DIRECTION')]);

// ----- a variable fillet at a corner -----

batch.add('fillet', 'a variable conic fillet (rho 0.7) of a block\'s top front and top right edges, meeting at a corner: 2 mm at the far ends, 4 mm at the corner', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('fillet', 'fillet1', {
    entities: `qUnion([${edgeAt(base, [20, 0, 12])}, ${edgeAt(base, [40, 15, 12])}])`, radius: mm(3), nonCircularRadius: mm(3), isVariable: 'true',
    crossSection: 'FilletCrossSection.CONIC', rho: '0.7', smoothTransition: 'true',
    vertexSettings: `[${[[0, 0, 2], [40, 0, 4], [40, 30, 2]].map(([x, y, r]) => `{ "vertex" : ${vertexAt(base, [x, y, 12])}, "vertexRadius" : ${mm(r)}, "variableRho" : 0.7 }`).join(', ')}]`,
  }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

batch.write('tools/fixtures/gen-batch-085.mjs');
