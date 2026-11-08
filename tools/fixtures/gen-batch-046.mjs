// Generates fixture batch 046: faces unwrapped from cylinders and cones onto planes (`opWrap`, the reverse of the
// `wrap` feature): a cylinder's whole side, a quarter of one, and a cone's side.
//
// Usage: node tools/fixtures/gen-batch-046.mjs
//
// Writes fixtures/batch-046/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, cylinder, mm, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

const batch = new Batch({ number: 46, title: 'faces unwrapped from cylinders and cones onto planes' });

const base = solidsOf('base');
const side = (p) => `qContainsPoint(qOwnedByBody(${base}, EntityType.FACE), ${vec3(...p)})`;
// The plane z = −5, anchored at (0, −40, −5) along x.
const flat = `{ "plane" : plane(${vec3(0, 0, -5)}, vector(0, 0, 1), vector(1, 0, 0)), "anchorPoint" : ${vec3(0, -40, -5)}, "anchorDirection" : vector(1, 0, 0) } as WrapSurface`;
const unwrap = (face) =>
  `opWrap(context, id + "unwrap1", { "wrapType" : WrapType.SIMPLE, "entities" : ${face}, "source" : makeWrapSurface(context, ${face}), "destination" : ${flat} });`;

batch.add('unwrap', 'a cylinder\'s side, radius 10 and 20 mm tall, unwrapped onto a plane', [
  cylinder('base', [0, 0], 10, 0, 20), unwrap(side([10, 0, 10])),
]);
// A quarter of a cylinder: a sheet extruded 20 mm up from a sketch arc of radius 10 from (10, 0) to (0, 10).
const sheet = `qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SHEET)`;
const sheetFace = `qOwnedByBody(${sheet}, EntityType.FACE)`;
batch.add('unwrap', 'a quarter cylinder sheet (radius 10, 20 mm tall, extruded from an arc) unwrapped onto a plane', [
  sketch('s1', PLANES.xy), arc('s1', 'a1', [10, 0], [7.0710678118654755, 7.0710678118654755], [0, 10]), solve('s1'),
  call('extrude', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qOwnedByBody(qBodyType(qCreatedBy(id + "s1", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`,
    endBound: 'BoundingType.BLIND', depth: mm(20),
  }),
  unwrap(sheetFace),
  // The cylinder's coordinate system, which anchors the unwrap.
  `const cs = evSurfaceDefinition(context, { "face" : ${sheetFace} }).coordSystem;`,
  'return { "frame" : [[roundToPrecision(cs.origin[0] / millimeter, 6), roundToPrecision(cs.origin[1] / millimeter, 6), roundToPrecision(cs.origin[2] / millimeter, 6)], [roundToPrecision(cs.xAxis[0], 9), roundToPrecision(cs.xAxis[1], 9), roundToPrecision(cs.xAxis[2], 9)], [roundToPrecision(cs.zAxis[0], 9), roundToPrecision(cs.zAxis[1], 9), roundToPrecision(cs.zAxis[2], 9)]] };',
]);
batch.add('unwrap', 'a cone\'s side, radii 10 and 5 and 20 mm tall, unwrapped onto a plane', [
  call('fCone', 'base', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 20), bottomRadius: mm(10), topRadius: mm(5) }),
  unwrap(side([7.5, 0, 10])),
]);

batch.write('tools/fixtures/gen-batch-046.mjs');
