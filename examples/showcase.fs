FeatureScript 3112;
import(path : "onshape/std/geometry.fs", version : "3112.0");

// A showcase part for the engine: one feature that builds a desk tidy from std's features. A base plate (a sketched
// rectangle extruded, its corners filleted, its top edges rounded and its bottom edges chamfered), a vase revolved
// from a fit spline and shelled, a tower lofted from a square to a circle, a handle swept along an arc,
// pins in a circular pattern, a row of slots in a linear pattern, counterbored holes at the corners, and text
// engraved in the top, all joined into one part.
//
// Run on the engine: cargo run --release -p fs-fixtures --example run_part -- examples/showcase.fs showcase showcase

annotation { "Feature Type Name" : "Showcase" }
export const showcase = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        const top = plane(vector(0, 0, 10) * millimeter, vector(0, 0, 1), vector(1, 0, 0));
        const solid = function(name) { return qBodyType(qCreatedBy(id + name, EntityType.BODY), BodyType.SOLID); };
        const base = solid("base");
        const faceAt = function(body, p) { return qContainsPoint(qOwnedByBody(body, EntityType.FACE), p * millimeter); };
        const edgeAt = function(body, p) { return qContainsPoint(qOwnedByBody(body, EntityType.EDGE), p * millimeter); };
        const joinTo = function(name, tools)
        {
            opBoolean(context, id + name, { "tools" : qUnion([base, tools]), "operationType" : BooleanOperationType.UNION });
        };

        // ----- the base plate -----
        const plate = newSketchOnPlane(context, id + "plate", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });
        skRectangle(plate, "r", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(120, 80) * millimeter });
        skSolve(plate);
        extrude(context, id + "base", { "entities" : qSketchRegion(id + "plate"), "endBound" : BoundingType.BLIND, "depth" : 10 * millimeter });
        fillet(context, id + "corners", { "entities" : qUnion([edgeAt(base, vector(0, 0, 5)), edgeAt(base, vector(120, 0, 5)),
                        edgeAt(base, vector(0, 80, 5)), edgeAt(base, vector(120, 80, 5))]), "radius" : 10 * millimeter });
        fillet(context, id + "rounds", { "entities" : qAdjacent(faceAt(base, vector(60, 40, 10)), AdjacencyType.EDGE, EntityType.EDGE),
                    "radius" : 2 * millimeter });
        chamfer(context, id + "bevel", { "entities" : qAdjacent(faceAt(base, vector(60, 40, 0)), AdjacencyType.EDGE, EntityType.EDGE),
                    "chamferType" : ChamferType.EQUAL_OFFSETS, "width" : 1 * millimeter });

        // ----- a vase revolved from a fit spline, shelled -----
        const vase = newSketchOnPlane(context, id + "vaseProfile", { "sketchPlane" : plane(vector(28, 54, 0) * millimeter, vector(0, -1, 0), vector(1, 0, 0)) });
        skLineSegment(vase, "axis", { "start" : vector(0, 55) * millimeter, "end" : vector(0, 9) * millimeter });
        skLineSegment(vase, "foot", { "start" : vector(0, 9) * millimeter, "end" : vector(12, 9) * millimeter });
        skFitSpline(vase, "side", { "points" : [vector(12, 9) * millimeter, vector(17, 22) * millimeter, vector(10, 38) * millimeter, vector(14, 55) * millimeter] });
        skLineSegment(vase, "lip", { "start" : vector(14, 55) * millimeter, "end" : vector(0, 55) * millimeter });
        skSolve(vase);
        revolve(context, id + "vase", { "entities" : qSketchRegion(id + "vaseProfile"),
                    "axis" : sketchEntityQuery(id + "vaseProfile", EntityType.EDGE, "axis"), "fullRevolve" : true });
        shell(context, id + "vaseShell", { "entities" : faceAt(solid("vase"), vector(28, 54, 55)), "thickness" : 1.5 * millimeter });
        joinTo("joinVase", solid("vase"));

        // ----- a tower lofted from a square to a circle -----
        const foot = newSketchOnPlane(context, id + "towerFoot", { "sketchPlane" : plane(vector(0, 0, 9) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });
        skRectangle(foot, "r", { "firstCorner" : vector(82, 44) * millimeter, "secondCorner" : vector(102, 64) * millimeter });
        skSolve(foot);
        const crown = newSketchOnPlane(context, id + "towerTop", { "sketchPlane" : plane(vector(0, 0, 45) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });
        skCircle(crown, "c", { "center" : vector(92, 54) * millimeter, "radius" : 7 * millimeter });
        skSolve(crown);
        loft(context, id + "tower", { "sheetProfilesArray" : [{ "sheetProfileEntities" : qSketchRegion(id + "towerFoot") },
                        { "sheetProfileEntities" : qSketchRegion(id + "towerTop") }] });
        joinTo("joinTower", solid("tower"));

        // ----- a handle swept along an arc -----
        const path = newSketchOnPlane(context, id + "handlePath", { "sketchPlane" : plane(vector(0, 70, 0) * millimeter, vector(0, -1, 0), vector(1, 0, 0)) });
        skArc(path, "arc", { "start" : vector(50, 9) * millimeter, "mid" : vector(61, 20) * millimeter, "end" : vector(72, 9) * millimeter });
        skSolve(path);
        const ring = newSketchOnPlane(context, id + "handleRing", { "sketchPlane" : plane(vector(50, 70, 9) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });
        skCircle(ring, "c", { "center" : vector(0, 0) * millimeter, "radius" : 3 * millimeter });
        skSolve(ring);
        sweep(context, id + "handle", { "profiles" : qSketchRegion(id + "handleRing"),
                    "path" : qOwnedByBody(qBodyType(qCreatedBy(id + "handlePath", EntityType.BODY), BodyType.WIRE), EntityType.EDGE) });
        joinTo("joinHandle", solid("handle"));

        // ----- pins in a circular pattern round the tower -----
        const pin = newSketchOnPlane(context, id + "pinSketch", { "sketchPlane" : top });
        skCircle(pin, "c", { "center" : vector(108, 54) * millimeter, "radius" : 2 * millimeter });
        skSolve(pin);
        extrude(context, id + "pin", { "entities" : qSketchRegion(id + "pinSketch"), "endBound" : BoundingType.BLIND, "depth" : 5 * millimeter });
        const axis = newSketchOnPlane(context, id + "towerAxis", { "sketchPlane" : plane(vector(92, 54, 0) * millimeter, vector(0, -1, 0), vector(1, 0, 0)) });
        skLineSegment(axis, "l", { "start" : vector(0, 0) * millimeter, "end" : vector(0, 60) * millimeter });
        skSolve(axis);
        circularPattern(context, id + "pins", { "patternType" : PatternType.PART, "entities" : solid("pin"),
                    "axis" : qCreatedBy(id + "towerAxis", EntityType.EDGE), "angle" : 360 * degree, "instanceCount" : 8, "equalSpace" : true });
        joinTo("joinPins", qUnion([solid("pin"), solid("pins")]));

        // ----- a row of slots in a linear pattern -----
        const slot = newSketchOnPlane(context, id + "slotSketch", { "sketchPlane" : top });
        skLineSegment(slot, "left", { "start" : vector(14, 18) * millimeter, "end" : vector(14, 32) * millimeter });
        skArc(slot, "top", { "start" : vector(14, 32) * millimeter, "mid" : vector(16, 34) * millimeter, "end" : vector(18, 32) * millimeter });
        skLineSegment(slot, "right", { "start" : vector(18, 32) * millimeter, "end" : vector(18, 18) * millimeter });
        skArc(slot, "bottom", { "start" : vector(18, 18) * millimeter, "mid" : vector(16, 16) * millimeter, "end" : vector(14, 18) * millimeter });
        skSolve(slot);
        extrude(context, id + "slot", { "entities" : qSketchRegion(id + "slotSketch"), "endBound" : BoundingType.BLIND, "depth" : 4 * millimeter,
                    "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : base });
        const along = newSketchOnPlane(context, id + "alongX", { "sketchPlane" : top });
        skLineSegment(along, "l", { "start" : vector(0, 0) * millimeter, "end" : vector(10, 0) * millimeter });
        skSolve(along);
        linearPattern(context, id + "slots", { "patternType" : PatternType.FACE, "faces" : qCreatedBy(id + "slot", EntityType.FACE),
                    "directionOne" : qCreatedBy(id + "alongX", EntityType.EDGE), "distance" : 7 * millimeter, "instanceCount" : 3 });

        // ----- counterbored holes at the corners -----
        const holes = newSketchOnPlane(context, id + "holeCentres", { "sketchPlane" : top });
        for (var corner in [[10, 10], [110, 10], [10, 70], [110, 70]])
            skPoint(holes, "p" ~ corner[0] ~ "_" ~ corner[1], { "position" : vector(corner[0], corner[1]) * millimeter });
        skSolve(holes);
        hole(context, id + "holes", { "locations" : qOwnedByBody(qBodyType(qCreatedBy(id + "holeCentres", EntityType.BODY), BodyType.POINT), EntityType.VERTEX),
                    "scope" : base, "style" : HoleStyle.C_BORE, "holeDiameter" : 4 * millimeter, "cBoreDiameter" : 7 * millimeter,
                    "cBoreDepth" : 3 * millimeter, "endStyle" : HoleEndStyle.THROUGH });

        // ----- text engraved in the top -----
        const label = newSketchOnPlane(context, id + "label", { "sketchPlane" : top });
        skText(label, "t", { "text" : "FS ENGINE", "fontName" : "OpenSans-Bold.ttf", "firstCorner" : vector(42, 16) * millimeter,
                    "secondCorner" : vector(104, 26) * millimeter });
        skSolve(label);
        extrude(context, id + "engrave", { "entities" : qSketchRegion(id + "label"), "endBound" : BoundingType.BLIND, "depth" : 1 * millimeter,
                    "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : base });
        setProperty(context, { "entities" : base, "propertyType" : PropertyType.NAME, "value" : "Desk tidy" });
    });
