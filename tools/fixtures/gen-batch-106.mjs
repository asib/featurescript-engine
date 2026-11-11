// Generates fixture batch 106: std's query functions whose query types no batch had evaluated: unique vertices,
// a tolerance filter, entities touching an infinite line, entities within a radius of a point, a coedge (which std
// says can't be evaluated like other queries), a mesh's source (no meshes here) and a filter by axis.
//
// Usage: node tools/fixtures/gen-batch-106.mjs
//
// Writes fixtures/batch-106/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, cylinder, mm, vec3 } from './lib.mjs';

const helpers = `
function mm(p is Vector) returns array
{
    return [roundToPrecision(p[0] / millimeter, 6), roundToPrecision(p[1] / millimeter, 6), roundToPrecision(p[2] / millimeter, 6)];
}

// How many bodies, faces, edges and vertices a query finds.
function counts(context is Context, q is Query) returns map
{
    return { "bodies" : size(evaluateQuery(context, qEntityFilter(q, EntityType.BODY))), "faces" : size(evaluateQuery(context, qEntityFilter(q, EntityType.FACE))),
             "edges" : size(evaluateQuery(context, qEntityFilter(q, EntityType.EDGE))), "vertices" : size(evaluateQuery(context, qEntityFilter(q, EntityType.VERTEX))) };
}

// Everything a case made, of every kind.
function everythingOf(id is Id) returns Query
{
    const bodies = qCreatedBy(id, EntityType.BODY);
    return qUnion([bodies, qOwnedByBody(bodies, EntityType.FACE), qOwnedByBody(bodies, EntityType.EDGE), qOwnedByBody(bodies, EntityType.VERTEX)]);
}

// The points of the vertices a query finds, sorted.
function vertexPoints(context is Context, q is Query) returns array
{
    return sort(mapArray(evaluateQuery(context, q), function(v) { return mm(evVertexPoint(context, { "vertex" : v })); }),
        function(a, b) { return a[0] != b[0] ? a[0] - b[0] : (a[1] != b[1] ? a[1] - b[1] : a[2] - b[2]); });
}
`;

const batch = new Batch({ number: 106, title: "std's query functions no batch had evaluated", helpers });

const caught = (statement) => `try { ${statement} } catch (error) { return { "error" : error }; }`;
const blocks = [box('a', [0, 0, 0], [10, 10, 10]), box('b', [10, 0, 0], [20, 10, 10])];
const verticesOf = (names) => `qOwnedByBody(qUnion([${names.map((n) => `qCreatedBy(id + "${n}", EntityType.BODY)`).join(', ')}]), EntityType.VERTEX)`;

