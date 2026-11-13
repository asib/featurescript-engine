// Generates fixture batch 152: routing curves through points read from CSV data, as the feature's editing logic reads
// them when its Process button is pressed (std's routingCurveEditLogic), then the feature built from what it gives:
// a polyline with bends through millimetre rows, a spline through rows with a label column in centimetres, rows with
// one repeated (skipped), and rows too short (refused).
//
// Usage: node tools/fixtures/gen-batch-152.mjs
//
// Writes fixtures/batch-152/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch } from './lib.mjs';

const helpers = `
function routingPoint(index is number, position is Vector) returns map
{
    return { "index" : index, "referenceType" : ReferenceType.ORIGIN, "vertex" : qNothing(), "curve" : qNothing(),
             "curveParameter" : 0, "alignCSys" : false, "flipCSys" : false, "dx" : position[0], "dy" : position[1],
             "dz" : position[2], "rotationMatrix" : identityMatrix(3), "overrideBendRadius" : false,
             "pointBendRadius" : 0 * meter, "hasDerivative" : false, "derivativeAlignment" : DerivativeAlignment.X,
             "magnitude" : 1, "oppositeDirection" : false, "derivativeDirection" : qNothing(), "arrayAdded" : false };
}

// A routing curve's definition with its CSV input set: the editing logic's Process button read, then the feature.
function routeFromCsv(context is Context, id is Id, rows is array, startColumn is number, unit is LengthUnitNames,
    curveType is RoutingCurveType, bendRadius is ValueWithUnits)
{
    var definition = { "preselectedEntities" : qNothing(), "curveType" : curveType, "step" : CurveStep.POINTS,
            "inputType" : InputType.CSV, "inputVertices" : qNothing(), "inputCurve" : qNothing(), "samples" : 10,
            "inputData" : { "csvData" : rows } as TableData, "startColumn" : startColumn, "csvUnit" : unit,
            "overwritePoints" : true, "isPeriodic" : false, "curveBaseCSys" : qNothing(),
            "pointsEditType" : PointsEditType.SINGLE, "pointIndex" : 0, "addPointOnAxisDrag" : false, "showCSys" : false,
            "keepPoints" : false, "hasTargetLength" : false, "showLRA" : false, "hiddenReferences" : qNothing(), "curveLength" : 0 * meter, "bendRadius" : bendRadius,
            "points" : [routingPoint(0, vector(0, 0, 0) * meter)] };
    const old = definition;
    definition = routingCurveEditLogic(context, id + "route1", old, definition, false, {}, qNothing(), "processInputs");
    routingCurve(context, id + "route1", definition);
    return { "points" : size(definition.points) };
}
`;
const batch = new Batch({ number: 152, title: 'routing curves through points read from CSV data', helpers });

batch.add('routing-curve', 'a polyline with 2 mm bends through four millimetre rows (0, 0, 0), (30, 0, 0), (30, 20, 0), (30, 20, 15)', [
  `return routeFromCsv(context, id, [[0, 0, 0], [30, 0, 0], [30, 20, 0], [30, 20, 15]], 0, LengthUnitNames.Millimeter, RoutingCurveType.POLYLINE, 2 * millimeter);`,
]);
batch.add('routing-curve', 'a spline through rows with a label column, in centimetres: (0, 0, 0), (2, 1, 0), (4, 0, 1), (6, -1, 1)', [
  `return routeFromCsv(context, id, [["a", 0, 0, 0], ["b", 2, 1, 0], ["c", 4, 0, 1], ["d", 6, -1, 1]], 1, LengthUnitNames.Centimeter, RoutingCurveType.INTERPOLATED_SPLINE, 0 * meter);`,
]);
batch.add('routing-curve', 'a polyline through millimetre rows with the second repeated (it is read once)', [
  `return routeFromCsv(context, id, [[0, 0, 0], [20, 0, 0], [20, 0, 0], [20, 15, 10]], 0, LengthUnitNames.Millimeter, RoutingCurveType.POLYLINE, 1 * millimeter);`,
]);
batch.add('routing-curve', 'rows of two numbers each, too few for a point from column 0', [
  `return routeFromCsv(context, id, [[0, 0], [20, 0], [20, 15]], 0, LengthUnitNames.Millimeter, RoutingCurveType.POLYLINE, 1 * millimeter);`,
]);

batch.write('tools/fixtures/gen-batch-152.mjs');
