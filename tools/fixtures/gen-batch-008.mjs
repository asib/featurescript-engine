// Generates fixture batch 008: frames (built from profiles made by local build functions, as an element library's
// profile would be), frame trims, gussets, end caps, and how a sketch's regions are grouped into sheets.
//
// Usage: node tools/fixtures/gen-batch-008.mjs
//
// Writes fixtures/batch-008/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, createdBy, deg, edgeAt, faceAt, line, mm, planeAt, planeBody, sketch, solve, vec2 } from './lib.mjs';

const helpers = `
// Frame profiles, each the only sketch of its own context, as a profile from an element library would be. They're
// lambdas: FeatureScript doesn't let a named function be used as a value.
const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-5, -5) * millimeter, "secondCorner" : vector(5, 5) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-4, -4) * millimeter, "secondCorner" : vector(4, 4) * millimeter });
    skSolve(s);
    return context;
};

const flatBar = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "bar", { "firstCorner" : vector(0, -2) * millimeter, "secondCorner" : vector(12, 2) * millimeter });
    skSolve(s);
    return context;
};

const roundTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skCircle(s, "outer", { "center" : vector(0, 0) * millimeter, "radius" : 6 * millimeter });
    skCircle(s, "inner", { "center" : vector(0, 0) * millimeter, "radius" : 5 * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}
`;

const batch = new Batch({ number: 8, title: 'frames, frame trims, gussets, end caps, and sketch regions as sheets', helpers });

const union = (...queries) => `qUnion([${queries.join(', ')}])`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const frames = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const frame = (name, path, profile, fields = {}) =>
  call('frame', name, { profileSketch: `profileOf(${profile})`, selections: wireEdges(path), ...fields });
const lPath = [sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path')];

// ----- how a sketch's regions become sheets -----

batch.add('sketch', 'a ring and the square inside it, and a square apart', [
  sketch('s1', xy),
  `skRectangle(s1, "outer", { "firstCorner" : ${vec2(0, 0)}, "secondCorner" : ${vec2(20, 20)} });`,
  `skRectangle(s1, "inner", { "firstCorner" : ${vec2(5, 5)}, "secondCorner" : ${vec2(15, 15)} });`,
  `skRectangle(s1, "apart", { "firstCorner" : ${vec2(30, 0)}, "secondCorner" : ${vec2(40, 10)} });`,
  solve('s1'),
  returns({
    sheets: 'evaluateQueryCount(context, qBodyType(qCreatedBy(id + "s1", EntityType.BODY), BodyType.SHEET))',
    faces: 'evaluateQueryCount(context, qOwnedByBody(qBodyType(qCreatedBy(id + "s1", EntityType.BODY), BodyType.SHEET), EntityType.FACE))',
    regions: 'evaluateQueryCount(context, qSketchRegion(id + "s1"))',
  }),
]);

// ----- frames -----

batch.add('frame', 'a square tube along an L, mitred', [lPath, frame('frame1', 'path', 'squareTube')]);
batch.add('frame', 'a square tube around a rectangle, mitred', [
  sketch('path', xy), `skRectangle(path, "rect1", { "firstCorner" : ${vec2(0, 0)}, "secondCorner" : ${vec2(80, 50)} });`, solve('path'),
  frame('frame1', 'path', 'squareTube'),
]);
batch.add('frame', 'a square tube along an L, butted', [
  lPath, frame('frame1', 'path', 'squareTube', { defaultCornerType: 'FrameCornerType.BUTT', defaultButtFlip: 'false' }),
]);
batch.add('frame', 'a square tube along an L, butted the other way', [
  lPath, frame('frame1', 'path', 'squareTube', { defaultCornerType: 'FrameCornerType.BUTT', defaultButtFlip: 'true' }),
]);
batch.add('frame', 'a square tube along an L, coped', [
  lPath, frame('frame1', 'path', 'squareTube', { defaultCornerType: 'FrameCornerType.COPED_BUTT', defaultButtFlip: 'false' }),
]);
batch.add('frame', 'a square tube along an L, corners left alone', [
  lPath, frame('frame1', 'path', 'squareTube', { defaultCornerType: 'FrameCornerType.NONE' }),
]);
batch.add('frame', 'a flat bar off its centre, turned 30 degrees and mirrored', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), solve('path'),
  frame('frame1', 'path', 'flatBar', { angle: deg(30), mirrorProfile: 'true' }),
]);
batch.add('frame', 'a round tube along a line and a tangent arc', [
  sketch('path', xy), line('path', 'l1', [0, 0], [60, 0]), arc('path', 'a1', [60, 0], [74.1421, 5.8579], [80, 20]), solve('path'),
  frame('frame1', 'path', 'roundTube'),
]);
batch.add('frame', 'a square tube trimmed to a slanted plane', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), solve('path'),
  planeBody('cut', [80, 0, 0], [1, 0, 0.5], [0, 1, 0]),
  frame('frame1', 'path', 'squareTube', { trim: 'true', trimPlanes: createdBy('cut', 'FACE'), trimBodies: 'qNothing()' }),
]);

