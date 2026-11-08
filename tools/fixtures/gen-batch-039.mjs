// Generates fixture batch 039: curved sheets other than cylinders extended. A cone's and a sphere's sheets carried
// on round and along them, and sheets extruded from fit splines extended past a spline's end, straight on and keeping
// its curvature, and along the extrusion. Each case returns its sheet's faces' surfaces (a B-spline's degrees, knots
// and poles), to show how Onshape builds the extensions.
//
// Usage: node tools/fixtures/gen-batch-039.mjs
//
// Writes fixtures/batch-039/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, call, edgeAt, edgeOf, line, mm, sketch, solve, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each face of a sheet: its area, its surface's type, and for a B-spline its degrees, knots and poles.
function sheetProbe(context is Context, sheet is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(sheet, EntityType.FACE)))
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
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 39, title: 'curved sheets other than cylinders extended: cones, spheres, extruded splines', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const sheet = `qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SHEET)`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const probe = returns({ faces: `sheetProbe(context, ${sheet})` });
const extend = (edge, distance, curvature = false) =>
  call('extendSurface', 'extend1', {
    entities: edge, endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(distance), oppositeDirection: 'false',
    maintainCurvature: curvature ? 'true' : 'false', tangentPropagation: 'false',
  });

// ----- a cone and a sphere -----

// A quarter of a cone: the line from (10, 0) to (5, 20) in the xz plane swept 90° about z.
const cone = [
  sketch('profile', PLANES.xz), line('profile', 'line1', [10, 0], [5, 20]), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '90 * degree',
  }),
];
batch.add('extend-surface', 'a quarter of a cone extended 5 mm round it, at its edge in the xz plane', [cone, extend(edgeAt(sheet, [7.5, 0, 10]), 5), probe]);
batch.add('extend-surface', 'a quarter of a cone extended 5 mm along it, past its narrow end', [
  cone, extend(edgeAt(sheet, [3.5355339059327378, 3.5355339059327378, 20]), 5), probe,
]);
// A quarter of a sphere's band: the arc of radius 10 from 0° to 60° in the xz plane swept 90° about z.
const sphere = [
  sketch('profile', PLANES.xz), arc('profile', 'arc1', [10, 0], [8.660254037844387, 5], [5, 8.660254037844386]), solve('profile'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '90 * degree',
  }),
];
batch.add('extend-surface', 'a band of a sphere extended 5 mm round it, at its edge in the xz plane', [sphere, extend(edgeAt(sheet, [8.660254037844387, 0, 5]), 5), probe]);
batch.add('extend-surface', 'a band of a sphere extended 5 mm past its top edge', [
  sphere, extend(edgeAt(sheet, [3.5355339059327378, 3.5355339059327378, 8.660254037844386]), 5), probe,
]);

// ----- extruded splines -----

// A sheet extruded 10 mm up from a fit spline through (0, 0), (10, 8), (25, 5), (35, 12) on the xy plane.
const splineSheet = (startDerivative) => [
  sketch('profile', PLANES.xy),
  `skFitSpline(profile, "spline1", { "points" : [vector(0, 0) * millimeter, vector(10, 8) * millimeter, vector(25, 5) * millimeter, vector(35, 12) * millimeter]${startDerivative ? `, "startDerivative" : vector(20, 10) * millimeter, "endDerivative" : vector(40, 0) * millimeter` : ''} });`,
  solve('profile'),
  call('extrude', 'base', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), endBound: 'BoundingType.BLIND', depth: mm(10) }),
];
const endEdge = edgeAt(sheet, [35, 12, 5]);
batch.add('extend-surface', 'a sheet extruded from a fit spline extended 5 mm past the spline\'s end, straight on', [splineSheet(false), extend(endEdge, 5), probe]);
batch.add('extend-surface', 'a sheet extruded from a fit spline extended 5 mm past the spline\'s end, keeping its curvature', [splineSheet(false), extend(endEdge, 5, true), probe]);
batch.add('extend-surface', 'a sheet extruded from a fit spline with end derivatives, arriving along x, extended 5 mm past the spline\'s end, straight on', [splineSheet(true), extend(endEdge, 5), probe]);
batch.add('extend-surface', 'a sheet extruded from a fit spline with end derivatives, arriving along x, extended 5 mm past the spline\'s end, keeping its curvature', [splineSheet(true), extend(endEdge, 5, true), probe]);
batch.add('extend-surface', 'a sheet extruded from a fit spline extended 5 mm up, past its top', [
  splineSheet(false), extend(`qContainsPoint(qOwnedByBody(${sheet}, EntityType.EDGE), vector(10, 8, 10) * millimeter)`, 5), probe,
]);

batch.write('tools/fixtures/gen-batch-039.mjs');
