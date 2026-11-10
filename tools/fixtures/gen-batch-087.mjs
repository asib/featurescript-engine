// Generates fixture batch 087: where collinear sketch lines lose their vertex (the sketch's region, an extrude, its
// extracted cap, a thickened sheet metal plate) and the directions of a filleted block's edges and its offset
// faces' edges (batch 086's gaps); a tab on a converted box's wall across its corner, cutting through the next
// wall (std's derip); a cone frustum's side replaced by a coaxial cone; a held partial flange aligned outside with
// rectangular reliefs; planes of a vertical line, a planar spline and pairs of edges; curvatures of a sphere, a
// torus and a cone; rays through a sphere and a torus.
//
// Usage: node tools/fixtures/gen-batch-087.mjs
//
// Writes fixtures/batch-087/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, vec3, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

function dir6(v) returns array
{
    return [roundToPrecision(v[0], 6), roundToPrecision(v[1], 6), roundToPrecision(v[2], 6)];
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

// Some edges: each one's length and three points along it (its ends and middle, in its own parameter).
function edgeList(context is Context, edges is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, edges))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6),
                            "points" : mapArray(lines, function(l) { return round6(l.origin); }) });
    }
    return out;
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

// Where a ray meets some entities: each hit's distance, point and parameter.
function rayProbe(context is Context, arg is map) returns array
{
    return mapArray(evRaycast(context, arg), function(r)
        {
            return { "distance" : roundToPrecision(r.distance / millimeter, 6), "point" : round6(r.intersection),
                     "parameter" : [roundToPrecision(r.parameter[0], 6), roundToPrecision(r.parameter[1], 6)] };
        });
}

// A face's curvatures at some parameters, with the points they are at.
function curvatureProbe(context is Context, face is Query, parameters is array) returns array
{
    const planes = evFaceTangentPlanes(context, { "face" : face, "parameters" : parameters });
    var out = [];
    for (var i, c in evFaceCurvatures(context, { "face" : face, "parameters" : parameters }))
    {
        out = append(out, { "at" : round6(planes[i].origin), "min" : roundToPrecision(c.minCurvature * millimeter, 6),
                            "max" : roundToPrecision(c.maxCurvature * millimeter, 6), "minDirection" : dir6(c.minDirection),
                            "maxDirection" : dir6(c.maxDirection) });
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

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "sheets" : evaluateQueryCount(context, qBodyType(all, BodyType.SHEET)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

// A sheet metal part's definition sheet: its edges, and the joints on them.
function definitionProbe(context is Context, part is Query) returns map
{
    const definition = qOwnerBody(qUnion(getSMDefinitionEntities(context, qOwnedByBody(part, EntityType.FACE))));
    var joints = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(definition, EntityType.EDGE)))
    {
        const joint = try silent(getJointAttribute(context, edge));
        if (joint != undefined)
            joints = append(joints, { "type" : joint.jointType.value, "edge" : edgeList(context, edge)[0] });
    }
    return { "edges" : edgeList(context, qOwnedByBody(definition, EntityType.EDGE)), "joints" : joints };
}
`;

const batch = new Batch({
  number: 87,
  title: 'collinear sketch lines, edge directions of offset faces, a tab across a corner, a cone replaced by a cone, a held flange, planes, curvatures and rays',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const base = solidsOf('base');
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);

// ----- collinear sketch lines -----

// A 40 × 30 rectangle whose long sides are each two lines meeting at x = 20.
const sixLines = (s) => [
  sketch(s, xy), line(s, 'l1', [0, 0], [20, 0]), line(s, 'l2', [20, 0], [40, 0]), line(s, 'l3', [40, 0], [40, 30]), line(s, 'l4', [40, 30], [20, 30]),
  line(s, 'l5', [20, 30], [0, 30]), line(s, 'l6', [0, 30], [0, 0]), solve(s),
];
batch.add('extrude', 'a 40 × 30 rectangle drawn as six lines (its long sides two collinear lines each): the region\'s edges, the region extruded 5 mm, the extrude\'s start cap extracted', [
  sixLines('s1'),
  call('extrude', 'ext', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(5), oppositeDirection: 'false' }),
  `opExtractSurface(context, id + "cap", { "faces" : qCapEntity(id + "ext", CapType.START, EntityType.FACE) });`,
  returns({
    regionEdges: `edgeList(context, qAdjacent(qSketchRegion(id + "s1"), AdjacencyType.EDGE, EntityType.EDGE))`,
    extruded: `faceProbe(context, ${solidsOf('ext')})`,
    capEdges: `edgeList(context, qOwnedByBody(${sheetsOf('cap')}, EntityType.EDGE))`,
  }),
]);
batch.add('sheet-metal-start', 'the same six lines\' region thickened 2 mm into sheet metal: the definition sheet\'s edges and the part\'s faces', [
  sixLines('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`,
    tangentPropagation: 'false', bends: 'qNothing()',
  }),
  returns({ definition: `definitionProbe(context, ${solidsOf('sm1')})`, faces: `faceProbe(context, ${solidsOf('sm1')})` }),
]);

