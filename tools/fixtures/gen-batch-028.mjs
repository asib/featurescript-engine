// Generates fixture batch 028: more options of covered features. Frame corner overrides (butt, coped butt, none), a
// thin loft, a cylinder's side replaced by a larger cylinder, the isoclines of a sphere as curves, and a fillet's
// radius changed with its neighbouring fillet refilleted.
//
// Usage: node tools/fixtures/gen-batch-028.mjs
//
// Writes fixtures/batch-028/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
// A square tube frame profile, the only sketch of its own context, as a profile from an element library would be.
const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-5, -5) * millimeter, "secondCorner" : vector(5, 5) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-4, -4) * millimeter, "secondCorner" : vector(4, 4) * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}
`;

const batch = new Batch({ number: 28, title: 'more feature options: frame corner overrides, thin lofts, curved faces replaced, sphere isoclines, refilleting', helpers });

const base = solidsOf('base');
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;

// ----- frame corner overrides -----

const lPath = [sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path')];
const corner = `qContainsPoint(qOwnedByBody(qCreatedBy(id + "path", EntityType.BODY), EntityType.VERTEX), ${vec3(100, 0, 0)})`;
const framed = (override) => [
  lPath,
  call('frame', 'frame1', {
    selections: wireEdges('path'), profileSketch: 'profileOf(squareTube)', cornerOverrides: `[{ "vertex" : ${corner}, ${override} }]`,
  }),
];
batch.add('frame', 'a square tube L with a butt corner', [framed('"cornerType" : FrameCornerType.BUTT, "cornerButtFlip" : false')]);
batch.add('frame', 'a square tube L with a coped butt corner, flipped', [framed('"cornerType" : FrameCornerType.COPED_BUTT, "cornerButtFlip" : true')]);
batch.add('frame', 'a square tube L with no corner treatment', [framed('"cornerType" : FrameCornerType.NONE')]);

// ----- thin loft -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
batch.add('loft', 'a thin loft 1 mm thick between a square and a smaller square, its ends left square to its walls', [
  sketch('s1', at(0)), rect('s1', 'rect1', [-10, -10], [10, 10]), solve('s1'),
  sketch('s2', at(20)), rect('s2', 'rect1', [-5, -5], [5, 5]), solve('s2'),
  call('loft', 'loft1', {
    bodyType: 'ExtendedToolBodyType.THIN', operationType: 'NewBodyOperationType.NEW',
    wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges("s1")}]) }, { "wireProfileEntities" : qUnion([${wireEdges("s2")}]) }]`,
    midplane: 'false', thickness1: mm(1), flipWall: 'false', thickness2: mm(0), trimEnds: 'false',
  }),
]);

// ----- a curved face replaced -----

batch.add('replace-face', 'a cylinder\'s side replaced by a coaxial cylindrical sheet 4 mm out', [
  cylinder('base', [0, 0], 10, 0, 20),
  sketch('targetSketch', xy), `skCircle(targetSketch, "circle1", { "center" : vector(0, 0) * millimeter, "radius" : ${mm(14)} });`,
  solve('targetSketch'),
  call('extrude', 'target', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('targetSketch'), endBound: 'BoundingType.BLIND', depth: mm(30),
  }),
  call('replaceFace', 'replace1', {
    replaceFaces: faceAt(base, [10, 0, 10]), templateFace: `qOwnedByBody(qBodyType(qCreatedBy(id + "target", EntityType.BODY), BodyType.SHEET), EntityType.FACE)`,
    offset: mm(0),
  }),
]);

// ----- isoclines of a sphere -----

batch.add('isocline', 'the 30 degree isoclines of a sphere, as curves', [
  call('fSphere', 'base', { radius: mm(10) }), axisLine('up', [0, 0, 0], [0, 0, 1]),
  call('isocline', 'iso1', { faces: `qOwnedByBody(${base}, EntityType.FACE)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(30), split: 'false' }),
]);

// ----- refilleting -----

// A block with its four upright edges rounded 3 mm and then its top edges 2 mm, the upright fillets changed to 5 mm
// with the top fillet refilleted.
batch.add('modify-fillet', 'upright fillets changed to 5 mm, the top fillet refilleted', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', {
    entities: `qUnion([${[[0, 0], [40, 0], [40, 30], [0, 30]].map(([x, y]) => edgeAt(base, [x, y, 10])).join(', ')}])`, radius: mm(3),
  }),
  call('fillet', 'fillet2', { entities: edgeAt(base, [20, 0, 20]), radius: mm(2), tangentPropagation: 'true' }),
  call('modifyFillet', 'modify1', {
    faces: createdBy('fillet1', 'FACE'), modifyFilletType: 'ModifyFilletType.CHANGE_RADIUS', radius: mm(5), reFillet: 'true',
  }),
]);

batch.write('tools/fixtures/gen-batch-028.mjs');
