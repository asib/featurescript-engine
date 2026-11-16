// Generates fixture batch 183: probes of sweeps locked to faces with profiles of several loops: a square with a hole
// and a ring along an L rounded 10 mm on a block's top, locked to the top; two squares at once along it; the square
// with a hole up the straight edge of half a cylinder, locked to its curved side; a ring along a spline on the block's
// top, locked to it.
//
// Usage: node tools/fixtures/gen-batch-183.mjs
//
// Writes fixtures/batch-183/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch } from './lib.mjs';

const helpers = `
// Each face of a body: its area and its surface's type; and the body's volume.
function sweepProbe(context is Context, body is Query) returns map
{
    var faces = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try silent
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face }).surfaceType;
        }
        faces = append(faces, entry);
    }
    return { "faces" : faces, "volume" : roundToPrecision(evVolume(context, { "entities" : body }) / millimeter ^ 3, 6) };
}
`;

const batch = new Batch({ number: 183, title: 'probes: sweeps locked to faces with profiles of several loops', helpers });
const solids = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const block = [
  'const baseSketch = newSketchOnPlane(context, id + "baseSketch", { "sketchPlane" : plane(vector(0, 0, -10) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
  'skRectangle(baseSketch, "rect1", { "firstCorner" : vector(-10, -10) * millimeter, "secondCorner" : vector(110, 70) * millimeter });',
  'skSolve(baseSketch);',
  'extrude(context, id + "base", { "entities" : qSketchRegion(id + "baseSketch"), "endBound" : BoundingType.BLIND, "depth" : 10 * millimeter });',
];
const lPath = [
  'const path = newSketchOnPlane(context, id + "path", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
  'skLineSegment(path, "l1", { "start" : vector(0, 0) * millimeter, "end" : vector(90, 0) * millimeter });',
  'skArc(path, "a1", { "start" : vector(90, 0) * millimeter, "mid" : vector(97.071068, 2.928932) * millimeter, "end" : vector(100, 10) * millimeter });',
  'skLineSegment(path, "l2", { "start" : vector(100, 10) * millimeter, "end" : vector(100, 60) * millimeter });',
  'skSolve(path);',
];
const splinePath = [
  'const path = newSketchOnPlane(context, id + "path", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
  'skFitSpline(path, "s1", { "points" : [vector(0, 0) * millimeter, vector(30, 15) * millimeter, vector(60, 5) * millimeter, vector(90, 30) * millimeter] });',
  'skSolve(path);',
];
// The profile in the plane x = 0 (its x along y, its y along z), regions picked by a point in them.
const profPlane = 'const prof = newSketchOnPlane(context, id + "prof", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(1, 0, 0), vector(0, 1, 0)) });';
const holedSquare = [profPlane,
  'skRectangle(prof, "r", { "firstCorner" : vector(-3, 0) * millimeter, "secondCorner" : vector(3, 6) * millimeter });',
  'skCircle(prof, "hole", { "center" : vector(0, 3) * millimeter, "radius" : 1.5 * millimeter });',
  'skSolve(prof);'];
const ring = [profPlane,
  'skCircle(prof, "outer", { "center" : vector(0, 3) * millimeter, "radius" : 3 * millimeter });',
  'skCircle(prof, "inner", { "center" : vector(0, 3) * millimeter, "radius" : 1.5 * millimeter });',
  'skSolve(prof);'];
const twoSquares = [profPlane,
  'skRectangle(prof, "r1", { "firstCorner" : vector(-5, 0) * millimeter, "secondCorner" : vector(-1, 4) * millimeter });',
  'skRectangle(prof, "r2", { "firstCorner" : vector(1, 0) * millimeter, "secondCorner" : vector(5, 4) * millimeter });',
  'skSolve(prof);'];
const annulusRegion = 'qContainsPoint(qSketchRegion(id + "prof"), vector(0, -2.5, 3) * millimeter)';
const sweep = (profiles, path, lock) =>
  `sweep(context, id + "sweep1", { "profiles" : ${profiles}, "path" : ${path}, "bodyType" : ExtendedToolBodyType.SOLID, "operationType" : NewBodyOperationType.NEW, "profileControl" : ProfileControlMode.LOCK_FACES, "lockFaces" : ${lock} });`;
const pathEdges = 'qOwnedByBody(qBodyType(qCreatedBy(id + "path", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)';
const top = `qContainsPoint(qOwnedByBody(${solids('base')}, EntityType.FACE), vector(50, 30, 0) * millimeter)`;
const probe = `return sweepProbe(context, ${solids('sweep1')});`;

batch.add('sweep', 'a 6 mm square with a hole of radius 1.5 in its middle swept along an L rounded 10 mm on a block\'s top, locked to the top', [
  ...block, ...lPath, ...holedSquare, sweep(annulusRegion, pathEdges, top), probe]);
batch.add('sweep', 'a ring (radii 3 and 1.5) swept along the L, locked to the top', [
  ...block, ...lPath, ...ring, sweep(annulusRegion, pathEdges, top), probe]);
batch.add('sweep', 'two 4 mm squares side by side swept along the L, locked to the top', [
  ...block, ...lPath, ...twoSquares, sweep('qSketchRegion(id + "prof")', pathEdges, top), probe]);
batch.add('sweep', 'the square with a hole swept up the straight edge of half a cylinder (radius 10, 30 long), locked to its curved side', [
  'const s1 = newSketchOnPlane(context, id + "s1", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
  'skArc(s1, "a1", { "start" : vector(10, 0) * millimeter, "mid" : vector(0, 10) * millimeter, "end" : vector(-10, 0) * millimeter });',
  'skLineSegment(s1, "l1", { "start" : vector(-10, 0) * millimeter, "end" : vector(10, 0) * millimeter });',
  'skSolve(s1);',
  'extrude(context, id + "base", { "entities" : qSketchRegion(id + "s1"), "endBound" : BoundingType.BLIND, "depth" : 30 * millimeter });',
  'const prof = newSketchOnPlane(context, id + "prof", { "sketchPlane" : plane(vector(10, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
  'skRectangle(prof, "r", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(4, 4) * millimeter });',
  'skCircle(prof, "hole", { "center" : vector(2, 2) * millimeter, "radius" : 1 * millimeter });',
  'skSolve(prof);',
  sweep('qContainsPoint(qSketchRegion(id + "prof"), vector(10.5, 0.5, 0) * millimeter)',
    `qContainsPoint(qOwnedByBody(${solids('base')}, EntityType.EDGE), vector(10, 0, 15) * millimeter)`,
    `qContainsPoint(qOwnedByBody(${solids('base')}, EntityType.FACE), vector(0, 10, 15) * millimeter)`), probe]);
batch.add('sweep', 'the ring swept along a fit spline on the block\'s top, locked to the top', [
  ...block, ...splinePath, ...ring, sweep(annulusRegion, pathEdges, top), probe]);

batch.write('tools/fixtures/gen-batch-183.mjs');
