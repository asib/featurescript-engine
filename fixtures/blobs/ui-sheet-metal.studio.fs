FeatureScript 3112;
import(path : "onshape/std/geometry.fs", version : "3112.0");

annotation { "Default Units" : ["cubicInch", "degree", "degreePerSecond", "footPoundForce", "hertz", "inch", "inchPerSecondSquared", "inchPound", "pound", "poundForce", "poundPerCubicInch", "poundPerSquareInch", "second", "squareInch"] }
export function main()
{
    return build({});
}

export function build()
{
    return buildPrivate({});
}

export function build(configuration is map)
{
    return buildPrivate(configuration);
}

const buildPrivate = definePartStudio(function(context is Context, configuration is map, lookup is function)
    precondition
    {
    }
    {
        const id is Id = newId();
        annotation { 'unused' : true }
        var features = {};
        features.FV9VYq8K2CHkGui_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    const initialGuessFV9VYq8K2CHkGui_0 = { "60W3tZLQONmH.bottom" : [0.03440786898136139, 0.03493451699614525, 1.0, 0.0, -0.024577047675848007, 0.024577047675848007], "60W3tZLQONmH.top" : [0.03440786898136139, -0.010708571411669254, 1.0, 0.0, -0.024577047675848007, 0.024577047675848007], "60W3tZLQONmH.left" : [0.009830821305513382, 0.012112972792237997, 0.0, -1.0, -0.02282154420390725, 0.02282154420390725], "60W3tZLQONmH.right" : [0.058984916657209396, 0.012112972792237997, 0.0, -1.0, -0.02282154420390725, 0.02282154420390725] };
                    {
                    }
                    var IqdKrHrBjVtydY_query;
                    IqdKrHrBjVtydY_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",id);
                    annotation { "Feature Name" : "Sketch 1" }
                    var sketch = newSketch(context, id + "FV9VYq8K2CHkGui_0", { "sketchPlane" : qUnion([IqdKrHrBjVtydY_query]), "disableImprinting" : false });
                    skLineSegment(sketch, "60W3tZLQONmH.bottom", { "construction" : false, "index" : "1" });
                    skLineSegment(sketch, "60W3tZLQONmH.top", { "construction" : false, "index" : "2" });
                    skLineSegment(sketch, "60W3tZLQONmH.left", { "construction" : false, "index" : "3" });
                    skLineSegment(sketch, "60W3tZLQONmH.right", { "construction" : false, "index" : "4" });
                    {
                        skConstraint(sketch, "60W3tZLQONmH.perpendicular", { "constraintType" : ConstraintType.PERPENDICULAR, "index" : "1", "name" : "", "localFirst" : "60W3tZLQONmH.top", "localSecond" : "60W3tZLQONmH.left" });
                    }
                    {
                        skConstraint(sketch, "60W3tZLQONmH.parallel.1", { "constraintType" : ConstraintType.PARALLEL, "index" : "1", "name" : "", "localFirst" : "60W3tZLQONmH.bottom", "localSecond" : "60W3tZLQONmH.top" });
                    }
                    {
                        skConstraint(sketch, "60W3tZLQONmH.parallel.2", { "constraintType" : ConstraintType.PARALLEL, "index" : "2", "name" : "", "localFirst" : "60W3tZLQONmH.left", "localSecond" : "60W3tZLQONmH.right" });
                    }
                    {
                        skConstraint(sketch, "60W3tZLQONmH.horizontal", { "constraintType" : ConstraintType.HORIZONTAL, "index" : "1", "name" : "", "localFirst" : "60W3tZLQONmH.top" });
                    }
                    {
                        skConstraint(sketch, "60W3tZLQONmH.corner0", { "constraintType" : ConstraintType.COINCIDENT, "index" : "1", "name" : "", "localFirst" : "60W3tZLQONmH.bottom.start", "localSecond" : "60W3tZLQONmH.left.start" });
                    }
                    {
                        skConstraint(sketch, "60W3tZLQONmH.corner1", { "constraintType" : ConstraintType.COINCIDENT, "index" : "2", "name" : "", "localFirst" : "60W3tZLQONmH.bottom.end", "localSecond" : "60W3tZLQONmH.right.start" });
                    }
                    {
                        skConstraint(sketch, "60W3tZLQONmH.corner2", { "constraintType" : ConstraintType.COINCIDENT, "index" : "3", "name" : "", "localFirst" : "60W3tZLQONmH.top.start", "localSecond" : "60W3tZLQONmH.left.end" });
                    }
                    {
                        skConstraint(sketch, "60W3tZLQONmH.corner3", { "constraintType" : ConstraintType.COINCIDENT, "index" : "4", "name" : "", "localFirst" : "60W3tZLQONmH.top.end", "localSecond" : "60W3tZLQONmH.right.end" });
                    }
                    skSetInitialGuess(sketch, initialGuessFV9VYq8K2CHkGui_0);
                    skSolve(sketch);
                }
            };
        try(features.FV9VYq8K2CHkGui_0(id));
        features.FaJl58oS1lCJ4yw_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    {
                    }
                    var RwGkORUgWaIUbX_query;
                    RwGkORUgWaIUbX_query=qCompressed(1.0,"&217$eJyNkF1vwiAUhv/M2aVG+mH1sh+ojdoqEBd3Y9rClLjaSnFL//2wLmaLu/CGwHl43sPhJXBhfRGqXboUWcBlk5W53F8yLatTlOnMR0vrkbC2FnQELF2li3S6NVtx0lJL0fiOb4UDk5YDF0p+Cj5RVdlVstul9ioHGeD7gTqAoyk2ykE2ulLtT3xIsM/iNDGgqoXqOsc8sCDmPqII9Ycw2Yw32/NoboWz4/Qid4MvqURa0zGcr1N1QRzoHLNwtsMJi9mWCmiOQheH2wNiTgsTNBy82vptsU6TcpZXWlclccmQeGRExmFh2vV43wNZ1kqeNCmoB/FyReKERT10m5g43Xq3SE4KwomgPWT3bdBV/Yshy0BkR8+4DnyId/0oE5Q9Ybug5P7wr26U0DOfP/FD/Jd9A44PpcI=",id);
                    annotation { "Feature Name" : "Sheet metal model 1" }
                    sheetMetalStart(context, id + "FaJl58oS1lCJ4yw_0", { "initEntities" : qUnion([]), "process" : SMProcessType.THICKEN, "partToConvert" : qUnion([]), "facesToExclude" : qUnion([]), "sketchCurves" : qUnion([]), "bendArcs" : qUnion([]), "endBound" : SMExtrudeBoundingType.BLIND, "oppositeExtrudeDirection" : false, "depth" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "endBoundEntityFace" : qUnion([]), "endBoundEntityBody" : qUnion([]), "endBoundEntityVertex" : qUnion([]), "hasOffset" : false, "offsetDistance" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "offsetOppositeDirection" : false, "symmetric" : false, "hasSecondDirection" : false, "secondDirectionBound" : SMExtrudeBoundingType.BLIND, "secondDirectionOppositeExtrudeDirection" : true, "secondDirectionDepth" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "secondDirectionBoundEntityFace" : qUnion([]), "secondDirectionBoundEntityBody" : qUnion([]), "secondDirectionBoundEntityVertex" : qUnion([]), "hasSecondDirectionOffset" : false, "secondDirectionOffsetDistance" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "secondDirectionOffsetOppositeDirection" : false, "regions" : qUnion([RwGkORUgWaIUbX_query]), "tangentPropagation" : false, "bends" : qUnion([]), "clearance" : { 'value' : try(0 * inch), 'expression' : "0 in" }.value, "bendsIncluded" : false, "keepInputParts" : false, "thickness" : { 'value' : try(0.0625 * inch), 'expression' : "0.0625 in" }.value, "oppositeDirection" : false, "radius" : { 'value' : try(0.09 * inch), 'expression' : "0.09 in" }.value, "flipDirectionUp" : false, "bendCalculationType" : SMBendCalculationType.K_FACTOR, "kFactor" : { 'value' : try(0.45), 'expression' : "0.45" }.value, "kFactorRolled" : { 'value' : try(0.5), 'expression' : "0.5" }.value, "minimalClearance" : { 'value' : try(0.001 * inch), 'expression' : "0.001 in" }.value, "defaultCornerStyle" : SMCornerStrategyType.SIMPLE, "defaultCornerReliefScale" : { 'value' : try(1.5), 'expression' : "1.5" }.value, "defaultRoundReliefDiameter" : { 'value' : try(0.25 * inch), 'expression' : "0.25 in" }.value, "defaultSquareReliefWidth" : { 'value' : try(0.25 * inch), 'expression' : "0.25 in" }.value, "defaultBendReliefStyle" : SMBendStrategyType.OBROUND, "defaultBendReliefDepthScale" : { 'value' : try(2), 'expression' : "2" }.value, "defaultBendReliefScale" : { 'value' : try(1.0625), 'expression' : "1.0625" }.value });
                }
            };
        try(features.FaJl58oS1lCJ4yw_0(id));
        return context;
    }, inch, {});
