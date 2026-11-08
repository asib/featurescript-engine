// Generates fixture batch 047: the coordinate systems Onshape gives fillets' cylinders (`evSurfaceDefinition`),
// which anchor wraps, unwraps and decals on them. A block's upright edges at each corner, its top edges along x and
// y, a bottom edge, and a concave edge (an L block's inside corner), each rounded and probed.
//
// Usage: node tools/fixtures/gen-batch-047.mjs
//
// Writes fixtures/batch-047/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm, solidsOf, vec3 } from './lib.mjs';

const helpers = `
function frameNumbers(cs) returns array
{
    return [[roundToPrecision(cs.origin[0] / millimeter, 6), roundToPrecision(cs.origin[1] / millimeter, 6), roundToPrecision(cs.origin[2] / millimeter, 6)],
            [roundToPrecision(cs.xAxis[0], 9), roundToPrecision(cs.xAxis[1], 9), roundToPrecision(cs.xAxis[2], 9)],
            [roundToPrecision(cs.zAxis[0], 9), roundToPrecision(cs.zAxis[1], 9), roundToPrecision(cs.zAxis[2], 9)]];
}

// Every cylindrical face of a body: its coordinate system as numbers, and a point inside it.
function cylinderFrames(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qGeometry(qOwnedByBody(body, EntityType.FACE), GeometryType.CYLINDER)))
    {
        const p = evFaceTangentPlane(context, { "face" : face, "parameter" : vector(0.5, 0.5) }).origin;
        out = append(out, { "frame" : frameNumbers(evSurfaceDefinition(context, { "face" : face }).coordSystem),
                            "at" : [roundToPrecision(p[0] / millimeter, 4), roundToPrecision(p[1] / millimeter, 4), roundToPrecision(p[2] / millimeter, 4)] });
    }
    return out;
}
`;

const batch = new Batch({ number: 47, title: 'the coordinate systems of fillets\' cylinders', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const block = `fCuboid(context, id + "base", { "corner1" : ${vec3(0, 0, 0)}, "corner2" : ${vec3(30, 20, 10)} });`;
const edgeAt = (p) => `qContainsPoint(qOwnedByBody(${base}, EntityType.EDGE), ${vec3(...p)})`;
const round = (edges) => call('fillet', 'round1', { entities: `qUnion([${edges.map(edgeAt).join(', ')}])`, radius: mm(4) });
const probe = returns({ cylinders: `cylinderFrames(context, ${base})` });

for (const [label, p] of [['(0, 0)', [0, 0, 5]], ['(30, 0)', [30, 0, 5]], ['(30, 20)', [30, 20, 5]], ['(0, 20)', [0, 20, 5]]]) {
  batch.add('fillet-frame', `a block's upright edge at ${label} rounded 4 mm`, [block, round([p]), probe]);
}
batch.add('fillet-frame', 'a block\'s top edge along x at y = 0 rounded 4 mm', [block, round([[15, 0, 10]]), probe]);
batch.add('fillet-frame', 'a block\'s top edge along y at x = 30 rounded 4 mm', [block, round([[30, 10, 10]]), probe]);
batch.add('fillet-frame', 'a block\'s bottom edge along x at y = 20 rounded 4 mm', [block, round([[15, 20, 0]]), probe]);
// An L block: a 30 × 20 × 10 block with a 10 × 10 × 10 block on its top at x 0..10; the inside edge along y at
// (10, y, 10) is concave.
batch.add('fillet-frame', 'an L block\'s concave inside edge rounded 4 mm', [
  block, `fCuboid(context, id + "riser", { "corner1" : ${vec3(0, 0, 10)}, "corner2" : ${vec3(10, 20, 20)} });`,
  call('booleanBodies', 'join', { tools: `qUnion([${base}, ${solidsOf('riser')}])`, operationType: 'BooleanOperationType.UNION' }),
  round([[10, 10, 10]]), probe,
]);

batch.write('tools/fixtures/gen-batch-047.mjs');
