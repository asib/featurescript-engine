// Generates fixture batch 139: std's last features no batch had run, ruled surfaces' up-to-vertex and other-type
// overrides on a curved edge, and everyday parts.
//
// `dummyFeature` on a block; `jogSectionPartInternal` (a jogged section that clears sheet metal data first) through
// a block and through a sheet metal L. A rounded block's top arc ruled normal to the round, one end's ruling up to a
// point; ruled tangent to the round, one end's ruling normal to it instead. Everyday parts: a bracket (a plate with a
// slot, its corners rounded, holes patterned along it and mirrored), a knob (a revolved profile, filleted and
// chamfered, holes patterned round it), a housing (a box shelled open at its top, a lip round its rim, bosses inside).
//
// Usage: node tools/fixtures/gen-batch-139.mjs
//
// Writes fixtures/batch-139/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, box, call, circle, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, vertexAt } from './lib.mjs';

const helpers = `
// The part's faces: each one's surface type and area.
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

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "sheets" : evaluateQueryCount(context, qBodyType(all, BodyType.SHEET)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({ number: 139, title: 'std\'s last features, ruled surface overrides on a curved edge, everyday parts', helpers });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const jogFields = (target) => ({
  target, sketchPlane: top(0), jogPoints: `[${vec3(25, 35, 10)}, ${vec3(25, 15, 10)}, ${vec3(15, 15, 10)}, ${vec3(15, -5, 10)}]`, isAlignedSection: 'false',
});

// ----- std's last features -----

batch.add('dummy-feature', 'a 40 × 30 × 20 block, then dummyFeature (which does nothing)', [
  box('base', [0, 0, 0], [40, 30, 20]), call('dummyFeature', 'dummy1', {}),
]);
batch.add('section-part', 'a 40 × 30 × 20 block cut by jogSectionPartInternal along a jogged line', [
  box('base', [0, 0, 0], [40, 30, 20]), call('jogSectionPartInternal', 'section1', jogFields(`qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)`)),
]);
batch.add('section-part', 'a sheet metal L (legs 30 and 20, 25 deep, 2 thick) cut by jogSectionPartInternal along a jogged line, its sheet metal data cleared', [
  sketch('s1', top(0)), line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [30, 20]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  call('jogSectionPartInternal', 'section1', {
    target: `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`, sketchPlane: planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]),
    jogPoints: `[${vec3(15, 25, 12)}, ${vec3(15, 10, 12)}, ${vec3(35, 10, 12)}]`, isAlignedSection: 'false',
  }),
  `return { "counts" : bodyCounts(context) };`,
]);

// ----- ruled surfaces' overrides on a curved edge -----

// A 30 × 20 × 10 block, its upright edge at (30, 0) rounded 10: its top edge is a quarter arc about (20, 10, 10).
const rounded = [box('base', [0, 0, 0], [30, 20, 10]), call('fillet', 'round1', { entities: edgeAt(base, [30, 0, 5]), radius: mm(10) })];
const arcEdge = edgeAt(base, [20 + 10 * Math.SQRT1_2, 10 - 10 * Math.SQRT1_2, 10]);
const roundFace = faceAt(base, [20 + 10 * Math.SQRT1_2, 10 - 10 * Math.SQRT1_2, 5]);
const sheets = `qBodyType(qCreatedBy(id + "ruled1", EntityType.BODY), BodyType.SHEET)`;
const ruled = (type, overrides) => [
  call('ruledSurface', 'ruled1', { edges: arcEdge, ruledType: `RuledSurfaceInterfaceType.${type}`, distance: mm(5), referenceFaces: roundFace, vertexOverrides: overrides }),
  `return { "faces" : faceProbe(context, ${sheets}) };`,
];

batch.add('ruled-surface', 'normal to a round from its top arc, 5 wide, the ruling at (30, 10, 10) up to a point at (36, 10, 14)', [
  rounded, call('opPoint', 'p1', { point: vec3(36, 10, 14) }),
  ruled('NORMAL', `[{ "vertex" : ${vertexAt(base, [30, 10, 10])}, "overrideType" : VertexOverrideType.UP_TO_VERTEX, "boundaryOverride" : qCreatedBy(id + "p1", EntityType.VERTEX) }]`),
]);
batch.add('ruled-surface', 'tangent to a round from its top arc, 5 wide, the ruling at (30, 10, 10) normal to it instead', [
  rounded,
  ruled('TANGENT', `[{ "vertex" : ${vertexAt(base, [30, 10, 10])}, "overrideType" : VertexOverrideType.NORMAL, "angleOverride" : ${deg(0)}, "oppositeAngleOverride" : false, "distanceOverride" : ${mm(5)}, "oppositeDirectionOverride" : false }]`),
]);

// ----- everyday parts -----

batch.add('fillet', 'a bracket: a 60 × 30 × 5 plate with a slot (30 × 6) through its middle, its upright corners rounded 5 mm, holes (radius 2) at x = 8 and 52 patterned 3 along y at 7 mm', [
  box('base', [0, 0, 0], [60, 30, 5]),
  sketch('slotSketch', top(5)), line('slotSketch', 'l1', [18, 12], [42, 12]), arc('slotSketch', 'a1', [42, 12], [45, 15], [42, 18]),
  line('slotSketch', 'l2', [42, 18], [18, 18]), arc('slotSketch', 'a2', [18, 18], [15, 15], [18, 12]), solve('slotSketch'),
  call('extrude', 'slot', { entities: regions('slotSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  call('fillet', 'corners', { entities: `qUnion([${[[0, 0], [60, 0], [60, 30], [0, 30]].map(([x, y]) => edgeAt(base, [x, y, 2.5])).join(', ')}])`, radius: mm(5) }),
  sketch('holeSketch', top(5)), circle('holeSketch', 'c1', [8, 8], 2), circle('holeSketch', 'c2', [52, 8], 2), solve('holeSketch'),
  sketch('dir', top(0)), line('dir', 'line1', [0, 0], [0, 10]), solve('dir'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  `linearPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "hole") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "holeSketch"), "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${base} }); } } as FeatureList, "directionOne" : qCreatedBy(id + "dir", EntityType.EDGE), "distance" : 7 * millimeter, "instanceCount" : 3, "oppositeDirection" : false, "isCentered" : false, "hasSecondDir" : false, "fullFeaturePattern" : false });`,
]);
batch.add('revolve', 'a knob: a profile (radius 15 at the base, 12 at the top, 20 tall) revolved, its top edge filleted 3 mm and base chamfered 1 mm, a hole (radius 1.5, 8 deep) at 6 from the axis patterned 4 round it', [
  sketch('profile', PLANES.xz), line('profile', 'l1', [0, 0], [15, 0]), line('profile', 'l2', [15, 0], [12, 20]), line('profile', 'l3', [12, 20], [0, 20]), line('profile', 'l4', [0, 20], [0, 0]), solve('profile'),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [-50, 0], [50, 0]), solve('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [12, 0, 20]), radius: mm(3) }),
  call('chamfer', 'chamfer1', { entities: edgeAt(base, [15, 0, 0]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(1) }),
  sketch('holeSketch', top(20)), circle('holeSketch', 'c1', [6, 0], 1.5), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.BLIND', depth: mm(8), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  `circularPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "hole") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "holeSketch"), "endBound" : BoundingType.BLIND, "depth" : 8 * millimeter, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${base} }); } } as FeatureList, "axis" : qCreatedBy(id + "axis", EntityType.EDGE), "angle" : 360 * degree, "instanceCount" : 4, "equalSpace" : true, "fullFeaturePattern" : false });`,
]);
batch.add('shell', 'a housing: a 50 × 40 × 25 box shelled 2 mm open at its top, a lip 1 mm wide and 2 tall round its rim, two bosses (radius 3, 10 tall) on its floor', [
  box('base', [0, 0, 0], [50, 40, 25]),
  call('shell', 'shell1', { entities: faceAt(base, [25, 20, 25]), thickness: mm(2) }),
  sketch('lipSketch', top(25)), rect('lipSketch', 'outer', [1, 1], [49, 39]), rect('lipSketch', 'inner', [2, 2], [48, 38]), solve('lipSketch'),
  call('extrude', 'lip', { entities: `qSubtraction(${regions('lipSketch')}, qContainsPoint(${regions('lipSketch')}, ${vec3(25, 20, 25)}))`, endBound: 'BoundingType.BLIND', depth: mm(2), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('bossSketch', top(2)), circle('bossSketch', 'c1', [10, 20], 3), circle('bossSketch', 'c2', [40, 20], 3), solve('bossSketch'),
  call('extrude', 'bosses', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(10), operationType: 'NewBodyOperationType.ADD', ...scoped }),
]);

batch.write('tools/fixtures/gen-batch-139.mjs');
