// Generates fixture batch 086: shells of a block with rounded corners, a hollow sphere and an open cone frustum
// (outwards); offset surfaces of a top and its fillet together and of a sphere inwards; a planar sheet and a
// cylindrical sheet through it trimmed to each other, both ways; helices round a cone, along a circle, by turns and
// pitch, and from a start point; bridging curves with flow (G3) matches and by control points; a round tab and a tab
// on a flange; a plate ripped across between two vertices along its edges; extrudes with a start offset (blind, from
// a face, from a circular edge); a spline flattened onto its best-fit plane, its ends free and locked; the planes of
// edges, rays cast through a part, a fillet's curvatures and radius.
//
// Usage: node tools/fixtures/gen-batch-086.mjs
//
// Writes fixtures/batch-086/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each face of a body: its area and its surface's type.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType;
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}

// Each edge of some bodies: its length and three points along it (its ends and middle, in its own parameter).
function edgePoints(context is Context, bodies is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(bodies, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6),
                            "points" : mapArray(lines, function(l) { return round6(l.origin); }) });
    }
    return out;
}

// The point at fractions (fx, fy, fz) of a part's bounding box.
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

// An edge, face or vertex of a part through a point at fractions of its bounding box.
function atFraction(context is Context, part is Query, entityType is EntityType, fx is number, fy is number, fz is number) returns Query
{
    return qContainsPoint(qOwnedByBody(part, entityType), pointAtFraction(context, part, fx, fy, fz));
}

function dir6(v) returns array
{
    return [roundToPrecision(v[0], 6), roundToPrecision(v[1], 6), roundToPrecision(v[2], 6)];
}

function planeProbe(p) returns map
{
    return { "origin" : round6(p.origin), "normal" : dir6(p.normal), "x" : dir6(p.x) };
}

// A plane from evPlanarEdge or evPlanarEdges, or "failed".
function planarProbe(context is Context, edges is Query, several is boolean)
{
    try silent
    {
        return planeProbe(several ? evPlanarEdges(context, { "edges" : edges }) : evPlanarEdge(context, { "edge" : edges }));
    }
    return "failed";
}

// Where a ray meets some entities: each hit's entity type, distance, point and parameter.
function rayProbe(context is Context, arg is map) returns array
{
    return mapArray(evRaycast(context, arg), function(r)
        {
            return { "type" : r.entityType, "distance" : roundToPrecision(r.distance / millimeter, 6), "point" : round6(r.intersection),
                     "parameter" : r.parameter is number ? roundToPrecision(r.parameter, 6) : [roundToPrecision(r.parameter[0], 6), roundToPrecision(r.parameter[1], 6)] };
        });
}

