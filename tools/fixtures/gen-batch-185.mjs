// Generates fixture batch 185: sketch text in the other fonts std's skText offers: "Ag" in Open Sans, Arimo, Tinos, Noto
// Sans and Noto Serif, regular, bold, italic and bold italic, in Allerta Stencil, Droid Sans Mono and Roboto Slab, and in
// a font Onshape doesn't have; each sketch's regions' areas and boxes.
//
// Usage: node tools/fixtures/gen-batch-185.mjs
//
// Writes fixtures/batch-185/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, solve, vec2, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 185, title: 'sketch text in the other fonts', helpers });
const text = (font) => [
  `const s = newSketchOnPlane(context, id + "s", { "sketchPlane" : ${PLANES.xy} });`,
  `skText(s, "text1", { "text" : "Ag", "fontName" : "${font}", "firstCorner" : ${vec2(0, 0)}, "secondCorner" : ${vec2(40, 10)} });`,
  solve('s'),
  `return { "regions" : regionList(context, qSketchRegion(id + "s")) };`,
];
batch.add('sketch', '"Ag" in OpenSans-Bold.ttf in a 40 × 10 box', text('OpenSans-Bold.ttf'));
batch.add('sketch', '"Ag" in OpenSans-Italic.ttf in a 40 × 10 box', text('OpenSans-Italic.ttf'));
batch.add('sketch', '"Ag" in OpenSans-BoldItalic.ttf in a 40 × 10 box', text('OpenSans-BoldItalic.ttf'));
batch.add('sketch', '"Ag" in Arimo-Bold.ttf in a 40 × 10 box', text('Arimo-Bold.ttf'));
batch.add('sketch', '"Ag" in Arimo-Italic.ttf in a 40 × 10 box', text('Arimo-Italic.ttf'));
batch.add('sketch', '"Ag" in Arimo-BoldItalic.ttf in a 40 × 10 box', text('Arimo-BoldItalic.ttf'));
batch.add('sketch', '"Ag" in Tinos-Regular.ttf in a 40 × 10 box', text('Tinos-Regular.ttf'));
batch.add('sketch', '"Ag" in Tinos-Bold.ttf in a 40 × 10 box', text('Tinos-Bold.ttf'));
batch.add('sketch', '"Ag" in Tinos-Italic.ttf in a 40 × 10 box', text('Tinos-Italic.ttf'));
batch.add('sketch', '"Ag" in Tinos-BoldItalic.ttf in a 40 × 10 box', text('Tinos-BoldItalic.ttf'));
batch.add('sketch', '"Ag" in NotoSans-Regular.ttf in a 40 × 10 box', text('NotoSans-Regular.ttf'));
batch.add('sketch', '"Ag" in NotoSans-Bold.ttf in a 40 × 10 box', text('NotoSans-Bold.ttf'));
batch.add('sketch', '"Ag" in NotoSans-Italic.ttf in a 40 × 10 box', text('NotoSans-Italic.ttf'));
batch.add('sketch', '"Ag" in NotoSans-BoldItalic.ttf in a 40 × 10 box', text('NotoSans-BoldItalic.ttf'));
batch.add('sketch', '"Ag" in NotoSerif-Regular.ttf in a 40 × 10 box', text('NotoSerif-Regular.ttf'));
batch.add('sketch', '"Ag" in NotoSerif-Bold.ttf in a 40 × 10 box', text('NotoSerif-Bold.ttf'));
batch.add('sketch', '"Ag" in NotoSerif-Italic.ttf in a 40 × 10 box', text('NotoSerif-Italic.ttf'));
batch.add('sketch', '"Ag" in NotoSerif-BoldItalic.ttf in a 40 × 10 box', text('NotoSerif-BoldItalic.ttf'));
batch.add('sketch', '"Ag" in AllertaStencil-Regular.ttf in a 40 × 10 box', text('AllertaStencil-Regular.ttf'));
batch.add('sketch', '"Ag" in DroidSansMono.ttf in a 40 × 10 box', text('DroidSansMono.ttf'));
batch.add('sketch', '"Ag" in RobotoSlab-Regular.ttf in a 40 × 10 box', text('RobotoSlab-Regular.ttf'));
batch.add('sketch', '"Ag" in Nonesuch-Regular.ttf in a 40 × 10 box', text('Nonesuch-Regular.ttf'));

batch.write('tools/fixtures/gen-batch-185.mjs');
