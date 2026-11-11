// Generates fixture batch 103: a Part Studio drawn in Onshape's UI, as Onshape writes it out (its "Show code",
// fixtures/blobs/ui-sketch.studio.fs): a sketch on Top of an open spline through four points with end handles, a
// closed spline through four, a cubic Bézier, a line, a circle, a half arc and a point, given by an initial guess
// (skSetInitialGuess) rather than by the sketch calls' own fields; and an extrude of the circle's and the closed
// spline's regions, found by compressed queries (qCompressed). The tab imported and built, its compressed queries
// decoded, its sketch and extrude run inline in a case, and its parts derived.
//
// Usage: node tools/fixtures/gen-batch-103.mjs
//
// Writes fixtures/batch-103/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { readFileSync } from 'node:fs';
import { Batch, uploadImport } from './lib.mjs';

const studio = readFileSync(new URL('../../fixtures/blobs/ui-sketch.studio.fs', import.meta.url), 'utf8');
const lines = studio.split('\n').map((l) => l.trim());
const compressed = [...studio.matchAll(/qCompressed\(1\.0,"([^"]*)",id\)/g)].map((m) => m[1]);
const [topPlane, circleRegion, splineRegion] = compressed;
// The sketch's statements, from its initial guess to its solve, and the extrude's.
const from = (start, end) => lines.slice(lines.findIndex((l) => l.startsWith(start)), lines.findIndex((l) => l.startsWith(end)) + 1);
const planeQuery = (l) => /^var \w+_query;$/.test(l) || (/^\w+_query=qCompressed/.test(l) && l.includes(topPlane));
const sketchLines = from('const initialGuess', 'skSolve(sketch);').filter((l) => l !== '{' && l !== '}' && !planeQuery(l) && !l.startsWith('annotation'));
const extrudeLines = from('var ufGUNhuRDXNJqr_query;', 'extrude(context').filter((l) => !l.startsWith('annotation'));

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
  number: 103,
  title: "a Part Studio drawn in Onshape's UI, as Onshape writes it",
  helpers,
  imports: [uploadImport('Ui', 'ui-sketch.studio.fs'), 'St::import(path : "onshape/std/math.fs", version : "3112.0");'],
});

const entities = ['8MiwA9l7j4dg', 'K4ASsjGTCmiS', 'Y2EsOpdVRahG', 'ogOM4F6dF4NB', 'jtR065vsGkZ5', 'rPK5drqJiVDG', 'h5uonviICl9I'];
const list = `[${entities.map((e) => `"${e}"`).join(', ')}]`;
const solids = 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)';
const volumes = (context, query) => `sort(mapArray(evaluateQuery(${context}, ${query}), function(b) { return roundToPrecision(evVolume(${context}, { "entities" : b }) / millimeter ^ 3, 3); }), function(a, b) { return a - b; })`;

// The Bézier's control points from its initial guess (after degree, rational, periodic and counts: x, y pairs), in
// metres, offset along x by `dx` metres.
const guess = JSON.parse('{' + studio.match(/initialGuess\w+ = \{(.*)\};/)[1] + '}');
const bezierPoints = (dx) => {
  const g = guess['Y2EsOpdVRahG'];
  const pts = [];
  for (let i = 0; i < g[4]; i++) pts.push(`vector(${g[5 + 2 * i] + dx}, ${g[6 + 2 * i]}) * meter`);
  return `[${pts.join(', ')}]`;
};
const withBezierPoints = (l, dx) => l.startsWith('skBezier(') ? l.replace('{ "construction"', `{ "points" : ${bezierPoints(dx)}, "construction"`) : l;
// A hand-encoded compressed query (fixtures/batch-103/README.md): the region of sketch `split` beside the imprint of
// its sketch entity `entity`, on side `side`.
const region = (entity, side) => `%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS8$TOPOLOGYS8$entitiesA1A2C0M5Sb$derivedFromC0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S5.6$splitwireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS${entity.length.toString(16)}$${entity}R5R6R7R8R9CcA1S-d.7$imprintRcS7$IMPRINTD${side}R5C7S4$FACER7R8R9R12RcR13`;