// ----- edge directions -----

batch.add('offset-surface', 'a 40 × 30 × 20 block with its top back edge filleted 5 mm: the block\'s edges, and its top offset 2 mm out alone', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 30, 20]), radius: mm(5) }),
  call('offsetSurface', 'offset1', { surfacesAndFaces: faceAt(base, [20, 10, 20]), offset: mm(2), oppositeDirection: 'false' }),
  returns({
    block: `edgeList(context, qOwnedByBody(${base}, EntityType.EDGE))`,
    offset: `edgeList(context, qOwnedByBody(${sheetsOf('offset1')}, EntityType.EDGE))`,
  }),
]);

// ----- a tab across a corner -----

// A 40 × 30 × 20 block converted to an open box, 1 mm thick, bent along the bottom (rips up the corners); a tab
// 15 × 10 on the front wall's plane from x = 30 to 45 (past the corner at x = 40), cutting through the right wall
// with 0.2 mm clearance.
const part = solidsOf('sm1');
batch.add('sheet-metal-tab', 'an open box converted from a 40 × 30 × 20 block, 1 mm thick: a tab on its front wall running 5 mm past the corner, subtracted from the right wall with 0.2 mm clearance', [
  box('block', [0, 0, 0], [40, 30, 20]),
  call('sheetMetalStart', 'sm1', {
    process: 'SMProcessType.CONVERT', partToConvert: solidsOf('block'), facesToExclude: faceAt(solidsOf('block'), [20, 15, 20]),
    bends: `qUnion([${[[20, 0, 0], [40, 15, 0], [20, 30, 0], [0, 15, 0]].map((p) => `qContainsPoint(qOwnedByBody(${solidsOf('block')}, EntityType.EDGE), ${vec3(...p)})`).join(', ')}])`,
    clearance: mm(0), keepInputParts: 'false', thickness: mm(1), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
  }),
  sketch('t1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), rect('t1', 'r1', [30, 5], [45, 15]), solve('t1'),
  call('sheetMetalTab', 'tab1', {
    tabFaces: `qSketchRegion(id + "t1")`, booleanUnionScope: `qContainsPoint(qOwnedByBody(${part}, EntityType.FACE), ${vec3(20, 0, 10)})`, booleanOffset: mm(0.2),
    booleanSubtractScope: `qContainsPoint(qOwnedByBody(${part}, EntityType.FACE), ${vec3(40, 15, 10)})`,
  }),
  returns({ counts: 'bodyCounts(context)', faces: `faceProbe(context, ${part})` }),
]);

// ----- a cone replaced by a cone -----

// A cone frustum (radius 10 at z = 0 to 6 at z = 20); a coaxial conical sheet revolved from the line (12, 0)-(4, 20).
batch.add('replace-face', 'a cone frustum (radius 10 to 6, 20 tall): its side replaced by a coaxial cone from radius 12 at z = 0 to 4 at z = 20', [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [6, 20]), line('s1', 'l3', [6, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('s1'), axis: edgeOf('axis'), fullRevolve: 'true' }),
  sketch('s2', PLANES.xz), line('s2', 'l1', [13, -5], [3, 25]), solve('s2'),
  call('revolve', 'tool', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), axis: edgeOf('axis'), fullRevolve: 'true' }),
  call('replaceFace', 'replace1', {
    replaceFaces: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`, templateFace: `qOwnedByBody(${sheetsOf('tool')}, EntityType.FACE)`,
    oppositeSense: 'false', offset: mm(0),
  }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

// ----- a held partial flange, aligned outside -----

batch.add('sheet-metal-flange', 'a 40 × 30 plate: a partial flange on its right edge, 8 mm in from one end and 4 from the other, aligned outside, rectangular reliefs, holding the adjacent edges', [
  sketch('s1', xy), rect('s1', 'r1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`,
    tangentPropagation: 'false', bends: 'qNothing()', defaultBendReliefStyle: 'SMBendStrategyType.RECTANGLE',
  }),
  call('sheetMetalFlange', 'flange1', {
    edges: `atFraction(context, ${part}, EntityType.EDGE, 1, 0.5, 1)`, flangeAlignment: 'SMFlangeAlignment.OUTER', limitType: 'SMFlangeBoundingType.BLIND',
    distance: mm(12), angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', isPartialFlange: 'true', chainType: 'SMPartialFlangeChainType.PER_EDGE', flipFlangeBounds: 'false', holdAdjacentEdges: 'true',
    firstBoundType: 'SMFlangeBoundingType.BLIND', firstBoundOffset: mm(8), hasSecondBound: 'true', secondBoundType: 'SMFlangeBoundingType.BLIND', secondBoundOffset: mm(4),
  }),
  returns({ counts: 'bodyCounts(context)', faces: `faceProbe(context, ${part})` }),
]);

