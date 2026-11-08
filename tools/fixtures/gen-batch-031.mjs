// Generates fixture batch 031: features the tracker had missed. External threads on a shaft (blind with its face
// split, with a chamfer, with an undercut, up to the next face), and a thin solid recognized as sheet metal.
//
// Usage: node tools/fixtures/gen-batch-031.mjs
//
// Writes fixtures/batch-031/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, cylinder, deg, edgeAt, line, mm, planeAt, sketch, solidsOf, solve } from './lib.mjs';

const helpers = `
// The hole attribute of a face (external threads describe themselves with one), as far as the thread goes.
function threadInfo(context is Context, face is Query) returns map
{
    const attributes = getHoleAttributes(context, face);
    if (size(attributes) != 1)
        return { "attributes" : size(attributes) };
    var out = {};
    for (var key in ["holeType", "isTappedHole", "tappedDepth", "majorDiameter", "minorDiameter", "threadPitch", "isExternalThread"])
    {
        const value = attributes[0][key];
        out[key] = value is ValueWithUnits ? round(value / millimeter * 1e6) / 1e6 : value;
    }
    return out;
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}
`;

const batch = new Batch({ number: 31, title: 'external threads and sheet metal recognition', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const m10 = `{ "standard" : "ISO", "size" : "M10", "pitch" : "1.5 mm" } as LookupTablePath`;
const shaft = cylinder('base', [0, 0], 5, 0, 40);
const thread = (fields) =>
  call('externalThread', 'thread1', {
    entities: edgeAt(base, [5, 0, 40]), branchOfStandard: m10, depthType: 'DepthType.Blind', threadDepth: mm(15), addChamfer: 'false',
    addUndercut: 'false', splitFace: 'true', ...fields,
  });
const info = returns({ thread: `threadInfo(context, qContainsPoint(qOwnedByBody(${base}, EntityType.FACE), vector(5, 0, 35) * millimeter))` });

batch.add('external-thread', 'an M10 thread 15 mm down the top of a 10 mm shaft, its face split', [shaft, thread({}), info]);
batch.add('external-thread', 'the thread with a 1 mm 45 degree chamfer', [
  shaft, thread({ addChamfer: 'true', chamferWidth: mm(1), chamferAngle: deg(45) }), info,
]);
batch.add('external-thread', 'the thread with an 8 mm undercut 3 mm long', [
  shaft, thread({ addUndercut: 'true', undercutDiameter: mm(8), undercutLength: mm(3) }), info,
]);
batch.add('external-thread', 'an M10 thread up to the step of a shaft', [
  cylinder('base', [0, 0], 5, 20, 40), cylinder('step', [0, 0], 8, 0, 20),
  call('booleanBodies', 'join', { tools: `qUnion([${base}, ${solidsOf('step')}])`, operationType: 'BooleanOperationType.UNION' }),
  thread({ depthType: 'DepthType.UpToNext' }), info,
]);

// ----- recognizing sheet metal -----

const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
batch.add('sheet-metal-recognize', 'an L bracket 2 mm thick, extruded 30 mm, recognized as sheet metal with 1 mm bends', [
  sketch('s1', xy),
  `skPolyline(s1, "l", { "points" : [vector(0, 0) * millimeter, vector(40, 0) * millimeter, vector(40, 25) * millimeter, vector(38, 25) * millimeter, vector(38, 2) * millimeter, vector(0, 2) * millimeter, vector(0, 0) * millimeter] });`,
  solve('s1'),
  call('extrude', 'base', { entities: `qSketchRegion(id + "s1")`, endBound: 'BoundingType.BLIND', depth: mm(30) }),
  call('sheetMetalRecognize', 'recognize1', { bodies: base, radius: mm(1), changeThickness: 'false', keepInputParts: 'false', kFactor: '0.45', minimalClearance: mm(0.02) }),
  returns({ counts: 'bodyCounts(context)' }),
]);

batch.write('tools/fixtures/gen-batch-031.mjs');
