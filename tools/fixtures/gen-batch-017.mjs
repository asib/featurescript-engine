// Generates fixture batch 017: sheet metal parts cut by extrudes and holes, patterned, rounded by fillets and
// chamfers; walls joined by make joint; a bend relief overridden; a flex PCB model; a sheet metal loft.
//
// Usage: node tools/fixtures/gen-batch-017.mjs
//
// Writes fixtures/batch-017/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, call, circle, deg, edgeOf, line, mm, planeAt, rect, sketch, solve, vec2, vec3 } from './lib.mjs';

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

// The face of a part nearest a point among those lying in planes with a given normal.
function planarFaceNear(context is Context, part is Query, normal is Vector, point is Vector) returns Query
{
    return qClosestTo(qParallelPlanes(qOwnedByBody(part, EntityType.FACE), plane(vector(0, 0, 0) * meter, normal), true), point);
}
`;

const batch = new Batch({ number: 17, title: 'sheet metal cuts, holes, patterns, blends, make joint, bend relief, PCB, loft', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const counts = returns({ counts: 'bodyCounts(context)' });
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const smParts = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const part = smParts('sm1');
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const nearest = (type, p) => `qClosestTo(qOwnedByBody(${part}, EntityType.${type}), ${vec3(...p)})`;

const smFields = { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false' };
const plateStart = (feature = 'sheetMetalStart', fields = {}) => [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call(feature, 'sm1', {
    ...smFields, process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()', ...fields,
  }),
];
const plate = plateStart();
const flange = (edges, fields = {}) =>
  call('sheetMetalFlange', 'flange1', {
    edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', ...fields,
  });
const sideFlange = flange(at('EDGE', 1, 0.5, 1));
const cut = (name, s, fields = {}) =>
  call('extrude', name, {
    entities: `qSketchRegion(id + "${s}")`, endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: part, ...fields,
  });

// ----- cuts and holes -----

batch.add('sheet-metal-cut', 'a 10 mm circle cut through a plate', [
  plate, sketch('c1', planeAt([0, 0, 5], [0, 0, 1], [1, 0, 0])), circle('c1', 'circle1', [20, 15], 5), solve('c1'), cut('cut1', 'c1'), counts,
]);
batch.add('sheet-metal-cut', 'a rectangle cut through a flange', [
  plate, sideFlange,
  sketch('c1', planeAt([50, 0, 0], [1, 0, 0], [0, 1, 0])), rect('c1', 'rect1', [10, 6], [20, 11]), solve('c1'), cut('cut1', 'c1'), counts,
]);
batch.add('sheet-metal-cut', 'a slot cut down through a plate, its bend and its flange', [
  plate, sideFlange,
  sketch('c1', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), rect('c1', 'rect1', [35, 12], [45, 18]), solve('c1'), cut('cut1', 'c1'), counts,
]);
const topPoints = (s, points) => [
  sketch(s, planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])),
  points.map(([x, y], i) => `skPoint(${s}, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve(s),
];
const pointsOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
batch.add('sheet-metal-hole', 'two 6 mm holes through a plate', [
  plate, topPoints('pts1', [[10, 10], [30, 20]]),
  call('hole', 'hole1', { locations: pointsOf('pts1'), scope: part, style: 'HoleStyle.SIMPLE', holeDiameter: mm(6), endStyle: 'HoleEndStyle.THROUGH' }),
  counts,
]);

// ----- patterns -----

const holeInPlate = [plate, sketch('c1', planeAt([0, 0, 5], [0, 0, 1], [1, 0, 0])), circle('c1', 'circle1', [8, 15], 3), solve('c1'), cut('cut1', 'c1')];
batch.add('sheet-metal-pattern', 'a cut hole\'s face patterned 3 times, 10 mm apart', [
  holeInPlate, axisLine('dirX', [0, 0, 0], [1, 0, 0]),
  call('linearPattern', 'pattern1', {
    patternType: 'PatternType.FACE', faces: `qCreatedBy(id + "cut1", EntityType.FACE)`, directionOne: edgeOf('dirX'), distance: mm(10),
    instanceCount: '3', oppositeDirection: 'false',
  }),
  counts,
]);
batch.add('sheet-metal-pattern', 'a plate with a flange patterned as a part, twice, 60 mm apart', [
  plate, sideFlange, axisLine('dirX', [0, 0, 0], [1, 0, 0]),
  call('linearPattern', 'pattern1', {
    patternType: 'PatternType.PART', entities: part, directionOne: edgeOf('dirX'), distance: mm(60), instanceCount: '2', oppositeDirection: 'false',
  }),
  counts,
]);

