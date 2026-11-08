// Generates fixture batch 020: more options of covered features. A face replaced by a curved sheet's face, a sheet
// extended up to a face, wraps as a solid and as an imprint, a measured diameter, an isoparametric curve through a
// point, a fit spline with a start magnitude, a curved edge offset on a face, a mate connector with its normal
// given, a curve pattern locked to faces, and sheet metal corner breaks: a two-offset
// chamfer and a conic fillet.
//
// Usage: node tools/fixtures/gen-batch-020.mjs
//
// Writes fixtures/batch-020/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, planeBody, rect,
  sketch, solidsOf, solve, vec3, vertexAt,
} from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

function roundVector(v is Vector) returns Vector
{
    return vector(round(v[0] * 1e6) / 1e6, round(v[1] * 1e6) / 1e6, round(v[2] * 1e6) / 1e6);
}

function connectorFrame(context is Context, connector is Query) returns map
{
    const cs = evMateConnector(context, { "mateConnector" : connector });
    return { "origin" : roundVector(cs.origin / millimeter), "x" : roundVector(cs.xAxis), "z" : roundVector(cs.zAxis) };
}
`;

const batch = new Batch({ number: 20, title: 'more feature options: replace face, extend up to, wraps, measured diameter, curves, mate connector normal, sheet metal corner breaks', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const base = solidsOf('base');
const block = box('base', [0, 0, 0], [30, 20, 10]);
const can = cylinder('base', [0, 0], 10, 0, 30);
const surfaceFrom = (name, plane, curve, depth) => [
  sketch(`${name}Sketch`, plane), curve(`${name}Sketch`), solve(`${name}Sketch`),
  call('extrude', name, {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges(`${name}Sketch`), endBound: 'BoundingType.BLIND', depth: mm(depth),
  }),
];

// ----- faces and surfaces -----

batch.add('replace-face', 'a block\'s top replaced by a curved sheet above it', [
  block, surfaceFrom('target', planeAt([0, -5, 0], [0, -1, 0], [1, 0, 0]), (s) => arc(s, 'arc1', [-5, 11], [15, 14], [35, 11]), 30),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: `qOwnedByBody(${sheetOf('target')}, EntityType.FACE)`, offset: mm(0) }),
]);
batch.add('extend-surface', 'a sheet extended up to a plane', [
  surfaceFrom('base', PLANES.xy, (s) => line(s, 'line1', [0, 0], [20, 0]), 20), planeBody('limit', [28, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('extendSurface', 'extend1', {
    entities: edgeAt(sheetOf('base'), [20, 0, 10]), endCondition: 'ExtendBoundingType.UP_TO_FACE', targetFace: createdBy('limit', 'FACE'),
    hasOffset: 'false',
  }),
]);
const wrapSource = [sketch('s1', planeAt([0, -10, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [-5, 10], [5, 20]), solve('s1')];
batch.add('wrap', 'a rectangle wrapped onto a cylinder as a 1 mm solid', [
  can, wrapSource,
  call('wrap', 'wrap1', {
    resultType: 'WrapResultType.SOLID', source: `qCreatedBy(id + "s1", EntityType.FACE)`, destination: faceAt(base, [0, -10, 15]),
    flipAlignment: 'false', trim: 'true', customAnchors: 'false', thickness: mm(1), oppositeDirection: 'false',
    operationType: 'NewBodyOperationType.NEW',
  }),
]);
batch.add('wrap', 'a rectangle wrapped onto a cylinder as an imprint', [
  can, wrapSource,
  call('wrap', 'wrap1', {
    resultType: 'WrapResultType.IMPRINT', source: `qCreatedBy(id + "s1", EntityType.FACE)`, destination: faceAt(base, [0, -10, 15]),
    flipAlignment: 'false', trim: 'true', customAnchors: 'false',
  }),
]);

// ----- variables, curves, mate connectors -----

batch.add('variable', 'measure a cylinder\'s diameter', [
  can,
  call('assignVariable', 'variable1', {
    name: '"d"', mode: 'VariableMode.MEASURED', measurementMode: 'VariableMeasurementMode.DIAMETER', radius: 'false',
    diameterEntity: edgeAt(base, [10, 0, 30]),
  }),
  'return { "d" : getVariable(context, "d") / millimeter };',
]);
batch.add('isoparametric-curve', 'a curve through a point on a cylinder', [
  can, `opPoint(context, id + "p1", { "point" : ${vec3(0, 10, 12)} });`,
  call('isoparametricCurve', 'curve1', {
    face: faceAt(base, [10, 0, 15]), directionType: 'DirectionType.V_DIRECTION', equalSpacing: 'false',
    uvParamList: `[{ "selectPoint" : true, "point" : qCreatedBy(id + "p1", EntityType.VERTEX) }]`,
  }),
]);
batch.add('fit-spline', 'through three vertices, leaving along x twice as strongly', [
  box('base', [0, 0, 0], [30, 20, 10]), axisLine('dirX', [0, 0, 0], [1, 0, 0]),
  call('fitSpline', 'spline1', {
    vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 10])}, ${vertexAt(base, [30, 20, 0])}])`,
    hasStartDirection: 'true', startDirection: edgeOf('dirX'), startMagnitude: '2',
  }),
]);
batch.add('offset-curve-on-face', 'the top edge of a cylinder offset 2 mm across its top', [
  can,
  call('offsetCurveOnFace', 'offset1', {
    edges: edgeAt(base, [10, 0, 30]), distance: mm(2), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false',
    gapFill: 'GapFill.LINEAR', scope: 'OffsetCurveScope.OFFSET', targets: faceAt(base, [0, 0, 30]),
  }),
]);
batch.add('mate-connector', 'at a face\'s centroid, its normal given as y', [
  block,
  call('mateConnector', 'mc1', {
    originQuery: faceAt(base, [15, 10, 10]), entityInferenceType: 'EntityInferenceType.CENTROID', ownerPart: base,
    specifyNormal: 'true', nx: '0', ny: '1', nz: '0',
  }),
  returns({ frame: `connectorFrame(context, qCreatedBy(id + "mc1", EntityType.BODY))` }),
]);
batch.add('curve-pattern', 'a block patterned along an arc on a cylinder\'s top, locked to the top', [
  cylinder('top', [0, 0], 40, -10, 0), box('base', [-2, -2, 0], [2, 2, 4]),
  sketch('path', PLANES.xy), arc('path', 'arc1', [0, 0], [20, 8.28427], [28.28427, 28.28427]), solve('path'),
  call('curvePattern', 'pattern1', {
    entities: base, edges: wireEdges('path'), spacingType: 'CurvePatternSpacingType.EQUAL', instanceCount: '3',
    orientationType: 'CurvePatternOrientationType.LOCK_FACES', lockedFaces: faceAt(solidsOf('top'), [0, 0, 0]),
  }),
]);

// ----- sheet metal corner breaks -----

const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const counts = returns({ counts: 'bodyCounts(context)' });
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
const corner = `qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), ${vec3(40, 30, 2)})`;
batch.add('sheet-metal-corner-break', 'a plate corner chamfered 3 mm by 6 mm', [
  plate,
  call('sheetMetalCornerBreak', 'break1', {
    blendType: 'EdgeBlendType.CHAMFER', entities: corner, chamferMethod: 'ChamferMethod.FACE_OFFSET', chamferType: 'ChamferType.TWO_OFFSETS',
    width1: mm(3), width2: mm(6), oppositeDirection: 'false',
  }),
  counts,
]);
batch.add('sheet-metal-corner-break', 'a plate corner rounded by a 5 mm conic', [
  plate,
  call('sheetMetalCornerBreak', 'break1', {
    blendType: 'EdgeBlendType.FILLET', entities: corner, blendControlType: 'BlendControlType.RADIUS',
    crossSection: 'FilletCrossSection.CONIC', radius: mm(5), rho: '0.7',
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-020.mjs');
