// Generates fixture batch 019: more options of covered features. Sweeps twisted and scaled, ribs extended to the
// part, end caps (circle, internal, chamfered), ruled surfaces tangent and at an angle from a vector, curve patterns
// keeping their orientation, a curve projected along the target's normal, and sheet
// metal: sized obround and extended bend reliefs, a bend from a sketch arc unfolded.
//
// Usage: node tools/fixtures/gen-batch-019.mjs
//
// Writes fixtures/batch-019/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, circle, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, sketch,
  solidsOf, solve,
} from './lib.mjs';

const frameProfiles = `
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

const helpers = `
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

function atFraction(context is Context, part is Query, entityType is EntityType, fx is number, fy is number, fz is number) returns Query
{
    return qContainsPoint(qOwnedByBody(part, entityType), pointAtFraction(context, part, fx, fy, fz));
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
${frameProfiles}`;

const batch = new Batch({ number: 19, title: 'more feature options: sweeps, ribs, end caps, ruled surfaces, curve patterns, projection, sheet metal reliefs and unfolds', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const regions = (s) => `qSketchRegion(id + "${s}")`;
const base = solidsOf('base');
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);

// ----- sweeps -----

const straightPath = [sketch('path', PLANES.xy), line('path', 'line1', [0, 0], [40, 0]), solve('path')];
batch.add('sweep', 'a rectangle along a line, twisted a quarter turn', [
  sketch('profile', PLANES.yz), rect('profile', 'rect1', [-3, -2], [3, 2]), solve('profile'), straightPath,
  call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasTwist: 'true', twistType: 'SweepTwistType.ANGLE', angle: deg(90), ccw: 'true' }),
]);
batch.add('sweep', 'a circle along a line, scaled to twice its size', [
  sketch('profile', PLANES.yz), circle('profile', 'circle1', [0, 0], 3), solve('profile'), straightPath,
  call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasScale: 'true', scaleFactor: '2' }),
]);

// ----- ribs, end caps, gussets -----

batch.add('rib', 'a short profile extended to the part', [
  box('base', [0, 0, 0], [40, 30, 4]), box('wall', [0, 0, 4], [4, 30, 24]),
  `opBoolean(context, id + "join", { "tools" : qUnion([${base}, ${solidsOf('wall')}]), "operationType" : BooleanOperationType.UNION });`,
  sketch('profile', planeAt([0, 15, 0], [0, -1, 0], [1, 0, 0])), line('profile', 'l1', [10, 10], [14, 6]), solve('profile'),
  call('rib', 'rib1', {
    profiles: wireEdges('profile'), parts: base, thickness: mm(3), ribExtrusionDirection: 'RibExtrusionDirection.PARALLEL_TO_SKETCH_PLANE',
    oppositeDirection: 'true', hasDraft: 'false', extendProfilesUpToPart: 'true', mergeRibs: 'true',
  }),
]);
const frames = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const lPath = [sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path')];
const frame = call('frame', 'frame1', {
  selections: wireEdges('path'), profileSketch: 'profileOf(squareTube)',
});
const openEnd = faceAt(frames('frame1'), [0, 4.5, 0]);
batch.add('endcap', 'a circle at the open end of an L', [
  lPath, frame,
  call('endcap', 'cap1', { faces: openEnd, profileType: 'ProfileType.CIRCLE', thickness: mm(2), thicknessDirection: 'false', offsetDistance: mm(1), offsetDirection: 'false' }),
]);
batch.add('endcap', 'internal, at the open end of an L', [
  lPath, frame,
  call('endcap', 'cap1', {
    faces: openEnd, profileType: 'ProfileType.INTERNAL', thickness: mm(2), thicknessDirection: 'false', internalOffsetDistance: mm(0.2),
    internalShiftDistance: mm(1),
  }),
]);
batch.add('endcap', 'a chamfered rectangle at the open end of an L', [
  lPath, frame,
  call('endcap', 'cap1', {
    faces: openEnd, profileType: 'ProfileType.RECTANGLE', thickness: mm(2), thicknessDirection: 'false', offsetDistance: mm(1),
    offsetDirection: 'false', cornersType: 'CornerType.CHAMFER', cornerDistance: mm(2),
  }),
]);

// ----- ruled surfaces, curve patterns, projection -----

const block = box('base', [0, 0, 0], [30, 20, 10]);
batch.add('ruled-surface', 'from a top edge of a block, tangent to its side, 6 mm', [
  block,
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [15, 0, 10]), ruledType: 'RuledSurfaceInterfaceType.TANGENT', distance: mm(6), referenceFaces: faceAt(base, [15, 0, 5]),
  }),
]);
batch.add('ruled-surface', 'from a top edge of a block, 30 degrees from a direction', [
  block, axisLine('dir', [0, 0, 0], [0, 0, 1]),
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [15, 0, 10]), ruledType: 'RuledSurfaceInterfaceType.ANGLE_FROM_VECTOR', axis: edgeOf('dir'), angleFromVector: deg(30),
    distance: mm(8), referenceFaces: faceAt(base, [15, 0, 5]),
  }),
]);
batch.add('curve-pattern', 'a block patterned along an arc, keeping its orientation', [
  box('base', [-2, -2, -2], [2, 2, 2]),
  sketch('path', PLANES.xy), arc('path', 'arc1', [0, 0], [20, 8.28427], [28.28427, 28.28427]), solve('path'),
  call('curvePattern', 'pattern1', {
    entities: base, edges: wireEdges('path'), spacingType: 'CurvePatternSpacingType.EQUAL', instanceCount: '3',
    orientationType: 'CurvePatternOrientationType.KEEP_ORIENTATION',
  }),
]);
batch.add('project-curves', 'a line projected onto a cylinder along its normals', [
  cylinder('base', [0, 0], 20, -20, 20),
  sketch('s1', planeAt([30, 0, 0], [1, 0, 0], [0, 1, 0])), line('s1', 'line1', [-5, -10], [5, 10]), solve('s1'),
  call('projectCurves', 'project1', {
    curveProjectionType: 'CurveProjectionType.CURVE_TO_FACE', dropTools: wireEdges('s1'), targets: faceAt(base, [20, 0, 0]),
    projectionType: 'ProjectionType.NORMAL_TO_TARGET',
  }),
]);

// ----- sheet metal -----

const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const counts = returns({ counts: 'bodyCounts(context)' });
const smFields = { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false' };
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    ...smFields, process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
const partialFlange = call('sheetMetalFlange', 'flange1', {
  edges: at('EDGE', 1, 0.5, 1), flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
  angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
  useDefaultRadius: 'true', isPartialFlange: 'true', chainType: 'SMPartialFlangeChainType.PER_EDGE', flipFlangeBounds: 'false',
  holdAdjacentEdges: 'false', firstBoundType: 'SMFlangeBoundingType.BLIND', firstBoundOffset: mm(5), hasSecondBound: 'true',
  secondBoundType: 'SMFlangeBoundingType.BLIND', secondBoundOffset: mm(5),
});
const bendEnd = `qClosestTo(qGeometry(qOwnedByBody(${part}, EntityType.EDGE), GeometryType.ARC), vector(40, 5, 3) * millimeter)`;
batch.add('sheet-metal-bend-relief', 'one end of a partial flange given a 4 mm deep sized obround relief', [
  plate, partialFlange,
  call('sheetMetalBendRelief', 'relief1', { bendRelief: bendEnd, bendReliefStyle: 'SMBendReliefStyle.SIZED_OBROUND', bendReliefDepth: mm(4), extendBendRelief: 'false' }),
  counts,
]);
batch.add('sheet-metal-bend-relief', 'one end of a partial flange given an extended obround relief', [
  plate, partialFlange,
  call('sheetMetalBendRelief', 'relief1', {
    bendRelief: bendEnd, bendReliefStyle: 'SMBendReliefStyle.OBROUND', bendReliefDepthScale: '1.5', bendReliefWidthScale: '1.0625', extendBendRelief: 'true',
  }),
  counts,
]);
batch.add('sheet-metal-unfold', 'an L bent round a 3 mm sketch arc, unfolded', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [27, 0]), arc('s1', 'a1', [27, 0], [29.12132, 0.87868], [30, 3]), line('s1', 'l2', [30, 3], [30, 20]),
  solve('s1'),
  call('sheetMetalStart', 'sm1', {
    ...smFields, process: 'SMProcessType.EXTRUDE',
    sketchCurves: `qGeometry(${wireEdges('s1')}, GeometryType.LINE)`, bendArcs: `qGeometry(${wireEdges('s1')}, GeometryType.ARC)`,
    endBound: 'SMExtrudeBoundingType.BLIND', oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  call('sheetMetalUnfold', 'unfold1', {
    unfoldAll: 'false', partToUnfold: 'qNothing()',
    entitiesToUnfold: `qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0)`,
    holdEntity: `qClosestTo(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.PLANE), vector(10, 0, 12.5) * millimeter)`,
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-019.mjs');