batch.add('unique', "two blocks side by side, sharing a face's corners: their vertices, and the unique ones (which block keeps the shared ones)", [
  blocks,
  `const all = ${verticesOf(['a', 'b'])};`,
  `const unique = qUniqueVertices(all);`,
  `return { "all" : size(evaluateQuery(context, all)), "unique" : size(evaluateQuery(context, unique)), "fromA" : size(evaluateQuery(context, qIntersection([unique, ${verticesOf(['a'])}]))), "points" : vertexPoints(context, unique) };`,
]);
batch.add('tolerance', "a block's and a filleted block's edges and vertices past tolerances of 0, 1e-9 m and 1e-5 m", [
  box('a', [0, 0, 0], [10, 10, 10]),
  box('b', [20, 0, 0], [30, 10, 10]),
  call('fillet', 'fillet1', { entities: `qContainsPoint(qOwnedByBody(qCreatedBy(id + "b", EntityType.BODY), EntityType.EDGE), ${vec3(25, 0, 10)})`, radius: mm(3) }),
  `const both = qUnion([qOwnedByBody(qCreatedBy(id + "a", EntityType.BODY), EntityType.EDGE), qOwnedByBody(qCreatedBy(id + "a", EntityType.BODY), EntityType.VERTEX), qOwnedByBody(qCreatedBy(id + "b", EntityType.BODY), EntityType.EDGE), qOwnedByBody(qCreatedBy(id + "b", EntityType.BODY), EntityType.VERTEX)]);`,
  `return { "zero" : counts(context, qToleranceFilter(both, 0 * meter)), "nano" : counts(context, qToleranceFilter(both, 1e-9 * meter)), "big" : counts(context, qToleranceFilter(both, 1e-5 * meter)) };`,
]);
batch.add('line', "a block (10 mm) and the entities touching lines: through its middle along x, along an edge's line, through a corner diagonally, and missing it", [
  box('a', [0, 0, 0], [10, 10, 10]),
  `const all = everythingOf(id + "a");`,
  `return { "middle" : counts(context, qIntersectsLine(all, line(${vec3(5, 5, 5)}, vector(1, 0, 0)))), "edgeLine" : counts(context, qIntersectsLine(all, line(${vec3(0, 0, 50)}, vector(0, 0, 1)))), "corner" : counts(context, qIntersectsLine(all, line(${vec3(10, 10, 10)}, normalize(vector(1, -1, 2))))), "miss" : counts(context, qIntersectsLine(all, line(${vec3(0, 20, 0)}, vector(1, 0, 0)))) };`,
]);
batch.add('ball', "a block (10 mm) and the entities within radii of points: 1 mm of a corner, 2 mm of a face's middle, 0 mm of an edge's middle, 1 mm of a point 0.5 mm off a face", [
  box('a', [0, 0, 0], [10, 10, 10]),
  `const all = everythingOf(id + "a");`,
  `return { "corner" : counts(context, qWithinRadius(all, ${vec3(10, 10, 10)}, 1 * millimeter)), "face" : counts(context, qWithinRadius(all, ${vec3(5, 5, 10)}, 2 * millimeter)), "edge" : counts(context, qWithinRadius(all, ${vec3(5, 0, 0)}, 0 * millimeter)), "near" : counts(context, qWithinRadius(all, ${vec3(5, 5, 10.5)}, 1 * millimeter)) };`,
]);
batch.add('coedge', "a coedge of a block's top and one of its edges, evaluated", [
  box('a', [0, 0, 0], [10, 10, 10]),
  `const top = qContainsPoint(qOwnedByBody(qCreatedBy(id + "a", EntityType.BODY), EntityType.FACE), ${vec3(5, 5, 10)});`,
  `const edge = qContainsPoint(qOwnedByBody(qCreatedBy(id + "a", EntityType.BODY), EntityType.EDGE), ${vec3(5, 0, 10)});`,
  caught(`return { "count" : size(evaluateQuery(context, qCoEdge(top, edge))) };`),
]);
batch.add('mesh', "a mesh's source of a block's vertex (no meshes)", [
  box('a', [0, 0, 0], [10, 10, 10]),
  caught(`return { "bodies" : size(evaluateQuery(context, qSourceMesh(qOwnedByBody(qCreatedBy(id + "a", EntityType.BODY), EntityType.VERTEX), EntityType.BODY))), "faces" : size(evaluateQuery(context, qSourceMesh(qOwnedByBody(qCreatedBy(id + "a", EntityType.BODY), EntityType.VERTEX), EntityType.FACE))) };`),
]);
batch.add('axis', "a cylinder about z (radius 5) beside a block: the faces and edges about z, about the cylinder's axis reversed, about x, and lines along a block's edge", [
  cylinder('cyl', [0, 0], 5, 0, 20),
  box('a', [10, 0, 0], [20, 10, 10]),
  `const all = qUnion([everythingOf(id + "cyl"), everythingOf(id + "a")]);`,
  `return { "z" : counts(context, qAxis(all, line(${vec3(0, 0, 0)}, vector(0, 0, 1)))), "down" : counts(context, qAxis(all, line(${vec3(0, 0, 7)}, vector(0, 0, -1)))), "x" : counts(context, qAxis(all, line(${vec3(0, 0, 0)}, vector(1, 0, 0)))), "blockEdge" : counts(context, qAxis(all, line(${vec3(10, 0, 3)}, vector(0, 0, 1)))) };`,
]);

batch.write('tools/fixtures/gen-batch-106.mjs');
