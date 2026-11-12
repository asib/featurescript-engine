// Generates fixture batch 135: which face an uneven fillet of a circle measures `radius` from (batch 132 c009: a
// cone frustum's top from the top, where batch 082's cylinder's top measures it from the cylinder), and sketches
// whose constraints move their entities (the engine had solved only those std's own features use).
//
// Uneven fillets, 1 and 2 mm, of: an extruded cylinder's top and bottom edges; a cone frustum's (fCone, a revolve)
// top and bottom edges, standing alone, and upside down; a cylinder revolved from a rectangle; a cylinder extruded
// down from its top; a hole's rim through a block; a boss's foot on a plate; a drafted extrude's top. Each returns
// its part's faces' types and areas, which give each side's setback.
//
// Sketches drawn off and pulled into place by constraints, each extruded 1 mm: a rectangle by lengths, by distances,
// a square by equal sides, a circle by radius and concentric to a fixed point, by diameter with its centre on one, a
// line at an angle to a fixed one, a slot of tangent arcs, a point at a line's midpoint, a line mirrored, a 30-40-50
// triangle. Each returns its sketch's edges' ends.
//
// Usage: node tools/fixtures/gen-batch-135.mjs
//
// Writes fixtures/batch-135/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, cylinder, edgeAt, mm, planeAt, regions, sketch, solidsOf, solve, vec2, vec3 } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

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

