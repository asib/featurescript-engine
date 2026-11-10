// Generates fixture batch 089: why Onshape refused batch 084 c005's revolve about the profile's own sketch edge
// (batch 065 c005's was accepted): a trapezoid, a quarter disc and a rectangle revolved about their own edges, found
// by point and by sketch entity; and why it refused batch 074 c006's circles lofted along a quarter circle: with two
// sections, with the path drawn the other way, along a longer arc, and with the profiles a little in from its ends.
//
// Usage: node tools/fixtures/gen-batch-089.mjs
//
// Writes fixtures/batch-089/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

const helpers = `
// Each face of a body: its area and its surface's type.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType;
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 89, title: 'revolves about a profile\'s own edge, circles lofted along a quarter circle', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const probe = returns({ faces: `faceProbe(context, ${base})`, axis: `size(evaluateQuery(context, axisQuery))` });
const byPoint = (s, z) => `const axisQuery = qContainsPoint(qCreatedBy(id + "${s}", EntityType.EDGE), ${vec3(0, 0, z)});`;
const byEntity = (s, name) => `const axisQuery = sketchEntityQuery(id + "${s}", EntityType.EDGE, "${name}");`;
const revolve = (s, fields = {}) => call('revolve', 'base', { entities: regions(s), axis: 'axisQuery', fullRevolve: 'true', ...fields });
const trapezoid = [sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [6, 20]), line('s1', 'l3', [6, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1')];
const quarter = [sketch('s1', PLANES.xz), arc('s1', 'a1', [10, 0], [7.0710678, 7.0710678], [0, 10]), line('s1', 'l1', [0, 10], [0, 0]), line('s1', 'l2', [0, 0], [10, 0]), solve('s1')];

// ----- revolves about the profile's own edge -----

batch.add('revolve', 'batch 084 c005\'s trapezoid revolved about its own edge on z, the edge found by a point on it', [trapezoid, byPoint('s1', 10), revolve('s1'), probe]);
batch.add('revolve', 'the same, the edge found as its sketch entity', [trapezoid, byEntity('s1', 'l4'), revolve('s1'), probe]);
batch.add('revolve', 'batch 065 c005\'s quarter disc revolved about its own edge, found by a point on it', [quarter, byPoint('s1', 5), revolve('s1'), probe]);
batch.add('revolve', 'a 10 × 20 rectangle revolved about its own edge on z, found by a point on it', [
  sketch('s1', PLANES.xz), rect('s1', 'r1', [0, 0], [10, 20]), solve('s1'), byPoint('s1', 10), revolve('s1'), probe,
]);
batch.add('revolve', 'the trapezoid as two regions (split by a line across it at z = 10), revolved about its edge on z found by a point', [
  trapezoid.slice(0, -1), line('s1', 'l5', [0, 10], [8, 10]), solve('s1'), byPoint('s1', 5), revolve('s1'), probe,
]);

// ----- circles lofted along a quarter circle -----

const loftProbe = returns({ faces: `faceProbe(context, ${solidsOf('loft1')})` });
const profiles = (names) => `[${names.map((s) => `{ "sheetProfileEntities" : ${regions(s)} }`).join(', ')}]`;
const circles = [
  sketch('s1', planeAt([20, 0, 0], [0, 0, 1], [1, 0, 0])), circle('s1', 'c1', [0, 0], 3), solve('s1'),
  sketch('s2', planeAt([0, 0, 20], [1, 0, 0], [0, 1, 0])), circle('s2', 'c1', [0, 0], 3), solve('s2'),
];
const path = (from, mid, to) => [sketch('path', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('path', 'a1', from, mid, to), solve('path')];
const loft = (fields) =>
  call('loft', 'loft1', { sheetProfilesArray: profiles(['s1', 's2']), addSections: 'true', spine: `qCreatedBy(id + "path", EntityType.EDGE)`, ...fields });
const r = 20 * Math.SQRT1_2;
batch.add('loft', 'batch 074 c006\'s circles lofted along the quarter circle with 2 sections', [circles, path([20, 0], [r, r], [0, 20]), loft({ sectionCount: '2' }), loftProbe]);
batch.add('loft', 'the same with 5 sections, the quarter circle drawn from (0, 20) to (20, 0)', [circles, path([0, 20], [r, r], [20, 0]), loft({ sectionCount: '5' }), loftProbe]);
batch.add('loft', 'the same, along an arc past both circles (from -10° to 100°)', [
  circles, path([20 * Math.cos(-Math.PI / 18), 20 * Math.sin(-Math.PI / 18)], [r, r], [20 * Math.cos(Math.PI * 10 / 18), 20 * Math.sin(Math.PI * 10 / 18)]), loft({ sectionCount: '5' }), loftProbe,
]);
batch.add('loft', 'circles of radius 3 at either end of a straight path 20 long, lofted along it with 5 sections (batch 075 c006 shorter)', [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('s1', 'c1', [0, 0], 3), solve('s1'),
  sketch('s2', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('s2', 'c1', [0, 0], 3), solve('s2'),
  sketch('path', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('path', 'l1', [0, 0], [0, 20]), solve('path'),
  loft({ sectionCount: '5' }), loftProbe,
]);

batch.write('tools/fixtures/gen-batch-089.mjs');
