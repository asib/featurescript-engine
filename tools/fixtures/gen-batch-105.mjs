// Generates fixture batch 105: std's last builtins no feature of a Part Studio reaches, through the std functions
// that call them, imported into the Feature Studio: a tolerance schema's validation (validateToleranceSchema, from
// toleranceSchemaValidation.fs's validate), a mesh's points (evMeshPoints) of bodies that aren't meshes, and a sheet
// metal rip's derip (opDerip, from moveFace.fs's deripEdges) on a box converted from a block with rips up its
// corners: the definition sheet's edges before and after.
//
// Usage: node tools/fixtures/gen-batch-105.mjs
//
// Writes fixtures/batch-105/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, faceAt, mm, solidsOf, vec3 } from './lib.mjs';

const helpers = `
function roundAll(a is array) returns array
{
    return mapArray(a, function(x) { return x is number ? roundToPrecision(x, 6) : (x is array ? roundAll(x) : x); });
}

function mm(p is Vector) returns array
{
    return roundAll([p[0] / millimeter, p[1] / millimeter, p[2] / millimeter]);
}

// A sheet metal part's definition sheet: its edges, each as its ends (sorted), how many of the sheet's faces it
// bounds, and its joint's type; and its faces' areas.
function sheetOf(context is Context, part is Query) returns map
{
    const sheet = qOwnerBody(qUnion(Sa::getSMDefinitionEntities(context, qOwnedByBody(part, EntityType.FACE))));
    var edges = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(sheet, EntityType.EDGE)))
    {
        const a = mm(evEdgeTangentLine(context, { "edge" : edge, "parameter" : 0 }).origin);
        const b = mm(evEdgeTangentLine(context, { "edge" : edge, "parameter" : 1 }).origin);
        const joint = try silent(Sa::getJointAttribute(context, edge));
        edges = append(edges, { "ends" : sort([a, b], function(p, q) { return p[0] != q[0] ? p[0] - q[0] : (p[1] != q[1] ? p[1] - q[1] : p[2] - q[2]); }),
                "faces" : size(evaluateQuery(context, qAdjacent(edge, AdjacencyType.EDGE, EntityType.FACE))),
                "joint" : joint == undefined || joint.jointType == undefined ? "none" : joint.jointType.value });
    }
    edges = sort(edges, function(e, f) { return e.ends[0][0] != f.ends[0][0] ? e.ends[0][0] - f.ends[0][0] : (e.ends[0][1] != f.ends[0][1] ? e.ends[0][1] - f.ends[0][1] : (e.ends[0][2] != f.ends[0][2] ? e.ends[0][2] - f.ends[0][2] : (e.ends[1][0] != f.ends[1][0] ? e.ends[1][0] - f.ends[1][0] : (e.ends[1][1] != f.ends[1][1] ? e.ends[1][1] - f.ends[1][1] : e.ends[1][2] - f.ends[1][2])))); });
    const areas = sort(mapArray(evaluateQuery(context, qOwnedByBody(sheet, EntityType.FACE)), function(f) { return roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 4); }), function(p, q) { return p - q; });
    return { "edges" : edges, "areas" : areas };
}

// A sheet metal feature (opDerip needs a sheet metal feature's scope) deripping \`edges\` of \`part\` with std's
// deripEdges, putting whether it did and the definition sheet then in the box \`result\`. (Rebuilding the parts then
// fails with the rip open, SHEET_METAL_REBUILD_ERROR, and fails the feature even caught: a second build.)
const deripFeature = Su::defineSheetMetalFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        const deripped = Mf::deripEdges(context, id + "derip", definition.edges);
        definition.result[] = { "deripped" : deripped, "definition" : sheetOf(context, definition.part) };
    }, {});

// The definition sheet's rip edges nearest a point.
function ripNear(context is Context, part is Query, point is Vector) returns Query
{
    const sheet = qOwnerBody(qUnion(Sa::getSMDefinitionEntities(context, qOwnedByBody(part, EntityType.FACE))));
    var rips = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(sheet, EntityType.EDGE)))
    {
        const joint = try silent(Sa::getJointAttribute(context, edge));
        if (joint != undefined && joint.jointType != undefined && joint.jointType.value == SMJointType.RIP)
            rips = append(rips, edge);
    }
    return qClosestTo(qUnion(rips), point);
}
`;

