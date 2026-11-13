// Generates fixture batch 153: fills of non-planar loops: four arcs rising over a rectangle's sides (a pillow), two
// arcs (one up, one down) and two lines (a saddle), a twisted loop of arcs and lines, and three fit splines (a
// triangle bulging up).
//
// Usage: node tools/fixtures/gen-batch-153.mjs
//
// Writes fixtures/batch-153/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, planeAt, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
// Each face's area and its point and normal at the middle of its parameters, rounded to 1e-6.
function fillProbe(context is Context, faces is Query) returns array
{
    var out = [];
    for (var f in evaluateQuery(context, faces))
    {
        const p = evFaceTangentPlane(context, { "face" : f, "parameter" : vector(0.5, 0.5) });
        const r = function(x) { return round(x * 1e6) / 1e6; };
        out = append(out, [r(evArea(context, { "entities" : f }) / millimeter ^ 2),
                [r(p.origin[0] / millimeter), r(p.origin[1] / millimeter), r(p.origin[2] / millimeter)],
                [r(p.normal[0]), r(p.normal[1]), r(p.normal[2])]]);
    }
    return out;
}
`;
const batch = new Batch({ number: 153, title: 'fills of non-planar loops: a pillow, a saddle of arcs and lines, a twisted loop, a triangle of splines', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheets = `qBodyType(qCreatedBy(id + "fill1", EntityType.BODY), BodyType.SHEET)`;
const fill = (names) => call('fill', 'fill1', {
  edges: `[{ "entities" : qUnion([${names.map(wires).join(', ')}]), "continuity" : GeometricContinuity.G0 }]`,
  surfaceOperationType: 'NewSurfaceOperationType.NEW',
});
const probe = `return { "faces" : fillProbe(context, qOwnedByBody(${sheets}, EntityType.FACE)) };`;
// Upright sketch planes along the rectangle's sides: y = 0 and y = 20 (sketch x along x), x = 0 and x = 30 (along y);
// sketch y is up.
const front = (y) => planeAt([0, y, 0], [0, -1, 0], [1, 0, 0]);
const side = (x) => planeAt([x, 0, 0], [1, 0, 0], [0, 1, 0]);

batch.add('fill', 'a pillow: arcs rising 5 over each side of a 30 × 20 rectangle', [
  sketch('e1', front(0)), arc('e1', 'a1', [0, 0], [15, 5], [30, 0]), solve('e1'),
  sketch('e2', side(30)), arc('e2', 'a1', [0, 0], [10, 5], [20, 0]), solve('e2'),
  sketch('e3', front(20)), arc('e3', 'a1', [30, 0], [15, 5], [0, 0]), solve('e3'),
  sketch('e4', side(0)), arc('e4', 'a1', [20, 0], [10, 5], [0, 0]), solve('e4'),
  fill(['e1', 'e2', 'e3', 'e4']), probe,
]);
batch.add('fill', 'a saddle: an arc rising 5 along y = 0, an arc dipping 5 along y = 20, lines along x = 0 and x = 30', [
  sketch('e1', front(0)), arc('e1', 'a1', [0, 0], [15, 5], [30, 0]), solve('e1'),
  sketch('e2', side(30)), line('e2', 'l1', [0, 0], [20, 0]), solve('e2'),
  sketch('e3', front(20)), arc('e3', 'a1', [30, 0], [15, -5], [0, 0]), solve('e3'),
  sketch('e4', side(0)), line('e4', 'l1', [20, 0], [0, 0]), solve('e4'),
  fill(['e1', 'e2', 'e3', 'e4']), probe,
]);
batch.add('fill', 'a twisted loop: corners at heights 0, 6, 0, 6 round a 30 × 20 rectangle, arcs along y = 0 and y = 20, lines along the ends', [
  sketch('e1', front(0)), arc('e1', 'a1', [0, 0], [15, 6], [30, 6]), solve('e1'),
  sketch('e2', side(30)), line('e2', 'l1', [0, 6], [20, 0]), solve('e2'),
  sketch('e3', front(20)), arc('e3', 'a1', [30, 0], [15, 0], [0, 6]), solve('e3'),
  sketch('e4', side(0)), line('e4', 'l1', [20, 6], [0, 0]), solve('e4'),
  fill(['e1', 'e2', 'e3', 'e4']), probe,
]);
batch.add('fill', 'a triangle of fit splines through (0, 0, 0), (30, 0, 0), (15, 25, 0), each bulging 4 up at its middle', [
  call('opFitSpline', 'e1', { points: `[${vec3(0, 0, 0)}, ${vec3(15, 0, 4)}, ${vec3(30, 0, 0)}]` }),
  call('opFitSpline', 'e2', { points: `[${vec3(30, 0, 0)}, ${vec3(22.5, 12.5, 4)}, ${vec3(15, 25, 0)}]` }),
  call('opFitSpline', 'e3', { points: `[${vec3(15, 25, 0)}, ${vec3(7.5, 12.5, 4)}, ${vec3(0, 0, 0)}]` }),
  fill(['e1', 'e2', 'e3']), probe,
]);

batch.write('tools/fixtures/gen-batch-153.mjs');
