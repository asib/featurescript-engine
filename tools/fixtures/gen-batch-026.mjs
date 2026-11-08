// Generates fixture batch 026: more options of covered features. Feature patterns that reapply the feature in each
// instance (a boss up to a slanted plane, so each instance ends at its own height), move face creating new surfaces,
// a hole measured to its tip, and a clearance and tapped hole through two stacked blocks.
//
// Usage: node tools/fixtures/gen-batch-026.mjs
//
// Writes fixtures/batch-026/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, createdBy, deg, edgeOf, faceAt, mm, planeAt, planeBody, sketch, solidsOf, solve, vec2, vec3 } from './lib.mjs';

const helpers = `
// The hole attribute of a face, as far as the fields a hole's standard and thread change.
function holeInfo(context is Context, face is Query) returns map
{
    const attributes = getHoleAttributes(context, face);
    if (size(attributes) != 1)
        return { "attributes" : size(attributes) };
    var out = {};
    for (var key in ["holeType", "isTappedHole", "isTappedThrough", "tappedDepth", "holeDiameter", "holeDepth", "partialThrough",
                     "isClearanceAndTapped", "tapDrillDiameter"])
    {
        const value = attributes[0][key];
        out[key] = value is ValueWithUnits ? round(value / millimeter * 1e6) / 1e6 : value;
    }
    return out;
}
`;

