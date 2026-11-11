// Generates fixture batch 104: a second Part Studio drawn in Onshape's UI ("Part Studio 3"), as Onshape writes it
// (fixtures/blobs/ui-sketch-2.studio.fs): a closed spline trimmed by a line across it (an interpolated spline segment
// on a periodic geometry), a sketch point and a spline segment made by "Use" of the trimmed spline (skSplineSegment),
// a closed spline, and a closed spline made by "Use" of it (skSpline). The tab imported and built: each sketch's
// curves, vertices, regions and bodies.
//
// Usage: node tools/fixtures/gen-batch-104.mjs
//
// Writes fixtures/batch-104/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, uploadImport } from './lib.mjs';

const helpers = `
function roundAll(a is array) returns array
{
    return mapArray(a, function(x) { return x is number ? roundToPrecision(x, 6) : (x is array ? roundAll(x) : x); });
}

function mm(p is Vector) returns array
{
    return roundAll([p[0] / millimeter, p[1] / millimeter, p[2] / millimeter]);
}

// A sketch edge's curve, in millimetres.
function curveOf(context is Context, edge is Query) returns map
{
    const c = evCurveDefinition(context, { "edge" : edge });
    if (c is Line)
        return { "line" : mm(c.origin), "direction" : roundAll([c.direction[0], c.direction[1], c.direction[2]]), "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) };
    if (c is Circle)
        return { "circle" : mm(c.coordSystem.origin), "x" : roundAll([c.coordSystem.xAxis[0], c.coordSystem.xAxis[1], c.coordSystem.xAxis[2]]), "normal" : roundAll([c.coordSystem.zAxis[0], c.coordSystem.zAxis[1], c.coordSystem.zAxis[2]]), "radius" : roundToPrecision(c.radius / millimeter, 6), "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) };
    if (c is BSplineCurve)
        return { "degree" : c.degree, "rational" : c.isRational, "periodic" : c.isPeriodic, "knots" : roundAll(c.knots), "poles" : mapArray(c.controlPoints, function(p) { return mm(p); }), "weights" : c.weights is array ? roundAll(c.weights) : undefined };
    return { "other" : true };
}

// A sketch's bodies of each type in a context, each with its edges' count and its vertices' points.
function sketchBodies(context is Context, sketchId is Id) returns map
{
    const describe = function(b) { return { "edges" : size(evaluateQuery(context, qOwnedByBody(b, EntityType.EDGE))), "points" : sort(mapArray(evaluateQuery(context, qOwnedByBody(b, EntityType.VERTEX)), function(v) { return mm(evVertexPoint(context, { "vertex" : v })); }), function(a, b) { return a[0] == b[0] ? a[1] - b[1] : a[0] - b[0]; }) }; };
    const bodies = qCreatedBy(sketchId, EntityType.BODY);
    return { "wire" : mapArray(evaluateQuery(context, qBodyType(bodies, BodyType.WIRE)), describe), "sheet" : mapArray(evaluateQuery(context, qBodyType(bodies, BodyType.SHEET)), describe), "point" : mapArray(evaluateQuery(context, qBodyType(bodies, BodyType.POINT)), describe) };
}

// Each sketch entity's edges' curves, its vertices' points and its regions' areas in a context.
function sketchOf(context is Context, sketchId is Id, entities is array) returns map
{
    var out = {};
    for (var entity in entities)
        out[entity] = mapArray(evaluateQuery(context, sketchEntityQuery(sketchId, EntityType.EDGE, entity)), function(e) { return curveOf(context, e); });
    out.vertices = size(evaluateQuery(context, qCreatedBy(sketchId, EntityType.VERTEX)));
    out.regions = sort(mapArray(evaluateQuery(context, qCreatedBy(sketchId, EntityType.FACE)), function(f) { return roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 4); }), function(a, b) { return a - b; });
    return out;
}
`;

const batch = new Batch({
  number: 104,
  title: "a second Part Studio drawn in Onshape's UI: trimmed and used splines",
  helpers,
  imports: [uploadImport('Ui2', 'ui-sketch-2.studio.fs')],
});

const sketches = [
  ['FLPhbb9wuHHouVB_0', ['9ffB4A0wATzy', 'MRvD9oGg3gnl'], 'a closed spline trimmed by a line across it, and the line'],
  ['FCJvi4vlHym7BwB_0', ['0gjWwWrrrgiU.0'], 'a point made by Use'],
  ['FXdsBkCz56RgE2Z_0', ['aYnjAS7ao6xm.0'], 'a spline segment made by Use of the trimmed spline (skSplineSegment)'],
  ['FTZCQcyfkOGh48w_0', ['f1m6jaIn00dC'], 'a closed spline through four points'],
  ['FQhQQJN7nZIqYGl_0', ['5DmMJN1TSQ0F.0.0'], 'a closed spline made by Use of that one (skSpline)'],
];
for (const [sketch, entities, what] of sketches) {
  batch.add('studio', `the tab built: its sketch of ${what}: curves, vertices, regions and bodies`, [
    'const built = Ui2::build({});',
    `return { "sketch" : sketchOf(built, makeId("${sketch}"), [${entities.map((e) => `"${e}"`).join(', ')}]), "bodies" : sketchBodies(built, makeId("${sketch}")) };`,
  ]);
}

batch.write('tools/fixtures/gen-batch-104.mjs');
