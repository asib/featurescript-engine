// Generates fixture batch 170: probes of how Onshape lofts a rectangle to a circle (batch 169 c001 is 0.23% of the
// volume off the engine's, whose circle is split at the corners' angles as a square's matches): the loft's faces with
// their poles, for a 20 × 10 rectangle, a 30 × 10 one and a 20 × 20 square, each to a circle of radius 6 20 up.
//
// Usage: node tools/fixtures/gen-batch-170.mjs
//
// Writes fixtures/batch-170/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, planeAt, rect, regions, sketch, solve } from './lib.mjs';

const helpers = `
function numbersProbe(numbers) returns array
{
    var out = [];
    for (var i = 0; i < size(numbers); i += 1)
        out = append(out, roundToPrecision(numbers[i], 9));
    return out;
}

// Each face's area, its point and normal at the middle of its parameters, and, for a B-spline surface, its degrees,
// knots, weights and poles (rounded to 1e-6 mm).
function surfaceProbe(context is Context, faces is Query) returns array
{
    var out = [];
    const r = function(x) { return roundToPrecision(x, 6); };
    for (var f in evaluateQuery(context, faces))
    {
        const p = evFaceTangentPlane(context, { "face" : f, "parameter" : vector(0.5, 0.5) });
        var entry = { "area" : r(evArea(context, { "entities" : f }) / millimeter ^ 2),
                "middle" : [r(p.origin[0] / millimeter), r(p.origin[1] / millimeter), r(p.origin[2] / millimeter)],
                "normal" : [r(p.normal[0]), r(p.normal[1]), r(p.normal[2])] };
        const s = evSurfaceDefinition(context, { "face" : f });
        entry["type"] = s.surfaceType;
        if (s is BSplineSurface)
        {
            entry.degrees = [s.uDegree, s.vDegree];
            entry.uKnots = numbersProbe(s.uKnots);
            entry.vKnots = numbersProbe(s.vKnots);
            if (s.isRational)
                entry.weights = s.weights;
            var rows = [];
            for (var i = 0; i < size(s.controlPoints); i += 1)
            {
                var points = [];
                for (var j = 0; j < size(s.controlPoints[i]); j += 1)
                {
                    const q = s.controlPoints[i][j];
                    points = append(points, [r(q[0] / millimeter), r(q[1] / millimeter), r(q[2] / millimeter)]);
                }
                rows = append(rows, points);
            }
            entry.poles = rows;
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 170, title: 'probes: the poles of lofts from rectangles to circles', helpers });
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loft = (w, h) => [
  sketch('a', at(0)), rect('a', 'r1', [-w / 2, -h / 2], [w / 2, h / 2]), solve('a'),
  sketch('b', at(20)), circle('b', 'c1', [0, 0], 6), solve('b'),
  call('loft', 'loft1', { sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('a')} }, { "sheetProfileEntities" : ${regions('b')} }]`,
    bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW', startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT' }),
  `return { "faces" : surfaceProbe(context, qCreatedBy(id + "loft1", EntityType.FACE)) };`,
];
batch.add('loft', 'a 20 × 10 rectangle lofted to a circle (radius 6) 20 up: the faces\' poles', loft(20, 10));
batch.add('loft', 'a 30 × 10 rectangle lofted to the circle: the faces\' poles', loft(30, 10));
batch.add('loft', 'a 20 × 20 square lofted to the circle: the faces\' poles', loft(20, 20));

batch.write('tools/fixtures/gen-batch-170.mjs');