// A B-spline curve's degree, poles, weights and knots.
function curveProbe(context is Context, edge is Query) returns map
{
    const c = evCurveDefinition(context, { "edge" : edge });
    if (!(c is BSplineCurve))
        return { "bspline" : false };
    return { "degree" : c.degree, "rational" : c.isRational, "periodic" : c.isPeriodic, "points" : mapArray(c.controlPoints, function(p) { return round6(p); }),
             "knots" : mapArray(c.knots, function(k) { return roundToPrecision(k, 9); }) };
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "sheets" : evaluateQueryCount(context, qBodyType(all, BodyType.SHEET)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({
  number: 86,
  title: 'shells, offset surfaces, a mutual trim of a plane and a cylinder, helices, bridging curves, sheet metal tabs and a rip',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const base = solidsOf('base');
const wires = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE)`;

// A sphere of radius 10 about the origin; a cone frustum of radius 10 at z = 0 to 6 at z = 20 (a revolved trapezoid).
const sphere = call('fSphere', 'base', { radius: mm(10) });
const frustum = [
  sketch('f1', PLANES.xz), line('f1', 'l1', [0, 0], [10, 0]), line('f1', 'l2', [10, 0], [6, 20]), line('f1', 'l3', [6, 20], [0, 20]), line('f1', 'l4', [0, 20], [0, 0]), solve('f1'),
  axisLine('faxis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('f1'), axis: edgeOf('faxis'), fullRevolve: 'true' }),
];

// ----- shells -----

batch.add('shell', 'a 40 × 30 × 20 block with its upright edges filleted 5 mm: its top removed, shelled 2 mm inwards', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: `qUnion([${[[0, 0], [40, 0], [40, 30], [0, 30]].map(([x, y]) => edgeAt(base, [x, y, 10])).join(', ')}])`, radius: mm(5) }),
  call('shell', 'shell1', { entities: faceAt(base, [20, 15, 20]), thickness: mm(2), oppositeDirection: 'false' }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);
batch.add('shell', 'a sphere of radius 10 hollowed, 2 mm', [
  sphere, call('shell', 'shell1', { isHollow: 'true', parts: base, thickness: mm(2), oppositeDirection: 'false' }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);
batch.add('shell', 'a cone frustum (radius 10 to 6, 20 tall): its top and bottom removed, shelled 1.5 mm outwards', [
  frustum,
  call('shell', 'shell1', { entities: `qUnion([${faceAt(base, [0, 0, 0])}, ${faceAt(base, [0, 0, 20])}])`, thickness: mm(1.5), oppositeDirection: 'true' }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

// ----- offset surfaces -----

batch.add('offset-surface', 'a 40 × 30 × 20 block with its top back edge filleted 5 mm: the top and the fillet offset 2 mm outwards together', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 30, 20]), radius: mm(5) }),
  call('offsetSurface', 'offset1', {
    surfacesAndFaces: `qUnion([${faceAt(base, [20, 10, 20])}, qCreatedBy(id + "fillet1", EntityType.FACE)])`, offset: mm(2), oppositeDirection: 'false',
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('offset1')})`, edges: `edgePoints(context, ${sheetsOf('offset1')})` }),
]);
batch.add('offset-surface', 'a sphere of radius 10: its face offset 3 mm inwards', [
  sphere, call('offsetSurface', 'offset1', { surfacesAndFaces: faceAt(base, [10, 0, 0]), offset: mm(3), oppositeDirection: 'true' }),
  returns({ faces: `faceProbe(context, ${sheetsOf('offset1')})` }),
]);

// ----- a plane and a cylinder trimmed to each other -----

// A level sheet z = 0 (x and y -20..20) and an upright cylindrical sheet of radius 10 about z, z -10..10.
const planeAndTube = [
  sketch('p1', PLANES.xz), line('p1', 'l1', [-20, 0], [20, 0]), solve('p1'),
  call('extrude', 'plane', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('p1'), endBound: 'BoundingType.BLIND', depth: mm(40), symmetric: 'true' }),
  sketch('t1', PLANES.xy), circle('t1', 'c1', [0, 0], 10), solve('t1'),
  call('extrude', 'tube', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('t1'), endBound: 'BoundingType.BLIND', depth: mm(20), symmetric: 'true' }),
];
const trimProbe = returns({ faces: `faceProbe(context, qUnion([${sheetsOf('plane')}, ${sheetsOf('tube')}]))`, counts: 'bodyCounts(context)' });
batch.add('mutual-trim', 'a level sheet and an upright cylindrical sheet through it, trimmed to each other and merged', [
  planeAndTube,
  call('mutualTrim', 'trim1', { body1: sheetsOf('plane'), keepOtherSide1: 'false', body2: sheetsOf('tube'), keepOtherSide2: 'false', merge: 'true' }),
  trimProbe,
]);
batch.add('mutual-trim', 'the same, keeping the other sides of both, not merged', [
  planeAndTube,
  call('mutualTrim', 'trim1', { body1: sheetsOf('plane'), keepOtherSide1: 'true', body2: sheetsOf('tube'), keepOtherSide2: 'true', merge: 'false' }),
  trimProbe,
]);

// ----- helices -----

