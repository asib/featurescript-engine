// Generates fixture batch 024: more options of covered features. Tapped and clearance holes from the standard
// tables (with their hole attributes), the newer (V3) hole, revolves with their second direction up to a face and a
// vertex, a gusset aligned to an edge, ruled surfaces with vertex overrides, and a wrap onto a cone.
//
// Usage: node tools/fixtures/gen-batch-024.mjs
//
// Writes fixtures/batch-024/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, axisLine, box, call, createdBy, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, sketch, solidsOf, solve, vec2, vec3,
  vertexAt,
} from './lib.mjs';

const helpers = `
// The hole attribute of a face, as far as the fields a hole's standard and thread change.
function holeInfo(context is Context, face is Query) returns map
{
    const attributes = getHoleAttributes(context, face);
    if (size(attributes) != 1)
        return { "attributes" : size(attributes) };
    var out = {};
    for (var key in ["holeType", "isTappedHole", "isTappedThrough", "tappedDepth", "tapDrillDiameter", "majorDiameter",
                     "threadPitch", "holeDiameter", "holeDepth", "partialThrough", "isClearanceAndTapped", "tapClearance",
                     "standard", "size", "pitch", "fit"])
    {
        const value = attributes[0][key];
        out[key] = value is ValueWithUnits ? round(value / millimeter * 1e6) / 1e6 : value;
    }
    return out;
}

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

const batch = new Batch({ number: 24, title: 'more feature options: tapped, clearance and V3 holes, revolve start bounds, aligned gusset, ruled vertex overrides, wrap onto a cone', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const block = box('base', [0, 0, 0], [30, 20, 10]);
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const points = (s, z, list) => [
  sketch(s, planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])),
  list.map(([x, y], i) => `skPoint(${s}, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve(s),
];
const pointsOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
const path = (fields) => `{ ${Object.entries(fields).map(([k, v]) => `"${k}" : "${v}"`).join(', ')} } as LookupTablePath`;
const info = (x, y, z) => returns({ hole: `holeInfo(context, ${faceAt(base, [x, y, z])})` });

// ----- holes -----

batch.add('hole', 'an M6 tapped hole, 8 mm deep and tapped 6 mm', [
  block, points('pts1', 10, [[15, 10]]),
  call('hole', 'hole1', {
    locations: pointsOf('pts1'), scope: base, style: 'HoleStyle.SIMPLE', endStyle: 'HoleEndStyle.BLIND', holeDiameter: mm(5), holeDepth: mm(8),
    tapDrillDiameter: mm(5), showTappedDepth: 'true', tappedDepth: mm(6),
    standardTappedOrClearance: path({ standard: 'ISO', type: 'Tapped', size: 'M6', pitch: '1.00 mm', engagement: '75%' }),
  }),
  info(17.5, 10, 5),
]);
batch.add('hole', 'an M6 normal clearance hole through', [
  block, points('pts1', 10, [[15, 10]]),
  call('hole', 'hole1', {
    locations: pointsOf('pts1'), scope: base, style: 'HoleStyle.SIMPLE', endStyle: 'HoleEndStyle.THROUGH', holeDiameter: mm(6.6),
    standardTappedOrClearance: path({ standard: 'ISO', type: 'Clearance', size: 'M6', fit: 'Normal' }),
  }),
  info(18.3, 10, 5),
]);
batch.add('hole', 'a V3 counterbored M5 clearance hole through', [
  block, points('pts1', 10, [[15, 10]]),
  call('hole', 'hole1', {
    isV2: 'true', holeVersion: 'HoleVersion.V3', unitsSystem: 'UnitsSystem.METRIC', styleV2: 'HoleStyle.C_BORE', style: 'HoleStyle.C_BORE', locations: pointsOf('pts1'),
    scope: base, isoHoleTable: path({ holeType: 'Clearance', size: 'M5', fit: 'Normal' }), endStyleV2: 'HoleEndStyleV2.THROUGH',
    holeDiameterV3: mm(5.5), tapDrillDiameterV3: mm(5.5), cBoreDiameterV3: mm(9.75), cBoreDepthV3: mm(5), cSinkDiameterV3: mm(11.2),
    cSinkAngleV3: deg(90), holeDepthComputedV3: mm(0), holeDepthV3: mm(10), tipAngleV3: deg(118), tappedDepthV3: mm(10),
  }),
  info(17.75, 10, 2.5),
]);
batch.add('hole', 'a V3 simple hole 8 mm deep with a 118 degree tip', [
  block, points('pts1', 10, [[15, 10]]),
  call('hole', 'hole1', {
    isV2: 'true', holeVersion: 'HoleVersion.V3', unitsSystem: 'UnitsSystem.METRIC', styleV2: 'HoleStyle.SIMPLE', locations: pointsOf('pts1'),
    scope: base, isoHoleTableEx: path({ holeType: 'Drilled' }), endStyleV2: 'HoleEndStyleV2.BLIND',
    holeDiameterV3: mm(4), tapDrillDiameterV3: mm(4), cBoreDiameterV3: mm(8), cBoreDepthV3: mm(4), cSinkDiameterV3: mm(8),
    cSinkAngleV3: deg(90), holeDepthComputedV3: mm(0), holeDepthV3: mm(8), tipAngleV3: deg(118), tappedDepthV3: mm(8),
  }),
  info(17, 10, 5),
]);

