// Generates fixture batch 036: curves other than lines trimmed and extended. Arcs extended by a distance (curving on
// and straight on) and up to a plane, an arc trimmed by a plane, a fit spline extended both ways, and a line with a
// tangent arc extended at the arc's end; then arcs of other radii, angles and distances, whose B-splines show how
// Onshape makes an arc one and how far a curving extension's blend runs.
//
// Usage: node tools/fixtures/gen-batch-036.mjs
//
// Writes fixtures/batch-036/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, createdBy, line, mm, planeBody, sketch, solidsOf, solve, vertexAt, PLANES } from './lib.mjs';

const helpers = `
// Each edge of a case's wires: its curve type and length, and a B-spline's degree, knots and poles.
function wireProbe(context is Context, wires is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(wires, EntityType.EDGE)))
    {
        const c = evCurveDefinition(context, { "edge" : edge });
        var entry = { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) };
        if (c is BSplineCurve)
        {
            var points = [];
            for (var p in c.controlPoints)
                points = append(points, [roundToPrecision(p[0] / millimeter, 6), roundToPrecision(p[1] / millimeter, 6), roundToPrecision(p[2] / millimeter, 6)]);
            var knots = [];
            for (var k in c.knots)
                knots = append(knots, roundToPrecision(k, 9));
            entry.bspline = { "degree" : c.degree, "rational" : c.isRational, "points" : points, "knots" : knots };
        }
        else if (c is Circle)
            entry.circle = { "radius" : roundToPrecision(c.radius / millimeter, 6) };
        else if (c is Line)
            entry.line = true;
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 36, title: 'curves other than lines trimmed and extended: arcs, fit splines, chains', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const curve1 = createdBy('curve1');
const probe = returns({ edges: `wireProbe(context, ${curve1})` });

// A quarter circle of radius 20 about the origin, from (20, 0) to (0, 20), as a curve.
const quarter = [
  sketch('s1', PLANES.xy), arc('s1', 'arc1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('s1'),
  call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
];
const extend = (fields) =>
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, [0, 20, 0]), ...fields,
  });

batch.add('trim-curve', 'a quarter circle extended 10 mm at its end, curving on', [
  quarter, extend({ endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: 'CurveExtensionShape.CURVATURE' }), probe,
]);
batch.add('trim-curve', 'a quarter circle extended 10 mm at its end, straight on', [
  quarter, extend({ endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: 'CurveExtensionShape.TANGENT' }), probe,
]);
batch.add('trim-curve', 'a quarter circle extended up to the plane x = -10, curving on', [
  quarter, planeBody('limit', [-10, 0, 0], [1, 0, 0], [0, 1, 0]),
  extend({ endCondition: 'CurveExtensionEndCondition.UP_TO_ENTITY', extendTo: createdBy('limit', 'FACE'), extensionShape: 'CurveExtensionShape.CURVATURE' }),
  probe,
]);
batch.add('trim-curve', 'a quarter circle trimmed by the plane x = 10, keeping its end at (0, 20)', [
  quarter, planeBody('limit', [10, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.TRIM', wires: curve1, trimTo: createdBy('limit', 'FACE'), helpPoint: vertexAt(curve1, [0, 20, 0]),
    flipHeuristics: 'false',
  }),
  probe,
]);

// ----- a fit spline -----

const base = solidsOf('base');
const spline = [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fitSpline', 'curve1', { vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 10])}, ${vertexAt(base, [30, 20, 0])}])` }),
];
const extendSpline = (shape) =>
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, [30, 20, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: `CurveExtensionShape.${shape}`,
  });
batch.add('trim-curve', 'a fit spline through three corners of a block extended 10 mm at its end, straight on', [spline, extendSpline('TANGENT'), probe]);
batch.add('trim-curve', 'a fit spline through three corners of a block extended 10 mm at its end, curving on', [spline, extendSpline('CURVATURE'), probe]);

// ----- a chain -----

batch.add('trim-curve', 'a line and a tangent arc extended 10 mm at the arc\'s end, curving on', [
  sketch('s1', PLANES.xy), line('s1', 'line1', [0, 0], [20, 0]), arc('s1', 'arc1', [20, 0], [27.071067811865476, 2.9289321881345245], [30, 10]),
  solve('s1'), call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, [30, 10, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: 'CurveExtensionShape.CURVATURE',
  }),
  probe,
]);

// ----- how Onshape makes an arc a B-spline to extend it -----

// An arc about the origin from angle 0 to `angle` degrees, as a curve, extended `distance` at its end.
const arcExtended = (radius, angle, distance, shape) => {
  const at = (deg) => [radius * Math.cos((deg * Math.PI) / 180), radius * Math.sin((deg * Math.PI) / 180)];
  const end = at(angle);
  return [
    sketch('s1', PLANES.xy), arc('s1', 'arc1', at(0), at(angle / 2), end), solve('s1'),
    call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
    call('trimCurve', 'trim1', {
      moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, [end[0], end[1], 0]),
      endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(distance), extensionShape: `CurveExtensionShape.${shape}`,
    }),
    probe,
  ];
};
for (const [radius, angle] of [[5, 90], [40, 90], [100, 90], [20, 45], [20, 180], [20, 30], [7, 120]]) {
  batch.add('trim-curve', `an arc of radius ${radius} through ${angle} degrees extended 10 mm, straight on`, [arcExtended(radius, angle, 10, 'TANGENT')]);
}
for (const [radius, distance] of [[20, 5], [20, 20], [10, 5], [40, 10]]) {
  batch.add('trim-curve', `a quarter circle of radius ${radius} extended ${distance} mm, curving on`, [arcExtended(radius, 90, distance, 'CURVATURE')]);
}

batch.write('tools/fixtures/gen-batch-036.mjs');
