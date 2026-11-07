
// ----- Onshape only: builds every case of batch 001 for the STEP export. Not used by our engine. -----

annotation { "Feature Type Name" : "Build fixture batch 001" }
export const buildFixtureBatch001 = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        var failed = [];
        if (!runFixtureCase(context, id, "c001", vector(0, 0, 0) * meter,
                function(context is Context, id is Id) { c001(context, id); }))
            failed = append(failed, "c001");
        if (!runFixtureCase(context, id, "c002", vector(0.4, 0, 0) * meter,
                function(context is Context, id is Id) { c002(context, id); }))
            failed = append(failed, "c002");
        if (!runFixtureCase(context, id, "c003", vector(0.8, 0, 0) * meter,
                function(context is Context, id is Id) { c003(context, id); }))
            failed = append(failed, "c003");
        if (!runFixtureCase(context, id, "c004", vector(1.2, 0, 0) * meter,
                function(context is Context, id is Id) { c004(context, id); }))
            failed = append(failed, "c004");
        if (!runFixtureCase(context, id, "c005", vector(1.6, 0, 0) * meter,
                function(context is Context, id is Id) { c005(context, id); }))
            failed = append(failed, "c005");
        if (!runFixtureCase(context, id, "c006", vector(2, 0, 0) * meter,
                function(context is Context, id is Id) { c006(context, id); }))
            failed = append(failed, "c006");
        if (!runFixtureCase(context, id, "c007", vector(2.4, 0, 0) * meter,
                function(context is Context, id is Id) { c007(context, id); }))
            failed = append(failed, "c007");
        if (!runFixtureCase(context, id, "c008", vector(2.8, 0, 0) * meter,
                function(context is Context, id is Id) { c008(context, id); }))
            failed = append(failed, "c008");
        if (!runFixtureCase(context, id, "c009", vector(3.2, 0, 0) * meter,
                function(context is Context, id is Id) { c009(context, id); }))
            failed = append(failed, "c009");
        if (!runFixtureCase(context, id, "c010", vector(3.6, 0, 0) * meter,
                function(context is Context, id is Id) { c010(context, id); }))
            failed = append(failed, "c010");
        if (!runFixtureCase(context, id, "c011", vector(0, 0.4, 0) * meter,
                function(context is Context, id is Id) { c011(context, id); }))
            failed = append(failed, "c011");
        if (!runFixtureCase(context, id, "c012", vector(0.4, 0.4, 0) * meter,
                function(context is Context, id is Id) { c012(context, id); }))
            failed = append(failed, "c012");
        if (!runFixtureCase(context, id, "c013", vector(0.8, 0.4, 0) * meter,
                function(context is Context, id is Id) { c013(context, id); }))
            failed = append(failed, "c013");
        if (!runFixtureCase(context, id, "c014", vector(1.2, 0.4, 0) * meter,
                function(context is Context, id is Id) { c014(context, id); }))
            failed = append(failed, "c014");
        if (!runFixtureCase(context, id, "c015", vector(1.6, 0.4, 0) * meter,
                function(context is Context, id is Id) { c015(context, id); }))
            failed = append(failed, "c015");
        if (!runFixtureCase(context, id, "c016", vector(2, 0.4, 0) * meter,
                function(context is Context, id is Id) { c016(context, id); }))
            failed = append(failed, "c016");
        if (!runFixtureCase(context, id, "c017", vector(2.4, 0.4, 0) * meter,
                function(context is Context, id is Id) { c017(context, id); }))
            failed = append(failed, "c017");
        if (!runFixtureCase(context, id, "c018", vector(2.8, 0.4, 0) * meter,
                function(context is Context, id is Id) { c018(context, id); }))
            failed = append(failed, "c018");
        if (!runFixtureCase(context, id, "c019", vector(3.2, 0.4, 0) * meter,
                function(context is Context, id is Id) { c019(context, id); }))
            failed = append(failed, "c019");
        if (!runFixtureCase(context, id, "c020", vector(3.6, 0.4, 0) * meter,
                function(context is Context, id is Id) { c020(context, id); }))
            failed = append(failed, "c020");
        if (!runFixtureCase(context, id, "c021", vector(0, 0.8, 0) * meter,
                function(context is Context, id is Id) { c021(context, id); }))
            failed = append(failed, "c021");
        if (!runFixtureCase(context, id, "c022", vector(0.4, 0.8, 0) * meter,
                function(context is Context, id is Id) { c022(context, id); }))
            failed = append(failed, "c022");
        if (!runFixtureCase(context, id, "c023", vector(0.8, 0.8, 0) * meter,
                function(context is Context, id is Id) { c023(context, id); }))
            failed = append(failed, "c023");
        if (!runFixtureCase(context, id, "c024", vector(1.2, 0.8, 0) * meter,
                function(context is Context, id is Id) { c024(context, id); }))
            failed = append(failed, "c024");
        if (!runFixtureCase(context, id, "c025", vector(1.6, 0.8, 0) * meter,
                function(context is Context, id is Id) { c025(context, id); }))
            failed = append(failed, "c025");
        if (!runFixtureCase(context, id, "c026", vector(2, 0.8, 0) * meter,
                function(context is Context, id is Id) { c026(context, id); }))
            failed = append(failed, "c026");
        if (!runFixtureCase(context, id, "c027", vector(2.4, 0.8, 0) * meter,
                function(context is Context, id is Id) { c027(context, id); }))
            failed = append(failed, "c027");
        if (!runFixtureCase(context, id, "c028", vector(2.8, 0.8, 0) * meter,
                function(context is Context, id is Id) { c028(context, id); }))
            failed = append(failed, "c028");
        if (!runFixtureCase(context, id, "c029", vector(3.2, 0.8, 0) * meter,
                function(context is Context, id is Id) { c029(context, id); }))
            failed = append(failed, "c029");
        if (!runFixtureCase(context, id, "c030", vector(3.6, 0.8, 0) * meter,
                function(context is Context, id is Id) { c030(context, id); }))
            failed = append(failed, "c030");
        if (!runFixtureCase(context, id, "c031", vector(0, 1.2, 0) * meter,
                function(context is Context, id is Id) { c031(context, id); }))
            failed = append(failed, "c031");
        if (!runFixtureCase(context, id, "c032", vector(0.4, 1.2, 0) * meter,
                function(context is Context, id is Id) { c032(context, id); }))
            failed = append(failed, "c032");
        if (!runFixtureCase(context, id, "c033", vector(0.8, 1.2, 0) * meter,
                function(context is Context, id is Id) { c033(context, id); }))
            failed = append(failed, "c033");
        if (!runFixtureCase(context, id, "c034", vector(1.2, 1.2, 0) * meter,
                function(context is Context, id is Id) { c034(context, id); }))
            failed = append(failed, "c034");
        if (!runFixtureCase(context, id, "c035", vector(1.6, 1.2, 0) * meter,
                function(context is Context, id is Id) { c035(context, id); }))
            failed = append(failed, "c035");
        if (!runFixtureCase(context, id, "c036", vector(2, 1.2, 0) * meter,
                function(context is Context, id is Id) { c036(context, id); }))
            failed = append(failed, "c036");
        if (!runFixtureCase(context, id, "c037", vector(2.4, 1.2, 0) * meter,
                function(context is Context, id is Id) { c037(context, id); }))
            failed = append(failed, "c037");
        if (!runFixtureCase(context, id, "c038", vector(2.8, 1.2, 0) * meter,
                function(context is Context, id is Id) { c038(context, id); }))
            failed = append(failed, "c038");
        if (!runFixtureCase(context, id, "c039", vector(3.2, 1.2, 0) * meter,
                function(context is Context, id is Id) { c039(context, id); }))
            failed = append(failed, "c039");
        if (!runFixtureCase(context, id, "c040", vector(3.6, 1.2, 0) * meter,
                function(context is Context, id is Id) { c040(context, id); }))
            failed = append(failed, "c040");
        if (!runFixtureCase(context, id, "c041", vector(0, 1.6, 0) * meter,
                function(context is Context, id is Id) { c041(context, id); }))
            failed = append(failed, "c041");
        if (!runFixtureCase(context, id, "c042", vector(0.4, 1.6, 0) * meter,
                function(context is Context, id is Id) { c042(context, id); }))
            failed = append(failed, "c042");
        if (!runFixtureCase(context, id, "c043", vector(0.8, 1.6, 0) * meter,
                function(context is Context, id is Id) { c043(context, id); }))
            failed = append(failed, "c043");
        if (!runFixtureCase(context, id, "c044", vector(1.2, 1.6, 0) * meter,
                function(context is Context, id is Id) { c044(context, id); }))
            failed = append(failed, "c044");
        if (!runFixtureCase(context, id, "c045", vector(1.6, 1.6, 0) * meter,
                function(context is Context, id is Id) { c045(context, id); }))
            failed = append(failed, "c045");
        if (!runFixtureCase(context, id, "c046", vector(2, 1.6, 0) * meter,
                function(context is Context, id is Id) { c046(context, id); }))
            failed = append(failed, "c046");
        if (!runFixtureCase(context, id, "c047", vector(2.4, 1.6, 0) * meter,
                function(context is Context, id is Id) { c047(context, id); }))
            failed = append(failed, "c047");
        if (!runFixtureCase(context, id, "c048", vector(2.8, 1.6, 0) * meter,
                function(context is Context, id is Id) { c048(context, id); }))
            failed = append(failed, "c048");
        if (!runFixtureCase(context, id, "c049", vector(3.2, 1.6, 0) * meter,
                function(context is Context, id is Id) { c049(context, id); }))
            failed = append(failed, "c049");
        if (!runFixtureCase(context, id, "c050", vector(3.6, 1.6, 0) * meter,
                function(context is Context, id is Id) { c050(context, id); }))
            failed = append(failed, "c050");
        if (!runFixtureCase(context, id, "c051", vector(0, 2, 0) * meter,
                function(context is Context, id is Id) { c051(context, id); }))
            failed = append(failed, "c051");
        if (!runFixtureCase(context, id, "c052", vector(0.4, 2, 0) * meter,
                function(context is Context, id is Id) { c052(context, id); }))
            failed = append(failed, "c052");
        if (!runFixtureCase(context, id, "c053", vector(0.8, 2, 0) * meter,
                function(context is Context, id is Id) { c053(context, id); }))
            failed = append(failed, "c053");
        if (!runFixtureCase(context, id, "c054", vector(1.2, 2, 0) * meter,
                function(context is Context, id is Id) { c054(context, id); }))
            failed = append(failed, "c054");
        if (!runFixtureCase(context, id, "c055", vector(1.6, 2, 0) * meter,
                function(context is Context, id is Id) { c055(context, id); }))
            failed = append(failed, "c055");
        if (!runFixtureCase(context, id, "c056", vector(2, 2, 0) * meter,
                function(context is Context, id is Id) { c056(context, id); }))
            failed = append(failed, "c056");
        if (!runFixtureCase(context, id, "c057", vector(2.4, 2, 0) * meter,
                function(context is Context, id is Id) { c057(context, id); }))
            failed = append(failed, "c057");
        if (!runFixtureCase(context, id, "c058", vector(2.8, 2, 0) * meter,
                function(context is Context, id is Id) { c058(context, id); }))
            failed = append(failed, "c058");
        if (!runFixtureCase(context, id, "c059", vector(3.2, 2, 0) * meter,
                function(context is Context, id is Id) { c059(context, id); }))
            failed = append(failed, "c059");
        if (!runFixtureCase(context, id, "c060", vector(3.6, 2, 0) * meter,
                function(context is Context, id is Id) { c060(context, id); }))
            failed = append(failed, "c060");
        if (!runFixtureCase(context, id, "c061", vector(0, 2.4, 0) * meter,
                function(context is Context, id is Id) { c061(context, id); }))
            failed = append(failed, "c061");
        if (!runFixtureCase(context, id, "c062", vector(0.4, 2.4, 0) * meter,
                function(context is Context, id is Id) { c062(context, id); }))
            failed = append(failed, "c062");
        if (!runFixtureCase(context, id, "c063", vector(0.8, 2.4, 0) * meter,
                function(context is Context, id is Id) { c063(context, id); }))
            failed = append(failed, "c063");
        if (!runFixtureCase(context, id, "c064", vector(1.2, 2.4, 0) * meter,
                function(context is Context, id is Id) { c064(context, id); }))
            failed = append(failed, "c064");
        if (!runFixtureCase(context, id, "c065", vector(1.6, 2.4, 0) * meter,
                function(context is Context, id is Id) { c065(context, id); }))
            failed = append(failed, "c065");
        if (!runFixtureCase(context, id, "c066", vector(2, 2.4, 0) * meter,
                function(context is Context, id is Id) { c066(context, id); }))
            failed = append(failed, "c066");
        if (!runFixtureCase(context, id, "c067", vector(2.4, 2.4, 0) * meter,
                function(context is Context, id is Id) { c067(context, id); }))
            failed = append(failed, "c067");
        if (!runFixtureCase(context, id, "c068", vector(2.8, 2.4, 0) * meter,
                function(context is Context, id is Id) { c068(context, id); }))
            failed = append(failed, "c068");
        if (!runFixtureCase(context, id, "c069", vector(3.2, 2.4, 0) * meter,
                function(context is Context, id is Id) { c069(context, id); }))
            failed = append(failed, "c069");
        if (!runFixtureCase(context, id, "c070", vector(3.6, 2.4, 0) * meter,
                function(context is Context, id is Id) { c070(context, id); }))
            failed = append(failed, "c070");
        if (!runFixtureCase(context, id, "c071", vector(0, 2.8, 0) * meter,
                function(context is Context, id is Id) { c071(context, id); }))
            failed = append(failed, "c071");
        if (!runFixtureCase(context, id, "c072", vector(0.4, 2.8, 0) * meter,
                function(context is Context, id is Id) { c072(context, id); }))
            failed = append(failed, "c072");
        if (!runFixtureCase(context, id, "c073", vector(0.8, 2.8, 0) * meter,
                function(context is Context, id is Id) { c073(context, id); }))
            failed = append(failed, "c073");
        if (!runFixtureCase(context, id, "c074", vector(1.2, 2.8, 0) * meter,
                function(context is Context, id is Id) { c074(context, id); }))
            failed = append(failed, "c074");
        if (!runFixtureCase(context, id, "c075", vector(1.6, 2.8, 0) * meter,
                function(context is Context, id is Id) { c075(context, id); }))
            failed = append(failed, "c075");
        if (!runFixtureCase(context, id, "c076", vector(2, 2.8, 0) * meter,
                function(context is Context, id is Id) { c076(context, id); }))
            failed = append(failed, "c076");
        if (!runFixtureCase(context, id, "c077", vector(2.4, 2.8, 0) * meter,
                function(context is Context, id is Id) { c077(context, id); }))
            failed = append(failed, "c077");
        if (!runFixtureCase(context, id, "c078", vector(2.8, 2.8, 0) * meter,
                function(context is Context, id is Id) { c078(context, id); }))
            failed = append(failed, "c078");
        if (!runFixtureCase(context, id, "c079", vector(3.2, 2.8, 0) * meter,
                function(context is Context, id is Id) { c079(context, id); }))
            failed = append(failed, "c079");
        if (!runFixtureCase(context, id, "c080", vector(3.6, 2.8, 0) * meter,
                function(context is Context, id is Id) { c080(context, id); }))
            failed = append(failed, "c080");
        if (!runFixtureCase(context, id, "c081", vector(0, 3.2, 0) * meter,
                function(context is Context, id is Id) { c081(context, id); }))
            failed = append(failed, "c081");
        if (!runFixtureCase(context, id, "c082", vector(0.4, 3.2, 0) * meter,
                function(context is Context, id is Id) { c082(context, id); }))
            failed = append(failed, "c082");
        if (!runFixtureCase(context, id, "c083", vector(0.8, 3.2, 0) * meter,
                function(context is Context, id is Id) { c083(context, id); }))
            failed = append(failed, "c083");
        if (!runFixtureCase(context, id, "c084", vector(1.2, 3.2, 0) * meter,
                function(context is Context, id is Id) { c084(context, id); }))
            failed = append(failed, "c084");
        if (!runFixtureCase(context, id, "c085", vector(1.6, 3.2, 0) * meter,
                function(context is Context, id is Id) { c085(context, id); }))
            failed = append(failed, "c085");
        if (!runFixtureCase(context, id, "c086", vector(2, 3.2, 0) * meter,
                function(context is Context, id is Id) { c086(context, id); }))
            failed = append(failed, "c086");
        if (!runFixtureCase(context, id, "c087", vector(2.4, 3.2, 0) * meter,
                function(context is Context, id is Id) { c087(context, id); }))
            failed = append(failed, "c087");
        if (!runFixtureCase(context, id, "c088", vector(2.8, 3.2, 0) * meter,
                function(context is Context, id is Id) { c088(context, id); }))
            failed = append(failed, "c088");
        if (!runFixtureCase(context, id, "c089", vector(3.2, 3.2, 0) * meter,
                function(context is Context, id is Id) { c089(context, id); }))
            failed = append(failed, "c089");
        if (!runFixtureCase(context, id, "c090", vector(3.6, 3.2, 0) * meter,
                function(context is Context, id is Id) { c090(context, id); }))
            failed = append(failed, "c090");
        if (!runFixtureCase(context, id, "c091", vector(0, 3.6, 0) * meter,
                function(context is Context, id is Id) { c091(context, id); }))
            failed = append(failed, "c091");
        if (!runFixtureCase(context, id, "c092", vector(0.4, 3.6, 0) * meter,
                function(context is Context, id is Id) { c092(context, id); }))
            failed = append(failed, "c092");
        if (!runFixtureCase(context, id, "c093", vector(0.8, 3.6, 0) * meter,
                function(context is Context, id is Id) { c093(context, id); }))
            failed = append(failed, "c093");
        if (!runFixtureCase(context, id, "c094", vector(1.2, 3.6, 0) * meter,
                function(context is Context, id is Id) { c094(context, id); }))
            failed = append(failed, "c094");
        if (!runFixtureCase(context, id, "c095", vector(1.6, 3.6, 0) * meter,
                function(context is Context, id is Id) { c095(context, id); }))
            failed = append(failed, "c095");
        if (!runFixtureCase(context, id, "c096", vector(2, 3.6, 0) * meter,
                function(context is Context, id is Id) { c096(context, id); }))
            failed = append(failed, "c096");
        if (!runFixtureCase(context, id, "c097", vector(2.4, 3.6, 0) * meter,
                function(context is Context, id is Id) { c097(context, id); }))
            failed = append(failed, "c097");
        if (!runFixtureCase(context, id, "c098", vector(2.8, 3.6, 0) * meter,
                function(context is Context, id is Id) { c098(context, id); }))
            failed = append(failed, "c098");
        if (!runFixtureCase(context, id, "c099", vector(3.2, 3.6, 0) * meter,
                function(context is Context, id is Id) { c099(context, id); }))
            failed = append(failed, "c099");
        if (!runFixtureCase(context, id, "c100", vector(3.6, 3.6, 0) * meter,
                function(context is Context, id is Id) { c100(context, id); }))
            failed = append(failed, "c100");
        if (!runFixtureCase(context, id, "c101", vector(0, 4, 0) * meter,
                function(context is Context, id is Id) { c101(context, id); }))
            failed = append(failed, "c101");
        if (!runFixtureCase(context, id, "c102", vector(0.4, 4, 0) * meter,
                function(context is Context, id is Id) { c102(context, id); }))
            failed = append(failed, "c102");
        if (!runFixtureCase(context, id, "c103", vector(0.8, 4, 0) * meter,
                function(context is Context, id is Id) { c103(context, id); }))
            failed = append(failed, "c103");
        if (!runFixtureCase(context, id, "c104", vector(1.2, 4, 0) * meter,
                function(context is Context, id is Id) { c104(context, id); }))
            failed = append(failed, "c104");
        if (!runFixtureCase(context, id, "c105", vector(1.6, 4, 0) * meter,
                function(context is Context, id is Id) { c105(context, id); }))
            failed = append(failed, "c105");
        if (!runFixtureCase(context, id, "c106", vector(2, 4, 0) * meter,
                function(context is Context, id is Id) { c106(context, id); }))
            failed = append(failed, "c106");
        println("batch-001 failed cases: " ~ toString(failed));
    });

// Builds one case, then moves its solids to the case's slot and names them "<case>.<n>". Returns false if the
// case threw or made no solid.
function runFixtureCase(context is Context, id is Id, name is string, slot is Vector, build is function) returns boolean
{
    const caseId = id + name;
    try
    {
        build(context, caseId);
    }
    catch
    {
        clearFeatureStatus(context, id, { "withDisplayData" : true });
        return false;
    }
    const solids = qBodyType(qCreatedBy(caseId, EntityType.BODY), BodyType.SOLID);
    const bodies = evaluateQuery(context, solids);
    if (size(bodies) == 0)
        return false;
    opTransform(context, caseId + "move", { "bodies" : solids, "transform" : transform(slot) });
    for (var i, body in bodies)
    {
        setProperty(context, { "entities" : body, "propertyType" : PropertyType.NAME, "value" : name ~ "." ~ (i + 1) });
    }
    return true;
}
