// Generates fixture batch 091: what sketch text makes (`skText`, not yet built by the engine): "I", "O" and "Ab" in
// Open Sans in a 40 × 10 box, "I" in a box twice as tall, "I" in Arimo; each sketch's edges (their curves' types, B-spline
// data or ends) and its regions' areas and boxes, and the regions extruded 2 mm.
//
// Usage: node tools/fixtures/gen-batch-091.mjs
//
// Writes fixtures/batch-091/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm, regions, solve, vec2, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each edge of some bodies: its curve's type and, for a B-spline, its degree, knots and poles; else its ends.
function curveList(context is Context, edges is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, edges))
    {
        const c = evCurveDefinition(context, { "edge" : edge });
        const ends = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 1], "arcLengthParameterization" : false });
        var entry = { "ends" : mapArray(ends, function(l) { return round6(l.origin); }) };
        if (c is BSplineCurve)
        {
            entry.degree = c.degree;
            entry.rational = c.isRational;
            entry.knots = mapArray(c.knots, function(k) { return roundToPrecision(k, 6); });
            entry.poles = mapArray(c.controlPoints, function(p) { return round6(p); });
        }
        else
        {
            entry["type"] = c is Line ? "line" : (c is Circle ? "circle" : toString(c.curveType));
        }
        out = append(out, entry);
    }
    return out;
}

// Each face of a sketch's regions: its area and box.
function regionList(context is Context, faces is Query) returns array
{
    return mapArray(evaluateQuery(context, faces), function(f)
        {
            const b = evBox3d(context, { "topology" : f, "tight" : true });
            return { "area" : roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 6), "min" : round6(b.minCorner), "max" : round6(b.maxCorner) };
        });
}
`;

const batch = new Batch({ number: 91, title: 'sketch text: what Onshape makes of it', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const text = (s, string, font, [w, h]) => [
  `const ${s} = newSketchOnPlane(context, id + "${s}", { "sketchPlane" : ${PLANES.xy} });`,
  `skText(${s}, "text1", { "text" : "${string}", "fontName" : "${font}", "firstCorner" : ${vec2(0, 0)}, "secondCorner" : ${vec2(w, h)} });`,
  solve(s),
];
const probe = (s) => [
  call('extrude', 'base', { entities: regions(s), endBound: 'BoundingType.BLIND', depth: mm(2), oppositeDirection: 'false' }),
  returns({
    edges: `curveList(context, qOwnedByBody(qCreatedBy(id + "${s}", EntityType.BODY), EntityType.EDGE))`,
    regions: `regionList(context, ${regions(s)})`,
  }),
];
batch.add('sketch', '"I" in Open Sans Regular in a 40 × 10 box', [text('s1', 'I', 'OpenSans-Regular.ttf', [40, 10]), probe('s1')]);
batch.add('sketch', '"I" in Open Sans Regular in a 40 × 20 box', [text('s1', 'I', 'OpenSans-Regular.ttf', [40, 20]), probe('s1')]);
batch.add('sketch', '"O" in Open Sans Regular in a 40 × 10 box', [text('s1', 'O', 'OpenSans-Regular.ttf', [40, 10]), probe('s1')]);
batch.add('sketch', '"Ab" in Open Sans Regular in a 40 × 10 box', [text('s1', 'Ab', 'OpenSans-Regular.ttf', [40, 10]), probe('s1')]);
batch.add('sketch', '"I" in Arimo Regular in a 40 × 10 box', [text('s1', 'I', 'Arimo-Regular.ttf', [40, 10]), probe('s1')]);

batch.write('tools/fixtures/gen-batch-091.mjs');
