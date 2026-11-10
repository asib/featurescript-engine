// Generates fixture batch 097: more sketch text, to test what batch 091 found (FreeType's unhinted outlines at 100
// pixels to the em, no kerning, a frame a quarter of a space wider than the ink): a phrase with spaces,
// punctuation and digits; "AV" (a kerning pair); "é" (a composite glyph); the box's corners swapped and the box
// away from the origin; construction text; descenders in Arimo.
//
// Usage: node tools/fixtures/gen-batch-097.mjs
//
// Writes fixtures/batch-097/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm, regions, solve, vec2, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6)];
}

// Each edge of some bodies: its curve's type and, for a B-spline, its poles; else its ends.
function curveList(context is Context, edges is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, edges))
    {
        const c = evCurveDefinition(context, { "edge" : edge });
        if (c is BSplineCurve)
        {
            out = append(out, { "degree" : c.degree, "poles" : mapArray(c.controlPoints, function(p) { return round6(p); }) });
        }
        else
        {
            const ends = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 1], "arcLengthParameterization" : false });
            out = append(out, { "ends" : mapArray(ends, function(l) { return round6(l.origin); }) });
        }
    }
    return out;
}

// A sketch's text in brief: how many of its wires' edges are lines and B-splines, the construction edges' box, and
// its regions' areas and boxes.
function textSummary(context is Context, sketchId is Id) returns map
{
    const wires = qBodyType(qCreatedBy(sketchId, EntityType.BODY), BodyType.WIRE);
    const edges = qOwnedByBody(wires, EntityType.EDGE);
    const construction = qConstructionFilter(edges, ConstructionObject.YES);
    const frameBox = evBox3d(context, { "topology" : construction, "tight" : true });
    return { "lines" : evaluateQueryCount(context, qGeometry(edges, GeometryType.LINE)),
             "curves" : evaluateQueryCount(context, qSubtraction(edges, qGeometry(edges, GeometryType.LINE))),
             "construction" : evaluateQueryCount(context, construction),
             "frame" : [round6(frameBox.minCorner), round6(frameBox.maxCorner)],
             "regions" : mapArray(evaluateQuery(context, qSketchRegion(sketchId)), function(f)
                 {
                     const b = evBox3d(context, { "topology" : f, "tight" : true });
                     return { "area" : roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 6), "min" : round6(b.minCorner), "max" : round6(b.maxCorner) };
                 }) };
}
`;

const batch = new Batch({ number: 97, title: 'more sketch text', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const text = (string, font, first, second, extra = '') => [
  `const s1 = newSketchOnPlane(context, id + "s1", { "sketchPlane" : ${PLANES.xy} });`,
  `skText(s1, "text1", { "text" : "${string}", "fontName" : "${font}", "firstCorner" : ${vec2(...first)}, "secondCorner" : ${vec2(...second)}${extra} });`,
  solve('s1'),
];
const extrude = call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(1), oppositeDirection: 'false' });
const summary = returns({ text: 'textSummary(context, id + "s1")' });
const full = returns({ edges: 'curveList(context, qOwnedByBody(qBodyType(qCreatedBy(id + "s1", EntityType.BODY), BodyType.WIRE), EntityType.EDGE))', text: 'textSummary(context, id + "s1")' });
const OPEN_SANS = 'OpenSans-Regular.ttf';

batch.add('sketch', '"Hello, World 123" in Open Sans Regular in a 40 × 8 box, extruded 1 mm', [text('Hello, World 123', OPEN_SANS, [0, 0], [40, 8]), extrude, summary]);
batch.add('sketch', '"AV" (a kerning pair) in Open Sans Regular in a 40 × 10 box', [text('AV', OPEN_SANS, [0, 0], [40, 10]), full]);
batch.add('sketch', '"é" (a composite glyph) in Open Sans Regular in a 20 × 10 box', [text('é', OPEN_SANS, [0, 0], [20, 10]), full]);
batch.add('sketch', '"Ab" in Open Sans Regular with the box\'s corners given from (40, 10) to (0, 0)', [text('Ab', OPEN_SANS, [40, 10], [0, 0]), summary]);
batch.add('sketch', '"Ab" in Open Sans Regular in a box from (5, 3) to (45, 11)', [text('Ab', OPEN_SANS, [5, 3], [45, 11]), summary]);
batch.add('sketch', '"T" in Open Sans Regular as construction, in a 20 × 10 box', [text('T', OPEN_SANS, [0, 0], [20, 10], ', "construction" : true'), summary]);
batch.add('sketch', '"gjpqy" in Arimo Regular in a 40 × 10 box, extruded 1 mm', [text('gjpqy', 'Arimo-Regular.ttf', [0, 0], [40, 10]), extrude, summary]);

batch.write('tools/fixtures/gen-batch-097.mjs');