const batch = new Batch({ number: 26, title: 'more feature options: reapplied feature patterns, move face creating surfaces, hole tips, clearance and tapped holes', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');

// ----- feature patterns that reapply the feature -----

// A boss on a 60 × 40 × 10 mm block, up to a plane rising 1 in 3 along x, joined to the block.
const bossSketch = [
  `const bossSketch = newSketchOnPlane(context, id + "bossSketch", { "sketchPlane" : plane(${vec3(0, 0, 10)}, vector(0, 0, 1), vector(1, 0, 0)) });`,
  `skCircle(bossSketch, "circle1", { "center" : ${vec2(8, 8)}, "radius" : ${mm(3)} });`,
  'skSolve(bossSketch);',
];
const bossFeature = (name) =>
  `extrude(context, ${name}, { "entities" : qSketchRegion(id + "bossSketch"), "endBound" : BoundingType.UP_TO_SURFACE, "endBoundEntityFace" : qCreatedBy(id + "roof", EntityType.FACE), "operationType" : NewBodyOperationType.ADD, "defaultScope" : false, "booleanScope" : ${base} });`;
const featureList = `{ (id + "boss") : function(instanceId) { ${bossFeature('instanceId')} } } as FeatureList`;
const withBoss = [
  box('base', [0, 0, 0], [60, 40, 10]), planeBody('roof', [30, 0, 30], [-0.31622776601683794, 0, 0.9486832980505138], [0.9486832980505138, 0, 0.31622776601683794]),
  bossSketch, bossFeature('id + "boss"'),
];
batch.add('feature-pattern', 'a boss up to a slanted plane patterned along a block, reapplied in each instance', [
  withBoss,
  call('linearPattern', 'pattern1', {
    patternType: 'PatternType.FEATURE', instanceFunction: featureList, directionOne: `qContainsPoint(qOwnedByBody(${base}, EntityType.EDGE), ${vec3(30, 0, 0)})`,
    distance: mm(20), instanceCount: '3', fullFeaturePattern: 'true',
  }),
]);
batch.add('feature-pattern', 'the boss patterned round the block\'s middle, reapplied in each instance', [
  withBoss, axisLine('axis1', [30, 20, 0], [30, 20, 30]),
  call('circularPattern', 'pattern1', {
    patternType: 'PatternType.FEATURE', instanceFunction: featureList, axis: edgeOf('axis1'), angle: deg(360), instanceCount: '3',
    equalSpace: 'true', fullFeaturePattern: 'true',
  }),
]);

// ----- move face creating surfaces -----

const block = box('base', [0, 0, 0], [30, 20, 10]);
batch.add('move-face', 'a block\'s top translated 5 mm up as a new surface', [
  block, axisLine('dirZ', [0, 0, 0], [0, 0, 1]),
  call('moveFace', 'move1', {
    outputType: 'MoveFaceOutputType.CREATE', surfacesAndFaces: faceAt(base, [15, 10, 10]), moveFaceType: 'MoveFaceType.TRANSLATE',
    direction: edgeOf('dirZ'), limitType: 'MoveFaceBoundingType.BLIND', translationDistance: mm(5), oppositeDirection: 'false',
  }),
]);
batch.add('move-face', 'a block\'s top and front offset 3 mm as new surfaces', [
  block,
  call('moveFace', 'move1', {
    outputType: 'MoveFaceOutputType.CREATE', surfacesAndFaces: `qUnion([${faceAt(base, [15, 10, 10])}, ${faceAt(base, [15, 0, 5])}])`,
    moveFaceType: 'MoveFaceType.OFFSET', limitType: 'MoveFaceBoundingType.BLIND', offsetDistance: mm(3), oppositeDirection: 'false',
  }),
]);

// ----- holes -----

const points = (s, z, list) => [
  sketch(s, planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])),
  list.map(([x, y], i) => `skPoint(${s}, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve(s),
];
const pointsOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
const path = (fields) => `{ ${Object.entries(fields).map(([k, v]) => `"${k}" : "${v}"`).join(', ')} } as LookupTablePath`;
batch.add('hole', 'a 5 mm hole 8 mm deep to the tip of its 118 degree point', [
  box('base', [0, 0, 0], [30, 20, 12]), points('pts1', 12, [[15, 10]]),
  call('hole', 'hole1', {
    locations: pointsOf('pts1'), scope: base, style: 'HoleStyle.SIMPLE', endStyle: 'HoleEndStyle.BLIND', holeDiameter: mm(5), holeDepth: mm(8),
    tipAngle: deg(118), useTipDepth: 'true',
  }),
]);
// Clearance and tapped holes are the newer (V3) hole's: an M6 tapped hole with a normal fit is clearance through
// the first part and tapped in the last.
batch.add('hole', 'a V3 M6 clearance and tapped hole 14 mm deep: clearance through the top block, tapped in the one below', [
  box('base', [0, 0, 0], [30, 20, 12]), box('top', [0, 0, 12], [30, 20, 18]), points('pts1', 18, [[15, 10]]),
  call('hole', 'hole1', {
    isV2: 'true', holeVersion: 'HoleVersion.V3', unitsSystem: 'UnitsSystem.METRIC', styleV2: 'HoleStyle.SIMPLE', style: 'HoleStyle.SIMPLE',
    locations: pointsOf('pts1'), scope: `qUnion([${base}, ${solidsOf('top')}])`, endStyleV2: 'HoleEndStyleV2.BLIND', hasClearance: 'true',
    isoHoleTableEx: path({ holeType: 'Tapped', type: 'Straight tap', size: 'M6', pitch: '1.00 mm (Coarse)', fit: 'Normal' }),
    holeDiameterV3: mm(6.6), tapDrillDiameterV3: mm(5), cBoreDiameterV3: mm(11.25), cBoreDepthV3: mm(6), cSinkDiameterV3: mm(13.44),
    cSinkAngleV3: deg(90), holeDepthComputedV3: mm(0), holeDepthV3: mm(14), tipAngleV3: deg(118), showTappedDepth: 'true', tappedDepthV3: mm(10),
    majorDiameter: mm(6),
  }),
  returns({
    clearance: `holeInfo(context, ${faceAt(solidsOf('top'), [18.3, 10, 15])})`, tapped: `holeInfo(context, ${faceAt(base, [17.5, 10, 9])})`,
  }),
]);

batch.write('tools/fixtures/gen-batch-026.mjs');
