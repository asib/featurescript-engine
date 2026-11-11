
// ----- Builds every case of batch 118: in Onshape for the fixture, and on our engine to compare. -----

annotation { "Feature Type Name" : "Build fixture batch 118" }
export const buildFixtureBatch118 = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        var failed = [];
        if (!runFixtureCase(context, id, "c001", vector(0, 0, 0) * meter,
                function(context is Context, id is Id) { return c001(context, id); }))
            failed = append(failed, "c001");
        if (!runFixtureCase(context, id, "c002", vector(0.4, 0, 0) * meter,
                function(context is Context, id is Id) { return c002(context, id); }))
            failed = append(failed, "c002");
        if (!runFixtureCase(context, id, "c003", vector(0.8, 0, 0) * meter,
                function(context is Context, id is Id) { return c003(context, id); }))
            failed = append(failed, "c003");
        if (!runFixtureCase(context, id, "c004", vector(1.2, 0, 0) * meter,
                function(context is Context, id is Id) { return c004(context, id); }))
            failed = append(failed, "c004");
        println("batch-118 failed cases: " ~ toString(failed));
    });

// The fixture document's Part Studio holds one instance of the feature batch 001 defined, so every batch also
// exports its generator under that name: pasting a new batch then rebuilds the same feature instance.
annotation { "Feature Type Name" : "Build fixture batch 118 (as 001)" }
export const buildFixtureBatch001 = buildFixtureBatch118;

const FIXTURE_BODY_TYPES = [BodyType.SOLID, BodyType.SHEET, BodyType.WIRE, BodyType.POINT, BodyType.MATE_CONNECTOR];

// Builds one case, moves everything it made to the case's slot, names its bodies "<case>.<n>" and prints its probe
// line. Returns false if the case threw, or made no body and returned nothing.
function runFixtureCase(context is Context, id is Id, name is string, slot is Vector, build is function) returns boolean
{
    const caseId = id + name;
    var values;
    try
    {
        values = build(context, caseId);
    }
    catch (error)
    {
        clearFeatureStatus(context, id, { "withDisplayData" : true });
        try silent(opDeleteBodies(context, id + (name ~ "cleanup"), { "entities" : qCreatedBy(caseId, EntityType.BODY) }));
        println("probe " ~ name ~ " failed " ~ fixtureErrorText(error));
        return false;
    }
    const everything = qCreatedBy(caseId, EntityType.BODY);
    if (!isQueryEmpty(context, everything))
    {
        try silent
        {
            opTransform(context, caseId + "move", { "bodies" : everything, "transform" : transform(slot) });
        }
        catch
        {
            // Sheet metal parts can't be moved (SHEET_METAL_PARTS_PROHIBITED): such a case stays where it was built.
            clearFeatureStatus(context, id, { "withDisplayData" : true });
        }
    }
    const notSketch = qSketchFilter(everything, SketchObject.NO);
    const bodies = fixtureSortedBodies(context, qConstructionFilter(notSketch, ConstructionObject.NO));
    var probes = [];
    for (var i, body in bodies)
    {
        const bodyName = name ~ "." ~ (i + 1);
        try silent(setProperty(context, { "entities" : body, "propertyType" : PropertyType.NAME, "value" : bodyName }));
        probes = append(probes, fixtureProbe(context, body, bodyName));
    }
    var construction = [];
    for (var body in fixtureSortedBodies(context, qConstructionFilter(notSketch, ConstructionObject.YES)))
        construction = append(construction, fixtureProbe(context, body, "construction"));
    println("probe " ~ name ~ " " ~ toString({ "bodies" : probes, "construction" : construction, "values" : values }));
    return size(bodies) > 0 || values != undefined;
}

function fixtureErrorText(error) returns string
{
    if (error is map && error.message != undefined)
        return toString(error.message);
    return error == undefined ? "error" : toString(error);
}

function fixtureBodyType(context is Context, body is Query)
{
    for (var bodyType in FIXTURE_BODY_TYPES)
    {
        if (!isQueryEmpty(context, qBodyType(body, bodyType)))
            return bodyType;
    }
    return undefined;
}

function fixtureNumbers(v is Vector, unit) returns array
{
    var out = [];
    for (var x in v)
        out = append(out, x / unit);
    return out;
}

function fixtureBox(context is Context, body is Query) returns array
{
    const bounds = evBox3d(context, { "topology" : body, "tight" : true });
    return concatenateArrays([fixtureNumbers(bounds.minCorner, millimeter), fixtureNumbers(bounds.maxCorner, millimeter)]);
}

// Sorts bodies by type, then by the corners of their bounding boxes rounded to 10 µm.
function fixtureSortedBodies(context is Context, query is Query) returns array
{
    var keyed = [];
    for (var body in evaluateQuery(context, query))
    {
        var key = [indexOf(FIXTURE_BODY_TYPES, fixtureBodyType(context, body))];
        const bounds = try silent(fixtureBox(context, body));
        if (bounds != undefined)
        {
            for (var x in bounds)
                key = append(key, round(x * 100));
        }
        keyed = append(keyed, { "body" : body, "key" : key });
    }
    keyed = sort(keyed, function(a, b) { return fixtureCompareKeys(a.key, b.key); });
    var out = [];
    for (var k in keyed)
        out = append(out, k.body);
    return out;
}

function fixtureCompareKeys(a is array, b is array) returns number
{
    for (var i = 0; i < min(size(a), size(b)); i += 1)
    {
        if (a[i] != b[i])
            return a[i] - b[i];
    }
    return size(a) - size(b);
}

function fixtureProbe(context is Context, body is Query, name is string) returns map
{
    const bodyType = fixtureBodyType(context, body);
    var probe = {
        "name" : name,
        "type" : bodyType,
        "faces" : evaluateQueryCount(context, qOwnedByBody(body, EntityType.FACE)),
        "edges" : evaluateQueryCount(context, qOwnedByBody(body, EntityType.EDGE)),
        "vertices" : evaluateQueryCount(context, qOwnedByBody(body, EntityType.VERTEX))
    };
    probe.bounds = try silent(fixtureBox(context, body));
    if (bodyType == BodyType.SOLID)
        probe.volume = evVolume(context, { "entities" : body }) / millimeter ^ 3;
    if (bodyType == BodyType.SOLID || bodyType == BodyType.SHEET)
        probe.area = evArea(context, { "entities" : qOwnedByBody(body, EntityType.FACE) }) / millimeter ^ 2;
    if (bodyType == BodyType.WIRE)
        probe.length = evLength(context, { "entities" : qOwnedByBody(body, EntityType.EDGE) }) / millimeter;
    if (bodyType == BodyType.SHEET && probe.faces == 1)
    {
        const plane = try silent(evPlane(context, { "face" : qOwnedByBody(body, EntityType.FACE) }));
        if (plane != undefined)
            probe.plane = [fixtureNumbers(plane.origin, millimeter), fixtureNumbers(plane.normal, 1), fixtureNumbers(plane.x, 1)];
    }
    if (bodyType == BodyType.MATE_CONNECTOR)
    {
        const cs = evMateConnector(context, { "mateConnector" : body });
        probe.frame = [fixtureNumbers(cs.origin, millimeter), fixtureNumbers(cs.xAxis, 1), fixtureNumbers(cs.zAxis, 1)];
    }
    return probe;
}