// ----- revolves with a second direction -----

const revolveProfile = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [10, -5], [20, 5]), solve('s1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
];
batch.add('revolve', '60 degrees one way, and up to a face the other', [
  box('base', [-40, -15, -20], [40, -5, 20]), revolveProfile,
  call('revolve', 'revolve1', {
    entities: `qSketchRegion(id + "s1")`, axis: edgeOf('axis1'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', angle: deg(60),
    oppositeDirection: 'false', symmetric: 'false', hasStartBound: 'true', startBound: 'RevolveBoundingType.UP_TO_SURFACE',
    startOppositeDirection: 'false', startBoundEntityFace: faceAt(base, [0, -5, 0]), startBoundHasOffset: 'false',
  }),
]);
batch.add('revolve', '60 degrees one way, and up to a vertex 10 degrees past it the other', [
  box('base', [-40, -40, -20], [-30, -30, 20]), revolveProfile,
  call('revolve', 'revolve1', {
    entities: `qSketchRegion(id + "s1")`, axis: edgeOf('axis1'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', angle: deg(60),
    oppositeDirection: 'false', symmetric: 'false', hasStartBound: 'true', startBound: 'RevolveBoundingType.UP_TO_VERTEX',
    startOppositeDirection: 'false', startBoundEntityVertex: vertexAt(base, [-30, -30, 20]), startBoundHasOffset: 'true',
    startBoundOffset: deg(10), startBoundOffsetFlip: 'false',
  }),
]);

// ----- a gusset aligned to an edge -----

const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
batch.add('gusset', 'a triangle in the inside corner of a mitred L, aligned 2 mm up to a block\'s edge', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path') }),
  box('other', [85, 20, -10], [95, 30, 2]),
  call('gusset', 'gusset1', {
    edges: edgeAt(solidsOf('frame1'), [95, 5, 0]), gussetType: 'GussetStyleType.TRIANGLE', length: mm(15), shouldFlipBaseSides: 'false',
    thickness: mm(2), gussetPosition: 'GussetPosition.ALIGNED', alignedReference: edgeAt(solidsOf('other'), [95, 25, 2]),
    shouldFlipAlignment: 'false', offset: mm(0), shouldFlipOffset: 'false', baseSweptFaces: 'qNothing()',
  }),
]);

// ----- ruled surfaces with vertex overrides -----

batch.add('ruled-surface', 'normal to a block\'s front from its top edge, 6 mm, 12 mm at one end', [
  block,
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [15, 0, 10]), ruledType: 'RuledSurfaceInterfaceType.NORMAL', distance: mm(6), referenceFaces: faceAt(base, [15, 0, 5]),
    vertexOverrides: `[{ "vertex" : ${vertexAt(base, [30, 0, 10])}, "overrideType" : VertexOverrideType.NORMAL, "angleOverride" : ${deg(0)},
            "oppositeAngleOverride" : false, "distanceOverride" : ${mm(12)}, "oppositeDirectionOverride" : false }]`,
  }),
]);
batch.add('ruled-surface', 'normal to a block\'s front from its top edge, up to a vertex at one end', [
  block, `opPoint(context, id + "p1", { "point" : ${vec3(-4, -8, 10)} });`,
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [15, 0, 10]), ruledType: 'RuledSurfaceInterfaceType.NORMAL', distance: mm(6), referenceFaces: faceAt(base, [15, 0, 5]),
    vertexOverrides: `[{ "vertex" : ${vertexAt(base, [0, 0, 10])}, "overrideType" : VertexOverrideType.UP_TO_VERTEX,
            "boundaryOverride" : qCreatedBy(id + "p1", EntityType.VERTEX) }]`,
  }),
]);

// A planar sheet that isn't symmetric, to see where Onshape puts a plane's origin: at the face's centroid (10, 4)
// or the middle of its box (15, 6).
batch.add('fill', 'a right triangle filled', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [0, 12]), line('s1', 'l3', [0, 12], [0, 0]), solve('s1'),
  call('fill', 'fill1', {
    edges: `[{ "entities" : ${wireEdges('s1')}, "continuity" : GeometricContinuity.G0 }]`, surfaceOperationType: 'NewSurfaceOperationType.NEW',
  }),
]);

// ----- a wrap onto a cone -----

batch.add('wrap', 'a rectangle wrapped onto a cone as a surface', [
  call('fCone', 'base', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 30), bottomRadius: mm(12), topRadius: mm(8) }),
  sketch('s1', planeAt([0, -12, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [-5, 10], [5, 20]), solve('s1'),
  call('wrap', 'wrap1', {
    resultType: 'WrapResultType.SURFACE', source: `qCreatedBy(id + "s1", EntityType.FACE)`, destination: faceAt(base, [0, -10, 15]),
    flipAlignment: 'false', trim: 'true', customAnchors: 'false',
  }),
]);

batch.write('tools/fixtures/gen-batch-024.mjs');