// ----- fillets and chamfers -----

batch.add('sheet-metal-blend', 'a plate\'s corner edge filleted 5 mm', [
  plate, call('fillet', 'fillet1', { entities: nearest('EDGE', [40, 30, 1]), radius: mm(5) }), counts,
]);
batch.add('sheet-metal-blend', 'a plate\'s corner edge chamfered 4 mm', [
  plate,
  call('chamfer', 'chamfer1', { entities: nearest('EDGE', [0, 0, 1]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(4) }),
  counts,
]);

// ----- make joint -----

// Four walls 25 mm high round a 30 x 20 mm rectangle, bent at three corners and open at the fourth, where the
// last wall stops 5 mm short.
const twoWalls = [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [30, 20]), line('s1', 'l3', [30, 20], [0, 20]),
  line('s1', 'l4', [0, 20], [0, 5]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    ...smFields, process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()',
    endBound: 'SMExtrudeBoundingType.BLIND', oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
];
const wallEnds = `qUnion([planarFaceNear(context, ${part}, vector(1, 0, 0), ${vec3(0, 0, 12.5)}), planarFaceNear(context, ${part}, vector(0, 1, 0), ${vec3(0, 5, 12.5)})])`;
batch.add('sheet-metal-make-joint', 'the open corner of four walls closed by a bend', [
  twoWalls, call('sheetMetalMakeJoint', 'joint1', { entities: wallEnds, joint: 'MakeJointType.BEND', useDefaultRadius: 'true' }), counts,
]);
batch.add('sheet-metal-make-joint', 'the open corner of four walls closed by a butt rip', [
  twoWalls, call('sheetMetalMakeJoint', 'joint1', { entities: wallEnds, joint: 'MakeJointType.RIP', jointType: 'SMJointStyle.BUTT' }), counts,
]);

// ----- bend relief -----

const partialFlange = flange(at('EDGE', 1, 0.5, 1), {
  isPartialFlange: 'true', chainType: 'SMPartialFlangeChainType.PER_EDGE', flipFlangeBounds: 'false', holdAdjacentEdges: 'false',
  firstBoundType: 'SMFlangeBoundingType.BLIND', firstBoundOffset: mm(5), hasSecondBound: 'true',
  secondBoundType: 'SMFlangeBoundingType.BLIND', secondBoundOffset: mm(5),
});
// The bend's outer arc at the end of the flange nearest y = 0.
const bendEnd = `qClosestTo(qGeometry(qOwnedByBody(${part}, EntityType.EDGE), GeometryType.ARC), ${vec3(40, 5, 3)})`;
batch.add('sheet-metal-bend-relief', 'one end of a partial flange given a 4 mm deep rectangular relief', [
  plate, partialFlange,
  call('sheetMetalBendRelief', 'relief1', { bendRelief: bendEnd, bendReliefStyle: 'SMBendReliefStyle.SIZED_RECTANGLE', bendReliefDepth: mm(4), extendBendRelief: 'false' }),
  counts,
]);
batch.add('sheet-metal-bend-relief', 'one end of a partial flange given a tear relief', [
  plate, partialFlange, call('sheetMetalBendRelief', 'relief1', { bendRelief: bendEnd, bendReliefStyle: 'SMBendReliefStyle.TEAR' }), counts,
]);

// ----- flex PCB -----

batch.add('pcb', 'a flex PCB plate, 0.2 mm thick, with a flange, finished', [
  plateStart('pcbStart', { thickness: mm(0.2), radius: mm(1), kFactor: '0.5' }),
  sideFlange,
  call('pcbEnd', 'end1', { unfoldType: 'PcbUnfoldType.UNFOLD_NONE', pcbPart: part, entitiesToUnfold: 'qNothing()', holdEntity: 'qNothing()' }),
  counts,
]);

// ----- loft -----

batch.add('sheet-metal-loft', 'a wall lofted between two parallel lines 20 mm apart in height', [
  sketch('p1', xy), line('p1', 'l1', [0, 0], [30, 0]), solve('p1'),
  sketch('p2', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), line('p2', 'l1', [0, 10], [30, 10]), solve('p2'),
  call('sheetMetalLoft', 'sm1', {
    surfaceOperationType: 'NewSurfaceOperationType.NEW', profile1: wireEdges('p1'), profile2: wireEdges('p2'), matchConnections: 'false',
    chordalTolerance: mm(0.1), ...smFields,
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-017.mjs');
