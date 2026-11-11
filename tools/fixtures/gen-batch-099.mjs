// Generates fixture batch 099: a sphere's principal directions, where every direction is one (batch 087 c008 found
// Onshape's least is one of its parameters' directions, reversed, the one whose curvature comes out smaller in
// floating point): a whole sphere of radius 10 at the origin at 24 parameters, one of radius 7 at (5, -3, 2), and a
// dome revolved from a quarter disc.
//
// Usage: node tools/fixtures/gen-batch-099.mjs
//
// Writes fixtures/batch-099/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, mm, planeAt, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0], 6), roundToPrecision(v[1], 6), roundToPrecision(v[2], 6)];
}

// A face's least and greatest curvatures' directions at some parameters, with the point.
function directions(context is Context, face is Query, parameters is array) returns array
{
    const curvatures = evFaceCurvatures(context, { "face" : face, "parameters" : parameters });
    const planes = evFaceTangentPlanes(context, { "face" : face, "parameters" : parameters });
    var out = [];
    for (var i = 0; i < size(parameters); i += 1)
    {
        out = append(out, { "at" : round6(planes[i].origin / millimeter), "min" : round6(curvatures[i].minDirection), "max" : round6(curvatures[i].maxDirection) });
    }
    return out;
}
`;

const batch = new Batch({ number: 99, title: "a sphere's principal directions", helpers });

const grid = [];
for (const u of [0, 0.1, 0.25, 0.4, 0.5, 0.65, 0.8, 0.9]) for (const v of [0.2, 0.5, 0.75]) grid.push(`vector(${u}, ${v})`);
const probe = (face) => `return { "directions" : directions(context, ${face}, [${grid.join(', ')}]) };`;

batch.add('evaluate', 'a sphere of radius 10 at the origin: its directions at 24 parameters', [
  call('fSphere', 'base', { radius: mm(10) }),
  probe(`qOwnedByBody(${solidsOf('base')}, EntityType.FACE)`),
]);
batch.add('evaluate', 'a sphere of radius 7 at (5, -3, 2): its directions at 24 parameters', [
  `opSphere(context, id + "base", { "center" : ${vec3(5, -3, 2)}, "radius" : ${mm(7)} });`,
  probe(`qOwnedByBody(${solidsOf('base')}, EntityType.FACE)`),
]);
batch.add('evaluate', 'a dome of radius 10 (a quarter disc revolved about z): its spherical face\'s directions', [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('s1', 'l1', [0, 10], [0, 0]), line('s1', 'l2', [0, 0], [10, 0]), arc('s1', 'a1', [10, 0], [7.0710678, 7.0710678], [0, 10]), solve('s1'),
  call('revolve', 'base', { entities: regions('s1'), axis: `sketchEntityQuery(id + "s1", EntityType.EDGE, "l1")`, fullRevolve: 'true' }),
  probe(`qGeometry(qOwnedByBody(${solidsOf('base')}, EntityType.FACE), GeometryType.SPHERE)`),
]);

batch.write('tools/fixtures/gen-batch-099.mjs');