const helixProbe = returns({ edges: `edgePoints(context, ${wires('helix1')})` });
const helix = (fields) => call('helix', 'helix1', { startType: 'StartType.START_ANGLE', startAngle: deg(0), handedness: 'Direction.CCW', oppositeDirection: 'false', ...fields });
batch.add('helix', 'round a cone frustum\'s side (radius 10 to 6, 20 tall), 3 turns', [
  frustum, helix({ axisType: 'AxisType.SURFACE', entities: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`, pathType: 'PathType.TURNS', revolutions: '3' }),
  helixProbe,
]);
batch.add('helix', 'along a circle of radius 8 in the xy plane, 4 turns, 20 high, clockwise', [
  sketch('s1', PLANES.xy), circle('s1', 'c1', [0, 0], 8), solve('s1'),
  helix({
    axisType: 'AxisType.CIRCLE', edge: wireEdges('s1'), pathType: 'PathType.TURNS', revolutions: '4', endType: 'EndType.HEIGHT', height: mm(20),
    handedness: 'Direction.CW',
  }),
  helixProbe,
]);
batch.add('helix', 'about an axis, 3 turns of 4 mm pitch, radius 6, starting at 30°', [
  axisLine('axis1', [0, 0, 0], [0, 0, 20]),
  helix({ axisType: 'AxisType.AXIS', axis: edgeOf('axis1'), pathType: 'PathType.TURNS_PITCH', revolutions: '3', helicalPitch: mm(4), startAngle: deg(30), startRadius: mm(6) }),
  helixProbe,
]);
batch.add('helix', 'about an axis from a start point at (8, 0, 0), 2 turns, 15 high', [
  axisLine('axis1', [0, 0, 0], [0, 0, 20]), `opPoint(context, id + "start", { "point" : ${vec3(8, 0, 0)} });`,
  helix({
    axisType: 'AxisType.AXIS', axis: edgeOf('axis1'), pathType: 'PathType.TURNS', revolutions: '2', startType: 'StartType.START_POINT',
    startPoint: `qCreatedBy(id + "start", EntityType.VERTEX)`, endType: 'EndType.HEIGHT', height: mm(15),
  }),
  helixProbe,
]);

// ----- bridging curves -----

const bridgeProbe = returns({ edges: `edgePoints(context, ${wires('bridge1')})` });
const sketchEdge = (s, name) => `sketchEntityQuery(id + "${s}", EntityType.EDGE, "${name}")`;
const lineAndArc = [sketch('s1', PLANES.xy), line('s1', 'l1', [0, 0], [20, 0]), arc('s1', 'a1', [40, 10], [50, 20], [60, 10]), solve('s1')];
batch.add('bridging-curve', 'a line and an arc bridged with flow (G3) matches at both ends', [
  lineAndArc,
  call('bridgingCurve', 'bridge1', {
    side1: sketchEdge('s1', 'l1'), match1: 'BridgingCurveMatchType.G3', flip1: 'false', side2: sketchEdge('s1', 'a1'), match2: 'BridgingCurveMatchType.G3', flip2: 'false',
    editControlPoints: 'false', magnitude: '1', bias: '0.5',
  }),
  bridgeProbe,
]);
batch.add('bridging-curve', 'a line and an arc bridged by control points, tangent to the line and matching the arc\'s curvature, magnitudes 1.5 and 0.8', [
  lineAndArc,
  call('bridgingCurve', 'bridge1', {
    side1: sketchEdge('s1', 'l1'), match1: 'BridgingCurveMatchType.TANGENCY', flip1: 'false', side2: sketchEdge('s1', 'a1'), match2: 'BridgingCurveMatchType.CURVATURE', flip2: 'false',
    method: 'BridgingCurveMethod.CONTROL_POINTS', editControlPoints: 'true', side1Magnitude: '1.5', side2Magnitude: '0.8', side2CurvatureOffset: '0.3',
    magnitude: '1', bias: '0.5',
  }),
  bridgeProbe,
]);

// ----- sheet metal -----

const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = solidsOf('sm1');
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const counts = returns({ counts: 'bodyCounts(context)', faces: `faceProbe(context, ${part})` });
const start = (fields) =>
  call('sheetMetalStart', 'sm1', { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', ...fields });
const thicken = start({ process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()' });
const plate = [sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'), thicken];

batch.add('sheet-metal-tab', 'a circle of radius 8 about the middle of a 40 × 30 plate\'s right edge, merged into it as a tab', [
  plate, sketch('t1', planeAt([0, 0, 2], [0, 0, 1], [1, 0, 0])), circle('t1', 'c1', [40, 15], 8), solve('t1'),
  call('sheetMetalTab', 'tab1', { tabFaces: `qSketchRegion(id + "t1")`, booleanUnionScope: at('FACE', 0.5, 0.5, 1), booleanOffset: mm(0) }),
  counts,
]);
batch.add('sheet-metal-tab', 'a plate with a 15 mm flange on its right edge: a 10 × 8 tab on the flange\'s top edge', [
  plate,
  call('sheetMetalFlange', 'flange1', {
    edges: at('EDGE', 1, 0.5, 1), flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
  }),
  sketch('t1', planeAt([40, 0, 0], [1, 0, 0], [0, 1, 0])), rect('t1', 'r1', [10, 14], [20, 22]), solve('t1'),
  call('sheetMetalTab', 'tab1', { tabFaces: `qSketchRegion(id + "t1")`, booleanUnionScope: at('FACE', 1, 0.5, 0.8), booleanOffset: mm(0) }),
  counts,
]);
batch.add('sheet-metal-flange', 'a 40 × 30 plate: a partial flange on its right edge, 5 mm in from each end, holding the adjacent edges (the default)', [
  plate,
  call('sheetMetalFlange', 'flange1', {
    edges: at('EDGE', 1, 0.5, 1), flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
    isPartialFlange: 'true', chainType: 'SMPartialFlangeChainType.PER_EDGE', flipFlangeBounds: 'false', holdAdjacentEdges: 'true',
    firstBoundType: 'SMFlangeBoundingType.BLIND', firstBoundOffset: mm(5), hasSecondBound: 'true', secondBoundType: 'SMFlangeBoundingType.BLIND', secondBoundOffset: mm(5),
  }),
  counts,
]);
batch.add('sheet-metal-rip', 'a 40 × 30 plate whose long edges are each two lines meeting at x = 20, ripped across between those vertices (default gap)', [
  sketch('s1', xy), line('s1', 'l1', [0, 0], [20, 0]), line('s1', 'l2', [20, 0], [40, 0]), line('s1', 'l3', [40, 0], [40, 30]), line('s1', 'l4', [40, 30], [20, 30]),
  line('s1', 'l5', [20, 30], [0, 30]), line('s1', 'l6', [0, 30], [0, 0]), solve('s1'), thicken,
  call('sheetMetalRip', 'rip1', {
    vertices: `qUnion([qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), ${vec3(20, 0, 2)}), qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), ${vec3(20, 30, 2)})])`,
    useDefaultGap: 'true',
  }),
  counts,
]);

// ----- extrudes with a start offset -----

const square = [sketch('e1', PLANES.xy), rect('e1', 'r1', [0, 0], [20, 10]), solve('e1')];
const offsetExtrude = (fields) =>
  call('extrude', 'ext1', { entities: regions('e1'), endBound: 'BoundingType.BLIND', depth: mm(10), oppositeDirection: 'false', startOffset: 'true', ...fields });
const extProbe = returns({ faces: `faceProbe(context, ${solidsOf('ext1')})` });
batch.add('extrude', 'a 20 × 10 rectangle on the xy plane extruded 10 mm, starting 5 mm below it', [
  square, offsetExtrude({ startOffsetBound: 'StartOffsetType.BLIND', startOffsetDistance: mm(5), startOffsetOppositeDirection: 'true' }), extProbe,
]);
batch.add('extrude', 'the same, starting from the top of a separate block 8 mm high', [
  box('ref', [-10, -10, 0], [30, 20, 8]), square,
  offsetExtrude({ startOffsetBound: 'StartOffsetType.ENTITY', startOffsetEntity: faceAt(solidsOf('ref'), [0, 0, 8]) }), extProbe,
]);
batch.add('extrude', 'the same, starting from the top edge of a cylinder (radius 5 about (40, 0), 12 tall)', [
  cylinder('ref', [40, 0], 5, 0, 12), square,
  offsetExtrude({ startOffsetBound: 'StartOffsetType.ENTITY', startOffsetEntity: edgeAt(solidsOf('ref'), [45, 0, 12]) }), extProbe,
]);

// ----- a spline flattened onto its best-fit plane -----

const editCurve = (fields) =>
  call('editCurve', 'edit1', {
    wire: `qCreatedBy(id + "spline1", EntityType.BODY)`, approximate: 'false', elevate: 'false', planarize: 'true', planeReference: 'PlaneReference.BEST',
    editControlPoints: 'false', showDetails: 'false', approximationShowDeviation: 'false', ...fields,
  });
const spaceSpline = call('opFitSpline', 'spline1', { points: `[${vec3(0, 0, 0)}, ${vec3(10, 5, 3)}, ${vec3(20, 0, -2)}, ${vec3(30, 8, 4)}]` });
const editProbe = returns({ curve: `curveProbe(context, qOwnedByBody(${wires('spline1')}, EntityType.EDGE))` });
batch.add('edit-curve', 'a spline through four points in space flattened onto its best-fit plane', [spaceSpline, editCurve({ lockEnds: 'false' }), editProbe]);
batch.add('edit-curve', 'the same, its ends locked', [spaceSpline, editCurve({ lockEnds: 'true' }), editProbe]);

// ----- evaluations -----

// A 40 × 30 × 20 block, its top front edge filleted 5 mm, a hole of radius 4 down through it at (20, 15).
const filletFace = `qCreatedBy(id + "fillet1", EntityType.FACE)`;
batch.add('evaluate', 'the planes of a fillet\'s arc, a hole\'s circle, a top face\'s loop and a straight edge; rays through the part; the fillet\'s curvatures and radius', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 0, 20]), radius: mm(5) }),
  sketch('h1', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('h1', 'c1', [20, 15], 4), solve('h1'),
  call('extrude', 'hole', { entities: regions('h1'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base }),
  returns({
    arc: `planarProbe(context, ${edgeAt(base, [0, 1.4644661, 18.5355339])}, false)`,
    circle: `planarProbe(context, ${edgeAt(base, [24, 15, 20])}, false)`,
    loop: `planarProbe(context, qAdjacent(${faceAt(base, [20, 25, 20])}, AdjacencyType.EDGE, EntityType.EDGE), true)`,
    line: `planarProbe(context, ${edgeAt(base, [40, 15, 0])}, false)`,
    rays: `rayProbe(context, { "entities" : ${base}, "ray" : line(${vec3(20, -10, 10)}, vector(0, 1, 0)), "closest" : false })`,
    closest: `rayProbe(context, { "entities" : ${base}, "ray" : line(${vec3(20, -10, 10)}, vector(0, 1, 0)) })`,
    behind: `rayProbe(context, { "entities" : ${base}, "ray" : line(${vec3(20, 15, 10)}, vector(0, 1, 0)), "closest" : false, "includeIntersectionsBehind" : true })`,
    curvatures: `mapArray(evFaceCurvatures(context, { "face" : ${filletFace}, "parameters" : [vector(0, 0), vector(0.5, 0.5), vector(1, 1)] }), function(c) { return { "min" : roundToPrecision(c.minCurvature * millimeter, 6), "max" : roundToPrecision(c.maxCurvature * millimeter, 6), "minDirection" : dir6(c.minDirection), "maxDirection" : dir6(c.maxDirection) }; })`,
    radius: `roundToPrecision(evFilletRadius(context, { "face" : ${filletFace} }) / millimeter, 6)`,
  }),
]);

batch.write('tools/fixtures/gen-batch-086.mjs');
