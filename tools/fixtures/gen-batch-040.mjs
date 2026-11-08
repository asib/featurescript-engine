// Generates fixture batch 040: ruled surfaces normal to a curved face along its edge. A rounded block's top arc
// ruled normal to the round, without overrides and with a width override at one end (smooth and linear). Each case
// returns its sheet's faces' surfaces.
//
// Usage: node tools/fixtures/gen-batch-040.mjs
//
// Writes fixtures/batch-040/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, deg, edgeAt, faceAt, mm, solidsOf, vertexAt } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each face of a case's sheets: its area, its surface's type, and for a B-spline its degrees, knots and poles.
function sheetProbe(context is Context, sheets is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(sheets, EntityType.FACE)))
    {
        const s = evSurfaceDefinition(context, { "face" : face });
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6), "type" : s.surfaceType };
        if (s is BSplineSurface)
        {
            var rows = [];
            for (var row in s.controlPoints)
                rows = append(rows, mapArray(row, round6));
            entry.bspline = { "uDegree" : s.uDegree, "vDegree" : s.vDegree, "rational" : s.isRational,
                              "uKnots" : mapArray(s.uKnots, function(k) { return roundToPrecision(k, 9); }),
                              "vKnots" : mapArray(s.vKnots, function(k) { return roundToPrecision(k, 9); }), "poles" : rows };
            if (s.isRational)
                entry.bspline.weights = s.weights;
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 40, title: 'ruled surfaces with vertex overrides on curved edges', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const sheets = `qBodyType(qCreatedBy(id + "ruled1", EntityType.BODY), BodyType.SHEET)`;
const probe = returns({ faces: `sheetProbe(context, ${sheets})` });

// A 30 × 20 × 10 block with its upright edge at (30, 0) rounded 10 mm: the top edge of the round is a quarter arc
// about (20, 10, 10) from (20, 0, 10) to (30, 10, 10), the round itself a quarter cylinder.
const rounded = [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fillet', 'round1', { entities: edgeAt(base, [30, 0, 5]), radius: mm(10) }),
];
const arcEdge = edgeAt(base, [20 + 10 * Math.SQRT1_2, 10 - 10 * Math.SQRT1_2, 10]);
const roundFace = faceAt(base, [20 + 10 * Math.SQRT1_2, 10 - 10 * Math.SQRT1_2, 5]);
const override = (vertex, fields) =>
  `[{ "vertex" : ${vertex}, "angleOverride" : ${deg(0)}, "oppositeAngleOverride" : false, "distanceOverride" : ${mm(10)}, "oppositeDirectionOverride" : false, ${fields} }]`;
const ruled = (fields) =>
  call('ruledSurface', 'ruled1', {
    edges: arcEdge, ruledType: 'RuledSurfaceInterfaceType.NORMAL', distance: mm(5), referenceFaces: roundFace, ...fields,
  });

batch.add('ruled-surface', 'normal to a round from its top arc, 5 mm wide but 10 mm at one end', [
  rounded, ruled({ vertexOverrides: override(vertexAt(base, [30, 10, 10]), '"overrideType" : VertexOverrideType.NORMAL') }), probe,
]);
batch.add('ruled-surface', 'the same, blending linearly', [
  rounded,
  ruled({ vertexOverrides: override(vertexAt(base, [30, 10, 10]), '"overrideType" : VertexOverrideType.NORMAL'), useCubicInterpolation: 'false' }),
  probe,
]);
batch.add('ruled-surface', 'normal to a round from its top arc, without overrides', [rounded, ruled({}), probe]);
// (Onshape fails an angle override, and an override along a vector, on a curved edge with "Execution error".)

batch.write('tools/fixtures/gen-batch-040.mjs');
