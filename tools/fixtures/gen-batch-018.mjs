// Generates fixture batch 018: options of features already covered that weren't checked yet. Splitting by a
// sheet, moving a face up to a plane, curve patterns by distance, isoparametric curves at given positions,
// trimming curves and extending them up to an entity, extending a curved sheet, ruled surfaces at an angle, mate
// connectors between entities and realigned, fit splines with end directions, and sheet metal: a counterbored
// hole and a rounded bend unfolded.
//
// Usage: node tools/fixtures/gen-batch-018.mjs
//
// Writes fixtures/batch-018/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, planeBody,
  rect, sketch, solidsOf, solve, vec2, vertexAt,
} from './lib.mjs';

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

// A mate connector's coordinate system, rounded so it compares as text.
function connectorFrame(context is Context, connector is Query) returns map
{
    const cs = evMateConnector(context, { "mateConnector" : connector });
    return { "origin" : roundVector(cs.origin / millimeter), "x" : roundVector(cs.xAxis), "z" : roundVector(cs.zAxis) };
}

function roundVector(v is Vector) returns Vector
{
    return vector(round(v[0] * 1e6) / 1e6, round(v[1] * 1e6) / 1e6, round(v[2] * 1e6) / 1e6);
}
`;

const batch = new Batch({ number: 18, title: 'feature options: split by a sheet, move face up to, curves, surfaces, mate connectors, sheet metal holes and unfolds', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const base = solidsOf('base');
const block = box('base', [0, 0, 0], [30, 20, 10]);
const union = (...qs) => `qUnion([${qs.join(', ')}])`;
const sheetOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
// A surface extruded 20 mm up from a sketch curve on the XY plane.
const surface = (name, curve) => [
  sketch(`${name}Sketch`, PLANES.xy), curve(`${name}Sketch`), solve(`${name}Sketch`),
  call('extrude', name, {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges(`${name}Sketch`), endBound: 'BoundingType.BLIND', depth: mm(20),
  }),
];

// ----- split, move face -----

batch.add('split', 'a block split by a curved sheet', [
  block, sketch('cutSketch', planeAt([0, 0, -5], [0, 0, 1], [1, 0, 0])), arc('cutSketch', 'arc1', [10, -5], [18, 10], [10, 25]), solve('cutSketch'),
  call('extrude', 'cut', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('cutSketch'), endBound: 'BoundingType.BLIND', depth: mm(20),
  }),
  call('splitPart', 'split1', { targets: base, tool: sheetOf('cut') }),
]);
batch.add('move-face', 'a block\'s top translated up to a plane 5 mm above', [
  block, axisLine('dirZ', [0, 0, 0], [0, 0, 1]), planeBody('limit', [0, 0, 15], [0, 0, 1], [1, 0, 0]),
  call('moveFace', 'move1', {
    moveFaces: faceAt(base, [15, 10, 10]), moveFaceType: 'MoveFaceType.TRANSLATE', direction: edgeOf('dirZ'),
    limitType: 'MoveFaceBoundingType.UP_TO_ENTITY', limitEntity: createdBy('limit', 'FACE'), hasOffset: 'false',
  }),
]);

// ----- curves -----

batch.add('curve-pattern', 'a cylinder patterned along an arc every 10 mm', [
  cylinder('base', [0, 0], 3, -3, 3),
  sketch('path', PLANES.xy), arc('path', 'arc1', [0, 0], [20, 8.28427], [28.28427, 28.28427]), solve('path'),
  call('curvePattern', 'pattern1', {
    entities: base, edges: wireEdges('path'), spacingType: 'CurvePatternSpacingType.DISTANCE', distance: mm(10), instanceCount: '3',
  }),
]);
const can = cylinder('base', [0, 0], 10, 0, 30);
batch.add('isoparametric-curve', 'curves at a quarter and two thirds along a cylinder', [
  can,
  call('isoparametricCurve', 'curve1', {
    face: faceAt(base, [10, 0, 15]), directionType: 'DirectionType.U_DIRECTION', equalSpacing: 'false',
    uvParamList: '[{ "selectPoint" : false, "uvValue" : 0.25 }, { "selectPoint" : false, "uvValue" : 0.6666 }]',
  }),
]);
const curve = [sketch('s1', PLANES.xy), line('s1', 'line1', [0, 0], [20, 10]), solve('s1'), call('compositeCurve', 'curve1', { edges: wireEdges('s1') })];
batch.add('trim-curve', 'a curve trimmed by a plane', [
  curve, planeBody('limit', [12, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.TRIM', wires: createdBy('curve1'), trimTo: createdBy('limit', 'FACE'),
    helpPoint: vertexAt(createdBy('curve1'), [20, 10, 0]), flipHeuristics: 'false',
  }),
]);
batch.add('trim-curve', 'a curve extended up to a plane', [
  curve, planeBody('limit', [30, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: createdBy('curve1'), endCondition: 'CurveExtensionEndCondition.UP_TO_ENTITY',
    extendTo: createdBy('limit', 'FACE'), extensionShape: 'CurveExtensionShape.TANGENT', flipHeuristics: 'false',
    helpPoint: vertexAt(createdBy('curve1'), [20, 10, 0]),
  }),
]);
batch.add('fit-spline', 'through three vertices, leaving along x and arriving along y', [
  box('base', [0, 0, 0], [30, 20, 10]), axisLine('dirX', [0, 0, 0], [1, 0, 0]), axisLine('dirY', [0, 0, 0], [0, 1, 0]),
  call('fitSpline', 'spline1', {
    vertices: union(vertexAt(base, [0, 0, 0]), vertexAt(base, [30, 0, 10]), vertexAt(base, [30, 20, 0])),
    hasStartDirection: 'true', startDirection: edgeOf('dirX'), hasEndDirection: 'true', endDirection: edgeOf('dirY'),
  }),
]);

// ----- surfaces -----

batch.add('extend-surface', 'a curved sheet extended 5 mm all round', [
  surface('base', (s) => arc(s, 'arc1', [0, 0], [10, 4], [20, 0])),
  call('extendSurface', 'extend1', { entities: `qOwnedByBody(${sheetOf('base')}, EntityType.EDGE)`, extendDistance: mm(5) }),
]);
batch.add('ruled-surface', 'from an edge, 30 degrees from a direction', [
  block, axisLine('dir', [0, 0, 0], [0, 0, 1]),
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [15, 0, 10]), ruledType: 'RuledSurfaceInterfaceType.ANGLE_FROM_VECTOR', axis: edgeOf('dir'), angle: deg(30),
    distance: mm(8), referenceFaces: faceAt(base, [15, 0, 5]),
  }),
]);

// ----- mate connectors -----

batch.add('mate-connector', 'between the top and bottom of a block', [
  block,
  call('mateConnector', 'mc1', {
    originType: 'OriginCreationType.BETWEEN_ENTITIES', originQuery: faceAt(base, [15, 10, 10]), entityInferenceType: 'EntityInferenceType.CENTROID',
    originAdditionalQuery: faceAt(base, [15, 10, 0]), ownerPart: base,
  }),
  returns({ frame: `connectorFrame(context, qCreatedBy(id + "mc1", EntityType.BODY))` }),
]);
batch.add('mate-connector', 'at a vertex, realigned to two edges', [
  block,
  call('mateConnector', 'mc1', {
    originQuery: vertexAt(base, [30, 20, 10]), entityInferenceType: 'EntityInferenceType.POINT', ownerPart: base,
    realign: 'true', primaryAxisQuery: edgeAt(base, [30, 10, 10]), secondaryAxisQuery: edgeAt(base, [15, 20, 10]),
  }),
  returns({ frame: `connectorFrame(context, qCreatedBy(id + "mc1", EntityType.BODY))` }),
]);

// ----- sheet metal -----

const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const counts = returns({ counts: 'bodyCounts(context)' });
const smFields = { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false' };
batch.add('sheet-metal-hole', 'a counterbored hole through a plate', [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    ...smFields, process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
  sketch('pts1', planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])), `skPoint(pts1, "p1", { "position" : ${vec2(20, 15)} });`, solve('pts1'),
  call('hole', 'hole1', {
    locations: `qOwnedByBody(qBodyType(qCreatedBy(id + "pts1", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`, scope: part,
    style: 'HoleStyle.C_BORE', holeDiameter: mm(4), cBoreDiameter: mm(8), cBoreDepth: mm(1), endStyle: 'HoleEndStyle.THROUGH',
  }),
  counts,
]);
// An L whose corner is a 3 mm sketch arc, extruded as sheet metal, its rounded bend unfolded.
batch.add('sheet-metal-unfold', 'an L with a rounded corner, unfolded', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [27, 0]), arc('s1', 'a1', [27, 0], [29.12132, 0.87868], [30, 3]), line('s1', 'l2', [30, 3], [30, 20]),
  solve('s1'),
  call('sheetMetalStart', 'sm1', {
    ...smFields, process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()',
    endBound: 'SMExtrudeBoundingType.BLIND', oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  call('sheetMetalUnfold', 'unfold1', {
    unfoldAll: 'false', partToUnfold: 'qNothing()',
    entitiesToUnfold: `qNthElement(qGeometry(qOwnedByBody(${part}, EntityType.FACE), GeometryType.CYLINDER), 0)`,
    holdEntity: `atFraction(context, ${part}, EntityType.FACE, 0.5, 0, 0.5)`,
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-018.mjs');