const batch = new Batch({
  number: 105,
  title: "std's last builtins through the std functions that call them",
  helpers,
  imports: [
    'import(path : "onshape/std/configurationInfo.fs", version : "3112.0");',
    'Tv::import(path : "onshape/std/toleranceSchemaValidation.fs", version : "3112.0");',
    'Sa::import(path : "onshape/std/sheetMetalAttribute.fs", version : "3112.0");',
    'Mf::import(path : "onshape/std/moveFace.fs", version : "3112.0");',
    'Su::import(path : "onshape/std/sheetMetalUtils.fs", version : "3112.0");',
  ],
});

const caught = (statement) => `try { ${statement} } catch (error) { return { "error" : error }; }`;
const block = box('base', [0, 0, 0], [10, 20, 30]);
const base = 'qCreatedBy(id + "base", EntityType.BODY)';

batch.add('tolerance', "a tolerance schema's validation of this Part Studio, unconfigured", [
  caught(`return { "problems" : Tv::validate(context, { "inputs" : [] } as ConfigurationInfo) };`),
]);
batch.add('tolerance', "a tolerance schema's validation of a new context, unconfigured", [
  caught(`return { "problems" : Tv::validate(newContext(), { "inputs" : [] } as ConfigurationInfo) };`),
]);
batch.add('tolerance', "a tolerance schema's validation of a new context with defaults, given a boolean input", [
  caught(`return { "problems" : Tv::validate(newContextWithDefaults(), { "inputs" : [{ "parameterId" : "flag", "parameterName" : "Flag", "inputType" : ConfigurationInputType.BOOLEAN, "defaultValue" : false } as ConfigurationInputInfo] } as ConfigurationInfo) };`),
]);
batch.add('mesh', "a block's mesh points (it isn't a mesh)", [
  block,
  caught(`return { "points" : evMeshPoints(context, { "meshes" : ${base} }) };`),
]);
batch.add('mesh', "a block's faces' mesh points", [
  block,
  caught(`return { "points" : evMeshPoints(context, { "meshes" : qOwnedByBody(${base}, EntityType.FACE) }) };`),
]);
batch.add('mesh', 'no mesh points of nothing', [
  caught(`return { "points" : evMeshPoints(context, { "meshes" : qNothing() }) };`),
]);

const openBox = [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('sheetMetalStart', 'sm1', {
    process: 'SMProcessType.CONVERT', partToConvert: solidsOf('base'), facesToExclude: faceAt(solidsOf('base'), [20, 15, 20]),
    bends: `qUnion([${[[20, 0, 0], [40, 15, 0], [20, 30, 0], [0, 15, 0]].map((p) => `qContainsPoint(qOwnedByBody(${solidsOf('base')}, EntityType.EDGE), ${vec3(...p)})`).join(', ')}])`,
    clearance: mm(0.2), keepInputParts: 'false', thickness: mm(1), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
  }),
  `const part = qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID);`,
];
batch.add('derip', 'an open box converted from a block with rips up its corners (0.2 mm clearance): its definition sheet', [
  openBox,
  `return { "definition" : sheetOf(context, part) };`,
]);
batch.add('derip', 'the same box with its corner rip at the origin deripped (moveFace.fs deripEdges) in a sheet metal feature: its definition sheet after the derip', [
  openBox,
  `const result = new box(undefined);`,
  `try silent(deripFeature(context, id + "derip1", { "part" : part, "edges" : ripNear(context, part, ${vec3(0, 0, 10)}), "result" : result }));`,
  `return result[];`,
]);
batch.add('derip', 'the same box with all four corner rips deripped at once: its definition sheet after', [
  openBox,
  `const rips = qUnion([${[[0, 0], [40, 0], [40, 30], [0, 30]].map(([x, y]) => `ripNear(context, part, ${vec3(x, y, 10)})`).join(', ')}]);`,
  `const result = new box(undefined);`,
  `try silent(deripFeature(context, id + "derip1", { "part" : part, "edges" : rips, "result" : result }));`,
  `return result[];`,
]);

batch.write('tools/fixtures/gen-batch-105.mjs');
