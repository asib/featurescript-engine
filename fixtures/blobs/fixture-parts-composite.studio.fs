FeatureScript 3112;
import(path : "onshape/std/geometry.fs", version : "3112.0");

// The engine's stand-in for Onshape's Part Studio tab "fixture-parts (2)", as a Feature Studio importing that tab sees
// it: its build function makes the Part Studio's one feature, the Import of fixture-parts.step its upload made
// ("Create a composite part" ticked, the composite named after the file; its parameters read from the page, fixtures/blobs/README.md).
export const build = function(configuration is map) returns Context
{
    var context = newContextWithDefaults(inch);
    importForeign(context, makeId("FQOtQ0yPeBRzwys"), { "dependsOnBlob" : true, "importAppearances" : true, "createComposite" : true, "compositeName" : "fixture-parts",
                "blobData" : { "elementId" : "aa7eee42ff6b54ecfee8ac2b", "microversionId" : "07911cc73b87f0bd2f9a105b" } as CADImportData });
    return context;
};
