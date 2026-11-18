FeatureScript 3112;
import(path : "onshape/std/geometry.fs", version : "3112.0");

// The WebAssembly engine's smoke test: a 20 mm cube, its top edges rounded.
annotation { "Feature Type Name" : "Rounded cube" }
export const roundedCube = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        const s = newSketchOnPlane(context, id + "s", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });
        skRectangle(s, "r", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(20, 20) * millimeter });
        skSolve(s);
        extrude(context, id + "cube", { "entities" : qSketchRegion(id + "s"), "endBound" : BoundingType.BLIND, "depth" : 20 * millimeter });
        fillet(context, id + "round", { "entities" : qAdjacent(qContainsPoint(qCreatedBy(id + "cube", EntityType.FACE), vector(10, 10, 20) * millimeter),
                        AdjacencyType.EDGE, EntityType.EDGE), "radius" : 3 * millimeter });
    });
