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
        features.FeUNWBXWR3P4AUC_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    const initialGuessFeUNWBXWR3P4AUC_0 = { "8MiwA9l7j4dg" : [0.0, 8.0, -0.06390032172203064, 0.017379483208060265, -0.04283428192138672, 0.04546753689646721, -0.018257232382893562, 0.024401497095823288, -0.012990722432732582, 0.059511564671993256, 0.05483101595637745, 0.13472899182962034, -0.0017886370871079706, 0.15930826070056148, -0.05773765576647041, 0.032522185868100656, -0.012788569335406676, 0.04150642625551727, 0.0, 1.0], "K4ASsjGTCmiS" : [1.0, 8.0, 0.020363841205835342, -0.017730584368109703, 0.05547390878200531, -0.024752598255872726, 0.05020739883184433, -0.059862665832042694, 0.013341828249394894, -0.05284065380692482], "Y2EsOpdVRahG" : [1.0, 0.0, 0.0, 3.0, 4.0, -0.06390032172203064, -0.06688467413187027, -0.05336729809641838, -0.024752598255872726, -0.028790252283215523, -0.059862665832042694, -0.011235219426453114, -0.017730584368109703, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0], "ogOM4F6dF4NB" : [0.03616337291896343, 0.06179371848702431, 0.9936389891263454, -0.11261242954476415, -0.026501125561309628, 0.026501125561309628], "jtR065vsGkZ5" : [0.020363841205835342, 0.03493451699614525, 1.0, 0.0, 0.011431010258589452, -1.0], "rPK5drqJiVDG" : [0.0581071674823761, 0.025279248133301735, 1.0, 0.0, 0.01144448263519075, -1.0, -6.206413478323447, -3.0648208247336544], "h5uonviICl9I" : [0.07022013515233994, 0.04546753689646721] };
                    {
                    }
                    var NhuPOATUzeXqro_query;
                    NhuPOATUzeXqro_query=qCompressed(1.0,"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",id);
                    annotation { "Feature Name" : "Sketch 1" }
                    var sketch = newSketch(context, id + "FeUNWBXWR3P4AUC_0", { "sketchPlane" : qUnion([NhuPOATUzeXqro_query]), "disableImprinting" : false });
                    skInterpolatedSplineSegment(sketch, "8MiwA9l7j4dg", { "construction" : false, "index" : "1", "geometryIsPeriodic" : false, ".hasHandlesInSketch" : true, ".0.hasInternalHandle" : true, ".3.hasInternalHandle" : true, ".1.hasInternalHandle" : false, ".2.hasInternalHandle" : false, "splinePointParamCount" : 0.0, "splinePointCount" : 4.0 });
                    skInterpolatedSpline(sketch, "K4ASsjGTCmiS", { "construction" : false, "index" : "2", "splinePointParamCount" : 0.0, "splinePointCount" : 4.0 });
                    skBezier(sketch, "Y2EsOpdVRahG", { "construction" : false, "index" : "3", "geometryIsPeriodic" : false });
                    skLineSegment(sketch, "ogOM4F6dF4NB", { "construction" : false, "index" : "1" });
                    skCircle(sketch, "jtR065vsGkZ5", { "construction" : false, "index" : "1" });
                    skArc(sketch, "rPK5drqJiVDG", { "construction" : false, "index" : "2" });
                    skPoint(sketch, "h5uonviICl9I", { "construction" : false, "index" : "1" });
                    {
                        skConstraint(sketch, "8MiwA9l7j4dg.startSnap", { "constraintType" : ConstraintType.COINCIDENT, "index" : "1", "name" : "", "localFirst" : "8MiwA9l7j4dg.start", "localSecond" : "8MiwA9l7j4dg.0.internal" });
                    }
                    {
                        skConstraint(sketch, "8MiwA9l7j4dg.endSnap", { "constraintType" : ConstraintType.COINCIDENT, "index" : "2", "name" : "", "localFirst" : "8MiwA9l7j4dg.end", "localSecond" : "8MiwA9l7j4dg.3.internal" });
                    }
                    {
                        skConstraint(sketch, "Y2EsOpdVRahG.startSnap", { "constraintType" : ConstraintType.COINCIDENT, "index" : "3", "name" : "", "localFirst" : "Y2EsOpdVRahG.start", "localSecond" : "Y2EsOpdVRahG.0.internal" });
                    }
                    {
                        skConstraint(sketch, "Y2EsOpdVRahG.endSnap", { "constraintType" : ConstraintType.COINCIDENT, "index" : "4", "name" : "", "localFirst" : "Y2EsOpdVRahG.end", "localSecond" : "Y2EsOpdVRahG.3.internal" });
                    }
                    {
                        skConstraint(sketch, "Y2EsOpdVRahG.bezierDegree", { "constraintType" : ConstraintType.BEZIER_DEGREE, "index" : "1", "name" : "", "localFirst" : "Y2EsOpdVRahG", "bezierDegree" : { 'value' : try(roundWithinTolerance(3)), 'expression' : "" }.value, "labelDistance" : 0.029492980479962433 * meter, "labelAngle" : 2.504043916303018 * radian });
                    }
                    {
                        skConstraint(sketch, "rPK5drqJiVDG.mid1", { "constraintType" : ConstraintType.MIDPOINT, "index" : "1", "name" : "", "localMidpoint" : "rPK5drqJiVDG.center", "localEntity1" : "rPK5drqJiVDG.start", "localEntity2" : "rPK5drqJiVDG.end" });
                    }
                    skSetInitialGuess(sketch, initialGuessFeUNWBXWR3P4AUC_0);
                    skSolve(sketch);
                }
            };
        try(features.FeUNWBXWR3P4AUC_0(id));
        features.FbGO9VXCeYzrs3h_0 = function(id)
            {
                annotation { 'unused' : true }
                var features = features;
                if (true)
                {
                    {
                    }
                    var ufGUNhuRDXNJqr_query;
                    ufGUNhuRDXNJqr_query=qCompressed(1.0,"%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS8$TOPOLOGYS8$entitiesA1A2C0M5Sb$derivedFromC0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FeUNWBXWR3P4AUC_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$jtR065vsGkZ5R5R6R7R8R9CcA1S-d.7$imprintRcS7$IMPRINTD1R5C7S4$FACER7R8R9R12RcR13",id);
                    var WMIiHwPgyLMIwy_query;
                    WMIiHwPgyLMIwy_query=qCompressed(1.0,"%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS8$TOPOLOGYS8$entitiesA1A2C0M5Sb$derivedFromC0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FeUNWBXWR3P4AUC_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdSc$K4ASsjGTCmiSR5R6R7R8R9CcA1S-d.7$imprintRcS7$IMPRINTD-1R5C7S4$FACER7R8R9R12RcR13",id);
                    annotation { "Feature Name" : "Extrude 1" }
                    extrude(context, id + "FbGO9VXCeYzrs3h_0", { "domain" : OperationDomain.MODEL, "bodyType" : ExtendedToolBodyType.SOLID, "operationType" : NewBodyOperationType.NEW, "surfaceOperationType" : NewSurfaceOperationType.NEW, "flatOperationType" : FlatOperationType.REMOVE, "entities" : qUnion([ufGUNhuRDXNJqr_query, WMIiHwPgyLMIwy_query]), "surfaceEntities" : qUnion([]), "wallShape" : qUnion([]), "midplane" : false, "thickness1" : { 'value' : try(0.25 * inch), 'expression' : "0.25 in" }.value, "flipWall" : false, "thickness2" : { 'value' : try(0 * inch), 'expression' : "0 in" }.value, "thickness" : { 'value' : try(0.25 * inch), 'expression' : "0.25 in" }.value, "endBound" : BoundingType.BLIND, "oppositeDirection" : false, "depth" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "endBoundEntityFace" : qUnion([]), "endBoundEntityBody" : qUnion([]), "endBoundEntityVertex" : qUnion([]), "hasOffset" : false, "offsetDistance" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "offsetOppositeDirection" : false, "hasExtrudeDirection" : false, "extrudeDirection" : qUnion([]), "startOffset" : false, "startOffsetBound" : StartOffsetType.BLIND, "startOffsetDistance" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "startOffsetOppositeDirection" : false, "startOffsetEntity" : qUnion([]), "symmetric" : false, "hasDraft" : false, "draftAngle" : { 'value' : try(3 * degree), 'expression' : "3 deg" }.value, "draftPullDirection" : false, "hasSecondDirection" : false, "secondDirectionBound" : BoundingType.BLIND, "secondDirectionOppositeDirection" : true, "secondDirectionDepth" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "secondDirectionBoundEntityFace" : qUnion([]), "secondDirectionBoundEntityBody" : qUnion([]), "secondDirectionBoundEntityVertex" : qUnion([]), "hasSecondDirectionOffset" : false, "secondDirectionOffsetDistance" : { 'value' : try(1 * inch), 'expression' : "1 in" }.value, "secondDirectionOffsetOppositeDirection" : false, "hasSecondDirectionDraft" : false, "secondDirectionDraftAngle" : { 'value' : try(3 * degree), 'expression' : "3 deg" }.value, "secondDirectionDraftPullDirection" : false, "defaultScope" : false, "booleanScope" : qUnion([]), "defaultSurfaceScope" : true, "booleanSurfaceScope" : qUnion([]) });
                }
            };
        try(features.FbGO9VXCeYzrs3h_0(id));
        return context;
    }, inch, {});