// ----- frame trims -----

const cross = [
  sketch('a', xy), line('a', 'l1', [0, 30], [100, 30]), solve('a'),
  sketch('b', xy), line('b', 'l1', [50, 30], [50, 80]), solve('b'),
  frame('fa', 'a', 'squareTube'), frame('fb', 'b', 'squareTube'),
];
batch.add('frame-trim', 'a tube ending on another, trimmed in ordered groups', [
  cross,
  call('frameTrim', 'trim1', {
    trimType: 'FrameTrimType.GROUP',
    frameGroups: `[{ "frames" : ${frames('fa')} }, { "frames" : ${frames('fb')} }]`,
  }),
]);
batch.add('frame-trim', 'a tube trimmed to a plane', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), solve('path'), frame('frame1', 'path', 'squareTube'),
  planeBody('cut', [70, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('frameTrim', 'trim1', {
    trimType: 'FrameTrimType.FACE', targets: frames('frame1'), tool: createdBy('cut', 'FACE'), flipTrim: 'false', useToolExtents: 'false',
  }),
]);

// ----- gussets and end caps -----

const corner = edgeAt(frames('frame1'), [95, 5, 0]);
batch.add('gusset', 'a triangle in the inside corner of a mitred L', [
  lPath, frame('frame1', 'path', 'squareTube'),
  call('gusset', 'gusset1', {
    edges: corner, gussetType: 'GussetStyleType.TRIANGLE', length: mm(15), shouldFlipBaseSides: 'false', thickness: mm(2),
    gussetPosition: 'GussetPosition.CENTERED',
    offset: mm(0), shouldFlipOffset: 'false', baseSweptFaces: 'qNothing()',
  }),
]);
batch.add('gusset', 'a chamfered rectangle in the inside corner of a mitred L', [
  lPath, frame('frame1', 'path', 'squareTube'),
  call('gusset', 'gusset1', {
    edges: corner, gussetType: 'GussetStyleType.RECTANGLE', length: mm(12), shouldFlipBaseSides: 'false', thickness: mm(2),
    chamfer: 'true', chamferSize: mm(4), gussetPosition: 'GussetPosition.CENTERED', offset: mm(0), shouldFlipOffset: 'false',
    baseSweptFaces: 'qNothing()',
  }),
]);
batch.add('endcap', 'matching the profile at the open end of an L', [
  lPath, frame('frame1', 'path', 'squareTube'),
  call('endcap', 'cap1', {
    faces: faceAt(frames('frame1'), [0, 4.5, 0]), profileType: 'ProfileType.MATCH_PROFILE', thickness: mm(2), thicknessDirection: 'false',
    offsetDistance: mm(0.5), offsetDirection: 'false',
  }),
]);
batch.add('endcap', 'a filleted rectangle at the open end of an L', [
  lPath, frame('frame1', 'path', 'squareTube'),
  call('endcap', 'cap1', {
    faces: faceAt(frames('frame1'), [0, 4.5, 0]), profileType: 'ProfileType.RECTANGLE', thickness: mm(2), thicknessDirection: 'false',
    offsetDistance: mm(1), offsetDirection: 'false', cornersType: 'CornerType.FILLET', cornerRadius: mm(2),
  }),
]);

batch.write('tools/fixtures/gen-batch-008.mjs');
