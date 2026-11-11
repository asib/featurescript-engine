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
        features.FLPhbb9wuHHouVB_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    const initialGuessFLPhbb9wuHHouVB_0 = { "9ffB4A0wATzy" : [0.0, 8.0, 0.00962708572425174, -0.03879662603140021, 0.020363841205835342, -0.017730584368109703, 0.05547390878200531, -0.024752598255872726, 0.059758050783477984, -0.03879662603142084, 0.002690908537345369, 0.07505536943011187, 0.0010022736213345525, -0.0581062078910088, 0.009924232153889778, -0.030508557246526766, 0.059670833915119195, -0.03374028076144324, 0.0, 1.0], "MRvD9oGg3gnl" : [0.03616337419953197, -0.03879662603139877, 1.0, 0.0, -0.03335456608328968, 0.03335456608328968] };
                    {
                    }
                    var rxYjJGCpJASIcH_query;
                    rxYjJGCpJASIcH_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",id);
                    annotation { "Feature Name" : "Sketch 1" }
                    var sketch = newSketch(context, id + "FLPhbb9wuHHouVB_0", { "sketchPlane" : qUnion([rxYjJGCpJASIcH_query]), "disableImprinting" : false });
                    skInterpolatedSplineSegment(sketch, "9ffB4A0wATzy", { "construction" : false, "index" : "1", "geometryIsPeriodic" : true, "splinePointCount" : 4.0, ".hasHandlesInSketch" : true, ".0.hasInternalHandle" : true, ".3.hasInternalHandle" : true });
                    skLineSegment(sketch, "MRvD9oGg3gnl", { "construction" : false, "index" : "1" });
                    {
                        skConstraint(sketch, "MRvD9oGg3gnl.endSnap0", { "constraintType" : ConstraintType.HORIZONTAL, "index" : "1", "name" : "", "localFirst" : "MRvD9oGg3gnl" });
                    }
                    {
                        skConstraint(sketch, "OagOv8nKEv0g.coinc1", { "constraintType" : ConstraintType.COINCIDENT, "index" : "1", "name" : "", "localFirst" : "9ffB4A0wATzy.start", "localSecond" : "MRvD9oGg3gnl", "sketchToolType" : SketchToolType.TRIM });
                    }
                    {
                        skConstraint(sketch, "OagOv8nKEv0g.coinc2", { "constraintType" : ConstraintType.COINCIDENT, "index" : "2", "name" : "", "localFirst" : "9ffB4A0wATzy.end", "localSecond" : "MRvD9oGg3gnl", "sketchToolType" : SketchToolType.TRIM });
                    }
                    {
                        skConstraint(sketch, "OagOv8nKEv0g.startSnap", { "constraintType" : ConstraintType.COINCIDENT, "index" : "3", "name" : "", "localFirst" : "9ffB4A0wATzy.0.internal", "localSecond" : "9ffB4A0wATzy.start", "sketchToolType" : SketchToolType.TRIM });
                    }
                    {
                        skConstraint(sketch, "OagOv8nKEv0g.endSnap", { "constraintType" : ConstraintType.COINCIDENT, "index" : "4", "name" : "", "localFirst" : "9ffB4A0wATzy.3.internal", "localSecond" : "9ffB4A0wATzy.end", "sketchToolType" : SketchToolType.TRIM });
                    }
                    skSetInitialGuess(sketch, initialGuessFLPhbb9wuHHouVB_0);
                    skSolve(sketch);
                }
            };
        try(features.FLPhbb9wuHHouVB_0(id));
        features.FCJvi4vlHym7BwB_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    const initialGuessFCJvi4vlHym7BwB_0 = { "0gjWwWrrrgiU.0" : [0.03386773399271756, -0.01638185634006387] };
                    {
                        features.FGJYMiRQ0lRJoIx = function(id)
                            {
                                annotation { 'unused' : true }
                                var features = features;
                                if (true)
                                {
                                    {
                                    }
                                    var YiotMvNWYWppkV_query;
                                    YiotMvNWYWppkV_query=qCompressed(1.0,"%B5$QueryM5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FLPhbb9wuHHouVB_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$9ffB4A0wATzy",id);
                                    annotation { "Feature Name" : "Edge point" }
                                    cPoint(context, id + "FGJYMiRQ0lRJoIx", { "entities" : qUnion([YiotMvNWYWppkV_query]), "parameter" : { 'value' : try(0.5), 'expression' : "0.5" }.value });
                                }
                            };
                        try(features.FGJYMiRQ0lRJoIx(id));
                    }
                    var qUAoAfCAPUiWZv_query;
                    qUAoAfCAPUiWZv_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",id);
                    annotation { "Feature Name" : "Sketch 2" }
                    var sketch = newSketch(context, id + "FCJvi4vlHym7BwB_0", { "sketchPlane" : qUnion([qUAoAfCAPUiWZv_query]), "disableImprinting" : false });
                    skPoint(sketch, "0gjWwWrrrgiU.0", { "construction" : false, "index" : "1" });
                    {
                        var djqJhceiWAVofo_query;
                        djqJhceiWAVofo_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS6$VERTEXSb$historyTypeS8$CREATIONSb$operationIdB2$IdA1Sf.7$FGJYMiRQ0lRJoIxpointOpS9$queryTypeS5$DUMMY",id);
                        skConstraint(sketch, "0gjWwWrrrgiU.0.project", { "constraintType" : ConstraintType.PROJECTED, "index" : "1", "name" : "", "projectionType" : SketchProjectionType.USE, "localFirst" : "0gjWwWrrrgiU.0", "externalSecond" : qUnion([djqJhceiWAVofo_query]), "sketchToolType" : SketchToolType.USE });
                    }
                    skSetInitialGuess(sketch, initialGuessFCJvi4vlHym7BwB_0);
                    skSolve(sketch);
                }
            };
        try(features.FCJvi4vlHym7BwB_0(id));
        features.FXdsBkCz56RgE2Z_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    const initialGuessFXdsBkCz56RgE2Z_0 = { "aYnjAS7ao6xm.0" : [0.0, 0.0, 3.0, 6.0, 0.00962708572425174, -0.03879662603139877, 0.00992423215388983, -0.03050855724652393, 0.015335713185477608, -0.00964460985334927, 0.06028125072083035, -0.02133265660746988, 0.05967083391511925, -0.03374028076142382, 0.059758050783477984, -0.03879662603139877, 0.0, 0.0, 0.0, 0.0, 0.33127818227284794, 0.7389429398253318, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0] };
                    {
                    }
                    var cRIZviBzpPDZAL_query;
                    cRIZviBzpPDZAL_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",id);
                    annotation { "Feature Name" : "Sketch 3" }
                    var sketch = newSketch(context, id + "FXdsBkCz56RgE2Z_0", { "sketchPlane" : qUnion([cRIZviBzpPDZAL_query]), "disableImprinting" : false });
                    skSplineSegment(sketch, "aYnjAS7ao6xm.0", { "construction" : false, "index" : "1" });
                    {
                        var leFajnOtKZXwrZ_query;
                        leFajnOtKZXwrZ_query=qCompressed(1.0,"%B5$QueryM5Sa$entityTypeBa$EntityTypeS6$VERTEXSb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FLPhbb9wuHHouVB_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc.5$9ffB4A0wATzystart",id);
                        var WgaevZpznGyuSh_query;
                        WgaevZpznGyuSh_query=qCompressed(1.0,"%B5$QueryM5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FLPhbb9wuHHouVB_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$9ffB4A0wATzy",id);
                        skConstraint(sketch, "aYnjAS7ao6xm.0.start.project", { "constraintType" : ConstraintType.PROJECTED, "index" : "1", "name" : "", "projectionType" : SketchProjectionType.USE_END, "localFirst" : "aYnjAS7ao6xm.0.start", "externalVertex" : qUnion([leFajnOtKZXwrZ_query]), "externalVertexEdge" : qUnion([WgaevZpznGyuSh_query]), "sketchToolType" : SketchToolType.USE });
                    }
                    {
                        var bNJtyaqwkxCBHc_query;
                        bNJtyaqwkxCBHc_query=qCompressed(1.0,"%B5$QueryM5Sa$entityTypeBa$EntityTypeS6$VERTEXSb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FLPhbb9wuHHouVB_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc.3$9ffB4A0wATzyend",id);
                        var PBUwcDfmqUUbiW_query;
                        PBUwcDfmqUUbiW_query=qCompressed(1.0,"%B5$QueryM5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FLPhbb9wuHHouVB_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$9ffB4A0wATzy",id);
                        skConstraint(sketch, "aYnjAS7ao6xm.0.end.project", { "constraintType" : ConstraintType.PROJECTED, "index" : "2", "name" : "", "projectionType" : SketchProjectionType.USE_END, "localFirst" : "aYnjAS7ao6xm.0.end", "externalVertex" : qUnion([bNJtyaqwkxCBHc_query]), "externalVertexEdge" : qUnion([PBUwcDfmqUUbiW_query]), "sketchToolType" : SketchToolType.USE });
                    }
                    {
                        var HSjNPqgxOlQTuY_query;
                        HSjNPqgxOlQTuY_query=qCompressed(1.0,"%B5$QueryM5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FLPhbb9wuHHouVB_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$9ffB4A0wATzy",id);
                        skConstraint(sketch, "aYnjAS7ao6xm.0.project", { "constraintType" : ConstraintType.PROJECTED, "index" : "3", "name" : "", "projectionType" : SketchProjectionType.USE, "localFirst" : "aYnjAS7ao6xm.0", "externalSecond" : qUnion([HSjNPqgxOlQTuY_query]), "sketchToolType" : SketchToolType.USE });
                    }
                    skSetInitialGuess(sketch, initialGuessFXdsBkCz56RgE2Z_0);
                    skSolve(sketch);
                }
            };
        try(features.FXdsBkCz56RgE2Z_0(id));
        features.FTZCQcyfkOGh48w_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    const initialGuessFTZCQcyfkOGh48w_0 = { "f1m6jaIn00dC" : [1.0, 8.0, -0.06038931384682655, 0.04546753689646721, -0.028790252283215523, 0.048978541046381, -0.03230125829577446, 0.017379483208060265, -0.06390032172203064, 0.0208904892206192] };
                    {
                    }
                    var HdMIypqwoMMdTs_query;
                    HdMIypqwoMMdTs_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",id);
                    annotation { "Feature Name" : "Sketch 4" }
                    var sketch = newSketch(context, id + "FTZCQcyfkOGh48w_0", { "sketchPlane" : qUnion([HdMIypqwoMMdTs_query]), "disableImprinting" : false });
                    skInterpolatedSpline(sketch, "f1m6jaIn00dC", { "construction" : false, "index" : "1", "splinePointParamCount" : 0.0, "splinePointCount" : 4.0 });
                    skSetInitialGuess(sketch, initialGuessFTZCQcyfkOGh48w_0);
                    skSolve(sketch);
                }
            };
        try(features.FTZCQcyfkOGh48w_0(id));
        features.FQhQQJN7nZIqYGl_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    const initialGuessFQhQQJN7nZIqYGl_0 = { "5DmMJN1TSQ0F.0.0" : [1.0, 0.0, 3.0, 7.0, -0.06038931384682655, 0.04546753689646721, -0.06792594208603091, 0.03856097043728966, -0.07164026563841551, 0.015616104855053138, -0.025081604714599345, 0.007322732797865395, -0.019962612657692558, 0.059035290595235504, -0.051860511423064644, 0.053283331823234734, -0.06038931384682655, 0.04546753689646721, -0.25748855266566917, 0.0, 0.0, 0.0, 0.2275343478335582, 0.485022908828604, 0.7425114473343308, 1.0, 1.0, 1.0, 1.2275343478335583] };
                    {
                    }
                    var WVuFuqTCINyqNA_query;
                    WVuFuqTCINyqNA_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",id);
                    annotation { "Feature Name" : "Sketch 5" }
                    var sketch = newSketch(context, id + "FQhQQJN7nZIqYGl_0", { "sketchPlane" : qUnion([WVuFuqTCINyqNA_query]), "disableImprinting" : false });
                    skSpline(sketch, "5DmMJN1TSQ0F.0.0", { "construction" : false, "index" : "1" });
                    {
                        var BGnvtHPlOsqjMt_query;
                        BGnvtHPlOsqjMt_query=qCompressed(1.0,"%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS8$TOPOLOGYS8$entitiesA1A2C0M5Sb$derivedFromC0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FTZCQcyfkOGh48w_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$f1m6jaIn00dCR5R6R7R8R9CcA1S-d.7$imprintRcS7$IMPRINTD-1R5C7S4$FACER7R8R9R12RcR13",id);
                        var IpMCFSJCoxGhwO_query;
                        IpMCFSJCoxGhwO_query=qCompressed(1.0,"%B5$QueryM5Sb$derivedFromC0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FTZCQcyfkOGh48w_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$f1m6jaIn00dCR1R2R3R4R5C8A1S-9.7$imprintR8S7$IMPRINT",id);
                        skConstraint(sketch, "5DmMJN1TSQ0F.0.0.project", { "constraintType" : ConstraintType.PROJECTED, "index" : "1", "name" : "", "projectionType" : SketchProjectionType.USE, "localFirst" : "5DmMJN1TSQ0F.0.0", "externalSecond" : qUnion([qCoEdge(BGnvtHPlOsqjMt_query, IpMCFSJCoxGhwO_query)]), "sketchToolType" : SketchToolType.USE });
                    }
                    skSetInitialGuess(sketch, initialGuessFQhQQJN7nZIqYGl_0);
                    skSolve(sketch);
                }
            };
        try(features.FQhQQJN7nZIqYGl_0(id));
        return context;
    }, inch, {});