// ----- planes, curvatures and rays -----

batch.add('evaluate', 'the planes of a block\'s upright edge, a planar spline, two parallel lines and a line with an arc', [
  box('base', [0, 0, 0], [40, 30, 20]),
  sketch('s1', planeAt([0, 0, 30], [0, 0, 1], [1, 0, 0])), `skFitSpline(s1, "spline1", { "points" : [${vec2(0, 0)}, ${vec2(10, 6)}, ${vec2(20, -3)}, ${vec2(30, 4)}] });`,
  line('s1', 'l1', [0, 20], [30, 20]), line('s1', 'l2', [0, 25], [30, 25]), line('s1', 'l3', [40, 0], [50, 0]), arc('s1', 'a1', [50, 0], [55, 5], [60, 0]), solve('s1'),
  returns({
    upright: `planarProbe(context, ${edgeAt(base, [40, 30, 10])}, false)`,
    spline: `planarProbe(context, sketchEntityQuery(id + "s1", EntityType.EDGE, "spline1"), false)`,
    parallel: `planarProbe(context, qUnion([sketchEntityQuery(id + "s1", EntityType.EDGE, "l1"), sketchEntityQuery(id + "s1", EntityType.EDGE, "l2")]), true)`,
    lineAndArc: `planarProbe(context, qUnion([sketchEntityQuery(id + "s1", EntityType.EDGE, "l3"), sketchEntityQuery(id + "s1", EntityType.EDGE, "a1")]), true)`,
  }),
]);
const sphere = call('fSphere', 'base', { radius: mm(10) });
const torus = [
  sketch('p1', PLANES.xz), circle('p1', 'c1', [20, 0], 5), solve('p1'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('p1'), axis: edgeOf('axis'), fullRevolve: 'true' }),
];
const params = '[vector(0.25, 0.25), vector(0.5, 0.5), vector(0.1, 0.7)]';
batch.add('evaluate', 'a sphere of radius 10: its curvatures at three parameters; rays along x through it, from the origin and from outside', [
  sphere,
  returns({
    curvatures: `curvatureProbe(context, qOwnedByBody(${base}, EntityType.FACE), ${params})`,
    through: `rayProbe(context, { "entities" : ${base}, "ray" : line(${vec3(-20, 1, 2)}, vector(1, 0, 0)), "closest" : false })`,
    inside: `rayProbe(context, { "entities" : ${base}, "ray" : line(${vec3(0, 0, 0)}, vector(0, 0.6, 0.8)), "closest" : false })`,
  }),
]);
batch.add('evaluate', 'a full torus (radii 20 and 5) about z: its curvatures at three parameters; a ray along x through it', [
  torus,
  returns({
    curvatures: `curvatureProbe(context, qOwnedByBody(${base}, EntityType.FACE), ${params})`,
    through: `rayProbe(context, { "entities" : ${base}, "ray" : line(${vec3(-30, 0, 1)}, vector(1, 0, 0)), "closest" : false })`,
  }),
]);
batch.add('evaluate', 'a cone frustum (radius 10 to 6, 20 tall): its side\'s curvatures at three parameters; a ray across it', [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [6, 20]), line('s1', 'l3', [6, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('s1'), axis: edgeOf('axis'), fullRevolve: 'true' }),
  returns({
    curvatures: `curvatureProbe(context, qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE), ${params})`,
    across: `rayProbe(context, { "entities" : ${base}, "ray" : line(${vec3(-20, 0, 10)}, vector(1, 0, 0)), "closest" : false })`,
  }),
]);

batch.write('tools/fixtures/gen-batch-087.mjs');