batch.add('studio', "the UI-drawn tab built: its sketch's curves, vertices and regions, and its parts' volumes", [
  'const built = Ui::build({});',
  `return { "sketch" : sketchOf(built, makeId("FeUNWBXWR3P4AUC_0"), ${list}), "parts" : ${volumes('built', solids)} };`,
]);
batch.add('compressed', "the tab's compressed queries decoded: the Top plane, and the circle's and closed spline's regions", [
  `return { "top" : qCompressed(1, "${topPlane}", newId()), "circle" : qCompressed(1, "${circleRegion}", newId()), "spline" : qCompressed(1, "${splineRegion}", newId()) };`,
]);
batch.add('inline', "the tab's sketch but its Bézier (on its Top plane query, rooted at the top) and extrude run inline", [
  `var sketch = newSketch(context, id + "FeUNWBXWR3P4AUC_0", { "sketchPlane" : qCompressed(1, "${topPlane}", newId()), "disableImprinting" : false });`,
  // Without the Bézier: a Feature Studio's skBezier wants points (std's precondition), and given them makes its own
  // snap constraints (a first build failed: "Duplicate id in sketch") and fails to solve with Onshape's guess for it
  // (a second: "Solve for sketch fails").
  sketchLines
    .map((l) => (l.startsWith('const initialGuess') ? `const initialGuessFeUNWBXWR3P4AUC_0 = ${JSON.stringify(Object.fromEntries(Object.entries(guess).filter(([k]) => k !== 'Y2EsOpdVRahG')))};` : l))
    .filter((l) => !l.startsWith('var sketch') && !l.includes('"Y2EsOpdVRahG')),
  extrudeLines,
  `return { "sketch" : sketchOf(context, id + "FeUNWBXWR3P4AUC_0", ${list}), "bodies" : sketchBodies(context, id + "FeUNWBXWR3P4AUC_0"), "parts" : ${volumes('context', `qCreatedBy(id + "FbGO9VXCeYzrs3h_0", EntityType.BODY)`)} };`,
]);
batch.add('derived', "the tab's parts derived", [
  `importDerived(context, id + "derived1", { "newUI" : true, "partStudio" : { "buildFunction" : Ui::build, "configuration" : {}, "partQuery" : ${solids} } as PartStudioData, "preserveActiveSheetMetal" : false, "location" : qNothing(), "placement" : DerivedPlacementType.AT_ORIGIN, "includeMateConnectors" : false, "includeProperties" : true });`,
  `return { "parts" : ${volumes('context', 'qCreatedBy(id + "derived1", EntityType.BODY)')} };`,
]);
batch.add('compressed', "the Top plane's compressed query counted rooted at the top and at the case's id", [
  `return { "top" : evaluateQueryCount(context, qCompressed(1, "${topPlane}", newId())), "rooted" : evaluateQueryCount(context, qCompressed(1, "${topPlane}", id)) };`,
]);

batch.add('studio', "the UI-drawn tab built: its sketch's bodies by type, each with its edges and its vertices' points", [
  `return sketchBodies(Ui::build({}), makeId("FeUNWBXWR3P4AUC_0"));`,
]);
batch.add('namespace', "std's abs, imported into a namespace, used as a value", [
  `return { "abs" : mapArray([1, -2], St::abs) };`,
]);
batch.add('compressed', "a 20 × 10 mm rectangle split by a line up x = 8: the regions beside the line's imprint on either side, by hand-encoded compressed queries", [
  `var sketch = newSketch(context, id + "split", { "sketchPlane" : qCreatedBy(makeId("Top"), EntityType.FACE) });`,
  `skLineSegment(sketch, "l1", { "start" : vector(0, 0) * millimeter, "end" : vector(20, 0) * millimeter });`,
  `skLineSegment(sketch, "l2", { "start" : vector(20, 0) * millimeter, "end" : vector(20, 10) * millimeter });`,
  `skLineSegment(sketch, "l3", { "start" : vector(20, 10) * millimeter, "end" : vector(0, 10) * millimeter });`,
  `skLineSegment(sketch, "l4", { "start" : vector(0, 10) * millimeter, "end" : vector(0, 0) * millimeter });`,
  `skLineSegment(sketch, "l5", { "start" : vector(8, 0) * millimeter, "end" : vector(8, 10) * millimeter });`,
  `skSolve(sketch);`,
  `const areas = function(q) { return mapArray(evaluateQuery(context, q), function(f) { return roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 6); }); };`,
  `return { "plus" : areas(qCompressed(1, "${region('l5', 1)}", id)), "minus" : areas(qCompressed(1, "${region('l5', -1)}", id)), "edgePlus" : areas(qCompressed(1, "${region('l2', 1)}", id)), "edgeMinus" : areas(qCompressed(1, "${region('l2', -1)}", id)) };`,
]);

batch.write('tools/fixtures/gen-batch-103.mjs');