// A sketch's curves: each edge's ends, in the order the sketch made them.
function sketchProbe(context is Context, sketchId is Id) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(qBodyType(qCreatedBy(sketchId, EntityType.BODY), BodyType.WIRE), EntityType.EDGE)))
    {
        const ends = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1] });
        out = append(out, [round6(ends[0].origin), round6(ends[1].origin), round6(ends[2].origin)]);
    }
    return out;
}
`;

const batch = new Batch({ number: 135, title: 'uneven fillets of circles; sketches solved by their constraints', helpers });
const base = solidsOf('base');
const uneven = { radius: mm(1), isAsymmetric: 'true', otherRadius: mm(2), flipAsymmetric: 'false' };
const filleted = (edge) => [
  call('fillet', 'fillet1', { entities: edge, ...uneven }),
  `return { "faces" : faceProbe(context, ${base}) };`,
];
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const fCone = (b, t, rb, rt) => call('fCone', 'base', { bottomCenter: vec3(...b), topCenter: vec3(...t), bottomRadius: mm(rb), topRadius: mm(rt) });

// ----- uneven fillets of circles -----

batch.add('fillet', 'an extruded cylinder (radius 10, 20 tall): its top edge filleted unevenly, 1 and 2 mm', [
  cylinder('base', [0, 0], 10, 0, 20), filleted(edgeAt(base, [10, 0, 20])),
]);
batch.add('fillet', 'an extruded cylinder (radius 10, 20 tall): its bottom edge filleted unevenly, 1 and 2 mm', [
  cylinder('base', [0, 0], 10, 0, 20), filleted(edgeAt(base, [10, 0, 0])),
]);
batch.add('fillet', 'a cone frustum (fCone, radius 8 to 4, 10 tall) alone: its top edge filleted unevenly, 1 and 2 mm', [
  fCone([0, 0, 0], [0, 0, 10], 8, 4), filleted(edgeAt(base, [4, 0, 10])),
]);
batch.add('fillet', 'a cone frustum (fCone, radius 8 to 4, 10 tall) alone: its bottom edge filleted unevenly, 1 and 2 mm', [
  fCone([0, 0, 0], [0, 0, 10], 8, 4), filleted(edgeAt(base, [8, 0, 0])),
]);
batch.add('fillet', 'a cone frustum (fCone, radius 4 to 8, 10 tall), upside down: its top edge filleted unevenly, 1 and 2 mm', [
  fCone([0, 0, 0], [0, 0, 10], 4, 8), filleted(edgeAt(base, [8, 0, 10])),
]);
batch.add('fillet', 'a cylinder revolved from a 10 × 20 rectangle about z: its top edge filleted unevenly, 1 and 2 mm', [
  sketch('profile', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])),
  `skRectangle(profile, "rect1", { "firstCorner" : ${vec2(0, 0)}, "secondCorner" : ${vec2(10, 20)} });`,
  solve('profile'),
  call('revolve', 'base', {
    entities: regions('profile'), axis: `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "profile", EntityType.BODY), BodyType.WIRE), EntityType.EDGE), ${vec3(0, 0, 10)})`
  }),
  filleted(edgeAt(base, [10, 0, 20])),
]);
batch.add('fillet', 'a cylinder (radius 10) extruded 20 down from a circle at z = 20: its top edge filleted unevenly, 1 and 2 mm', [
  sketch('baseSketch', top(20)), circle('baseSketch', 'circle1', [0, 0], 10), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
  filleted(edgeAt(base, [10, 0, 20])),
]);
batch.add('fillet', 'a 30 × 30 × 10 block with a hole (radius 5) through its middle: the hole\'s top rim filleted unevenly, 1 and 2 mm', [
  box('base', [-15, -15, 0], [15, 15, 10]),
  sketch('holeSketch', top(10)), circle('holeSketch', 'circle1', [0, 0], 5), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base }),
  filleted(edgeAt(base, [5, 0, 10])),
]);
batch.add('fillet', 'a 30 × 30 × 5 plate with a boss (radius 5) 10 tall on it: the boss\'s foot filleted unevenly, 1 and 2 mm', [
  box('base', [-15, -15, 0], [15, 15, 5]),
  sketch('bossSketch', top(5)), circle('bossSketch', 'circle1', [0, 0], 5), solve('bossSketch'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(10), operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base }),
  filleted(edgeAt(base, [5, 0, 5])),
]);
batch.add('fillet', 'a cylinder (radius 10) extruded 10 with a 10° draft inwards: its top edge filleted unevenly, 1 and 2 mm', [
  sketch('baseSketch', top(0)), circle('baseSketch', 'circle1', [0, 0], 10), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(10), hasDraft: 'true', draftAngle: '10 * degree', draftPullDirection: 'false' }),
  filleted(`qNthElement(qSubtraction(qOwnedByBody(${base}, EntityType.EDGE), ${edgeAt(base, [10, 0, 0])}), 0)`),
]);

// ----- sketches solved by their constraints -----

const S = 's';
const seg = (name, a, b, extra = '') => `skLineSegment(${S}, "${name}", { "start" : ${vec2(...a)}, "end" : ${vec2(...b)}${extra} });`;
const con = (name, type, fields) =>
  `skConstraint(${S}, "${name}", { "constraintType" : ConstraintType.${type}${Object.entries(fields).map(([k, v]) => `, "${k}" : ${v}`).join('')} });`;
const q = (s) => `"${s}"`;
const solved = (body) => [
  sketch(S, top(0)), body, solve(S),
  `return { "curves" : sketchProbe(context, id + "${S}") };`,
];
// Four lines of a rectangle drawn a little off, their ends joined.
const quad = [seg('l1', [0.3, -0.2], [29, 0.6]), seg('l2', [29.4, 0.2], [30.5, 19]), seg('l3', [30.2, 19.6], [0.4, 20.5]), seg('l4', [-0.3, 20.2], [0.2, 0.3])];
const joined = ['l1.end:l2.start', 'l2.end:l3.start', 'l3.end:l4.start', 'l4.end:l1.start'].map((p, i) => {
  const [a, b] = p.split(':');
  return con(`c${i + 1}`, 'COINCIDENT', { localFirst: q(a), localSecond: q(b) });
});
const squared = [
  con('h1', 'HORIZONTAL', { localFirst: q('l1') }), con('h3', 'HORIZONTAL', { localFirst: q('l3') }),
  con('v2', 'VERTICAL', { localFirst: q('l2') }), con('v4', 'VERTICAL', { localFirst: q('l4') }),
];

batch.add('sketch', 'four lines drawn a little off, joined, made horizontal and vertical, a corner fixed, 30 and 20 long by length constraints', solved([
  quad, joined, squared, con('fix', 'FIX', { localFirst: q('l1.start') }),
  con('len1', 'LENGTH', { localFirst: q('l1'), length: mm(30) }), con('len2', 'LENGTH', { localFirst: q('l2'), length: mm(20) }),
]));
batch.add('sketch', 'the same lines with distance constraints between corners instead, horizontal 30 and vertical 20', solved([
  quad, joined, squared, con('fix', 'FIX', { localFirst: q('l1.start') }),
  con('d1', 'DISTANCE', { localFirst: q('l1.start'), localSecond: q('l1.end'), length: mm(30), direction: 'DimensionDirection.HORIZONTAL' }),
  con('d2', 'DISTANCE', { localFirst: q('l2.start'), localSecond: q('l2.end'), length: mm(20), direction: 'DimensionDirection.VERTICAL' }),
]));
batch.add('sketch', 'the same lines made a square by equal sides, a side 25 long', solved([
  quad, joined, squared, con('fix', 'FIX', { localFirst: q('l1.start') }),
  con('eq', 'EQUAL', { localFirst: q('l1'), localSecond: q('l2') }), con('len1', 'LENGTH', { localFirst: q('l1'), length: mm(25) }),
]));
batch.add('sketch', 'a circle drawn at (12, 9) with radius 6, made concentric with a fixed point at (10, 10) and radius 7', solved([
  `skPoint(${S}, "p", { "position" : ${vec2(10, 10)} });`,
  `skCircle(${S}, "circle1", { "center" : ${vec2(12, 9)}, "radius" : ${mm(6)} });`,
  con('fix', 'FIX', { localFirst: q('p') }), con('conc', 'CONCENTRIC', { localFirst: q('circle1'), localSecond: q('p') }),
  con('r', 'RADIUS', { localFirst: q('circle1'), length: mm(7) }),
]));
batch.add('sketch', 'a circle drawn at (12, 9) with radius 6, its centre made coincident with a fixed point at (10, 10), diameter 12', solved([
  `skPoint(${S}, "p", { "position" : ${vec2(10, 10)} });`,
  `skCircle(${S}, "circle1", { "center" : ${vec2(12, 9)}, "radius" : ${mm(6)} });`,
  con('fix', 'FIX', { localFirst: q('p') }), con('co', 'COINCIDENT', { localFirst: q('circle1.center'), localSecond: q('p') }),
  con('d', 'DIAMETER', { localFirst: q('circle1'), length: mm(12) }),
]));
batch.add('sketch', 'a fixed horizontal line and a line from its start drawn at 40°, made 30° to it and 20 long', solved([
  seg('l1', [0, 0], [30, 0]), seg('l2', [0.4, 0.3], [15, 13]),
  con('fix', 'FIX', { localFirst: q('l1') }), con('co', 'COINCIDENT', { localFirst: q('l1.start'), localSecond: q('l2.start') }),
  con('a', 'ANGLE', { localFirst: q('l1'), localSecond: q('l2'), angle: '30 * degree' }), con('len', 'LENGTH', { localFirst: q('l2'), length: mm(20) }),
]));
batch.add('sketch', 'a slot: two lines and two arcs drawn off, joined and tangent, the arcs of radius 5, the lines horizontal, one end fixed and the lines 20 long', solved([
  seg('l1', [0.2, 0.3], [20.5, -0.2]), `skArc(${S}, "a1", { "start" : ${vec2(20.5, 0)}, "mid" : ${vec2(25.5, 5.3)}, "end" : ${vec2(20, 10.4)} });`,
  seg('l2', [20.3, 10], [-0.3, 9.6]), `skArc(${S}, "a2", { "start" : ${vec2(0, 10.2)}, "mid" : ${vec2(-4.6, 5)}, "end" : ${vec2(0.1, -0.2)} });`,
  con('c1', 'COINCIDENT', { localFirst: q('l1.end'), localSecond: q('a1.start') }), con('c2', 'COINCIDENT', { localFirst: q('a1.end'), localSecond: q('l2.start') }),
  con('c3', 'COINCIDENT', { localFirst: q('l2.end'), localSecond: q('a2.start') }), con('c4', 'COINCIDENT', { localFirst: q('a2.end'), localSecond: q('l1.start') }),
  con('t1', 'TANGENT', { localFirst: q('l1'), localSecond: q('a1') }), con('t2', 'TANGENT', { localFirst: q('a1'), localSecond: q('l2') }),
  con('t3', 'TANGENT', { localFirst: q('l2'), localSecond: q('a2') }), con('t4', 'TANGENT', { localFirst: q('a2'), localSecond: q('l1') }),
  con('h', 'HORIZONTAL', { localFirst: q('l1') }), con('fix', 'FIX', { localFirst: q('l1.start') }),
  con('r1', 'RADIUS', { localFirst: q('a1'), length: mm(5) }), con('eq', 'EQUAL', { localFirst: q('a1'), localSecond: q('a2') }),
  con('len', 'LENGTH', { localFirst: q('l1'), length: mm(20) }),
]));
batch.add('sketch', 'a fixed line from (0, 0) to (20, 10) and a point drawn at (9, 7), made its midpoint', solved([
  seg('l1', [0, 0], [20, 10]), `skPoint(${S}, "p", { "position" : ${vec2(9, 7)} });`,
  con('fix', 'FIX', { localFirst: q('l1') }), con('mid', 'MIDPOINT', { localFirst: q('p'), localSecond: q('l1') }),
  `skLineSegment(${S}, "l2", { "start" : ${vec2(9, 7)}, "end" : ${vec2(9, 20)} });`,
  con('co', 'COINCIDENT', { localFirst: q('p'), localSecond: q('l2.start') }), con('v', 'VERTICAL', { localFirst: q('l2') }),
  con('len', 'LENGTH', { localFirst: q('l2'), length: mm(10) }),
]));
batch.add('sketch', 'a fixed line and a line drawn near its mirror image across a fixed vertical construction line at x = 10, made its mirror image', solved([
  seg('axis', [10, -5], [10, 25], ', "construction" : true'), seg('l1', [2, 3], [6, 15]), seg('l2', [17.5, 3.4], [14.3, 14.6]),
  con('fa', 'FIX', { localFirst: q('axis') }), con('f1', 'FIX', { localFirst: q('l1') }),
  con('m', 'MIRROR', { localMirror: q('axis'), localFirst: q('l1'), localSecond: q('l2') }),
]));
batch.add('sketch', 'a triangle drawn off, joined, its base fixed and horizontal, its sides 40 and 50 long', solved([
  seg('l1', [0, 0], [30, 0]), seg('l2', [30.4, 0.3], [29, 41]), seg('l3', [29.5, 40.2], [0.4, -0.3]),
  con('fix', 'FIX', { localFirst: q('l1') }),
  con('c1', 'COINCIDENT', { localFirst: q('l1.end'), localSecond: q('l2.start') }), con('c2', 'COINCIDENT', { localFirst: q('l2.end'), localSecond: q('l3.start') }),
  con('c3', 'COINCIDENT', { localFirst: q('l3.end'), localSecond: q('l1.start') }),
  con('len2', 'LENGTH', { localFirst: q('l2'), length: mm(40) }), con('len3', 'LENGTH', { localFirst: q('l3'), length: mm(50) }),
]));

batch.write('tools/fixtures/gen-batch-135.mjs');
