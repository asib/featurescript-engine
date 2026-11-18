// Generates fixture batch 191: what Onshape's swept and revolved tori report: each curved face's surface type, its
// periodicity (trimmed and not) and two auto-spaced iso curves each way, for a frame's round tube along a quarter
// circle (batch 072 c001, whose cut list finds no length), the same circle swept along it, a quarter torus revolved,
// and a cylinder.
//
// Usage: node tools/fixtures/gen-batch-191.mjs
//
// Writes fixtures/batch-191/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, call, circle, edgeOf, mm, planeAt, regions, sketch, solidsOf, solve, PLANES } from './lib.mjs';

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

// Nine points along each edge of a body, evenly spaced in its own parameter.
function edgeProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1], "arcLengthParameterization" : false });
        var points = [];
        for (var l in lines)
            points = append(points, round6(l.origin));
        out = append(out, points);
    }
    return out;
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

function tableText(t is map) returns array
{
    var out = [mapArray(t.columnDefinitions, function(c) { return toString(c.name); })];
    for (var row in t.rows)
        out = append(out, mapArray(t.columnDefinitions, function(c) { return toString(row.columnIdToCell[c.id]); }));
    return out;
}

function caseTables(context is Context, caseId is Id, tables) returns array
{
    var out = [];
    for (var t in tables)
    {
        if (t.entities == undefined || !isQueryEmpty(context, qIntersection([t.entities, qCreatedBy(caseId, EntityType.BODY)])))
            out = append(out, tableText(t));
    }
    return out;
}

// Frame profiles, each the only sketch of its own context, as a profile from an element library would be.
const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-5, -5) * millimeter, "secondCorner" : vector(5, 5) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-4, -4) * millimeter, "secondCorner" : vector(4, 4) * millimeter });
    skSolve(s);
    return context;
};

const roundTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skCircle(s, "outer", { "center" : vector(0, 0) * millimeter, "radius" : 6 * millimeter });
    skCircle(s, "inner", { "center" : vector(0, 0) * millimeter, "radius" : 5 * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}

// Each curved face of a body: its surface's type, its periodicity trimmed and not, and the lengths and ends of two
// auto-spaced iso curves each way.
function isoProbe(context is Context, id is Id, body is Query) returns array
{
    var out = [];
    var k = 0;
    for (var face in evaluateQuery(context, qSubtraction(qOwnedByBody(body, EntityType.FACE), qGeometry(qOwnedByBody(body, EntityType.FACE), GeometryType.PLANE))))
    {
        var entry = { "type" : evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType,
                      "periodic" : evFacePeriodicity(context, { "face" : face }),
                      "periodicSurface" : evFacePeriodicity(context, { "face" : face, "trimmed" : false }) };
        for (var dir in [["dir1", FaceCurveCreationType.DIR1_AUTO_SPACED_ISO], ["dir2", FaceCurveCreationType.DIR2_AUTO_SPACED_ISO]])
        {
            const cid = id + ("iso" ~ k);
            k += 1;
            try
            {
                opCreateCurvesOnFace(context, cid, { "curveDefinition" : [curveOnFaceDefinition(face, dir[1], ["a", "b"], 2)] });
                var curves = [];
                for (var edge in evaluateQuery(context, qCreatedBy(cid, EntityType.EDGE)))
                {
                    const ends = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 1], "arcLengthParameterization" : false });
                    curves = append(curves, { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6),
                                              "start" : round6(ends[0].origin), "end" : round6(ends[1].origin) });
                }
                entry[dir[0]] = curves;
                opDeleteBodies(context, cid + "delete", { "entities" : qCreatedBy(cid, EntityType.BODY) });
            }
            catch (error)
            {
                entry[dir[0]] = "failed";
            }
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 191, title: 'probes: periodicity and iso curves of swept and revolved tori', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const quarter = [sketch('path', PLANES.xy), arc('path', 'a1', [40, 0], [28.284271247461902, 28.284271247461902], [0, 40]), solve('path')];

batch.add('probe', "a frame's round tube along a quarter circle of radius 40", [
  ...quarter, call('frame', 'frame1', { profileSketch: 'profileOf(roundTube)', selections: wireEdges('path') }),
  returns({ faces: `isoProbe(context, id, ${solidsOf('frame1')})` })]);
batch.add('probe', 'a circle of radius 5 swept along the quarter circle', [
  ...quarter, sketch('prof', planeAt([40, 0, 0], [0, 1, 0], [1, 0, 0])), circle('prof', 'c', [0, 0], 5), solve('prof'),
  call('sweep', 'sweep1', { profiles: regions('prof'), path: wireEdges('path'), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW' }),
  returns({ faces: `isoProbe(context, id, ${solidsOf('sweep1')})` })]);
batch.add('probe', 'the circle revolved a quarter turn about z', [
  sketch('prof', planeAt([40, 0, 0], [0, 1, 0], [1, 0, 0])), circle('prof', 'c', [0, 0], 5), solve('prof'),
  ...axisLine('axis1', [0, 0, 0], [0, 0, 10]),
  call('revolve', 'rev1', { entities: regions('prof'), axis: edgeOf('axis1'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', angle: '90 * degree', bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW' }),
  returns({ faces: `isoProbe(context, id, ${solidsOf('rev1')})` })]);
batch.add('probe', 'a cylinder of radius 5 and 20 long', [
  sketch('prof', PLANES.xy), circle('prof', 'c', [0, 0], 5), solve('prof'),
  call('extrude', 'ext1', { entities: regions('prof'), endBound: 'BoundingType.BLIND', depth: mm(20) }),
  returns({ faces: `isoProbe(context, id, ${solidsOf('ext1')})` })]);

batch.write('tools/fixtures/gen-batch-191.mjs');
