// Generates fixture batch 109: a sphere's principal directions (batch 099: always the normal crossed with a world
// axis, which axis by a rule not found) at the same 48 world normals on a sphere as made, turned 90° about z, and
// turned 37° about (1, 2, 3): whether the choice follows the world's axes or the sphere's own parameters.
//
// Usage: node tools/fixtures/gen-batch-109.mjs
//
// Writes fixtures/batch-109/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm, vec3 } from './lib.mjs';

// 48 normals spread over the sphere (a Fibonacci lattice), rounded so the generator's text is stable.
const normals = [];
const N = 48;
for (let i = 0; i < N; i++) {
  const z = 1 - (2 * i + 1) / N;
  const r = Math.sqrt(1 - z * z);
  const phi = i * Math.PI * (3 - Math.sqrt(5));
  normals.push([r * Math.cos(phi), r * Math.sin(phi), z].map((x) => Math.round(x * 1e6) / 1e6));
}

const helpers = `
// The least curvature's direction at each of \`normals\` on a sphere of radius 10 about the origin, its face \`face\`.
function leastDirections(context is Context, face is Query, normals is array) returns array
{
    return mapArray(normals, function(n) {
            const point = normalize(vector(n[0], n[1], n[2])) * 10 * millimeter;
            const parameter = evDistance(context, { "side0" : face, "side1" : point }).sides[0].parameter;
            const d = evFaceCurvature(context, { "face" : face, "parameter" : parameter }).minDirection;
            return [roundToPrecision(d[0], 4), roundToPrecision(d[1], 4), roundToPrecision(d[2], 4)];
        });
}
`;

const batch = new Batch({ number: 109, title: "a sphere's principal directions at the same normals, as made and turned", helpers });

const list = `[${normals.map((n) => `[${n.join(', ')}]`).join(', ')}]`;
const sphere = call('fSphere', 'ball', { radius: mm(10) });
const face = 'qOwnedByBody(qCreatedBy(id + "ball", EntityType.BODY), EntityType.FACE)';
const turned = (axis, degrees) => `opTransform(context, id + "turn", { "bodies" : qCreatedBy(id + "ball", EntityType.BODY), "transform" : rotationAround(line(${vec3(0, 0, 0)}, normalize(vector(${axis.join(', ')}))), ${degrees} * degree) });`;

batch.add('sphere', 'a sphere of radius 10 at the origin, as made: least directions at 48 normals', [
  sphere, `return { "least" : leastDirections(context, ${face}, ${list}) };`,
]);
batch.add('sphere', 'the sphere turned 90° about z: least directions at the same world normals', [
  sphere, turned([0, 0, 1], 90), `return { "least" : leastDirections(context, ${face}, ${list}) };`,
]);
batch.add('sphere', 'the sphere turned 37° about (1, 2, 3): least directions at the same world normals', [
  sphere, turned([1, 2, 3], 37), `return { "least" : leastDirections(context, ${face}, ${list}) };`,
]);

batch.write('tools/fixtures/gen-batch-109.mjs');
