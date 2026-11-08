// Generates fixture batch 038: tags for PCB holes and standard content, and more curve trims and extensions: curves
// extended at their start, a fit spline trimmed by a plane, an arc trimmed by a line, and an elliptical arc extended.
//
// Usage: node tools/fixtures/gen-batch-038.mjs
//
// Writes fixtures/batch-038/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, createdBy, line, mm, planeBody, sketch, solidsOf, solve, vertexAt, PLANES } from './lib.mjs';

const helpers = `
// Each edge of a case's wires: its curve type and length, and a B-spline's degree, knots and poles.
function wireProbe(context is Context, wires is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(wires, EntityType.EDGE)))
    {
        const c = evCurveDefinition(context, { "edge" : edge });
        var entry = { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) };
        if (c is BSplineCurve)
        {
            var points = [];
            for (var p in c.controlPoints)
                points = append(points, [roundToPrecision(p[0] / millimeter, 6), roundToPrecision(p[1] / millimeter, 6), roundToPrecision(p[2] / millimeter, 6)]);
            var knots = [];
            for (var k in c.knots)
                knots = append(knots, roundToPrecision(k, 9));
            entry.bspline = { "degree" : c.degree, "rational" : c.isRational, "points" : points, "knots" : knots };
        }
        else if (c is Circle)
            entry.circle = { "radius" : roundToPrecision(c.radius / millimeter, 6) };
        else if (c is Line)
            entry.line = true;
        out = append(out, entry);
    }
    return out;
}

// The attribute of a name on each of a body's faces that has it, by the face's area.
function faceAttributes(context is Context, body is Query, name is string) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        const attribute = getAttribute(context, { "entity" : face, "name" : name });
        if (attribute != undefined)
        {
            // Its queries and ids hold transient ids and the feature's id, which differ between kernels.
            var stable = {};
            for (var key, value in attribute)
            {
                if (!(value is Query) && key != "holeId")
                    stable[key] = value;
            }
            out = append(out, { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6), "attribute" : stable });
        }
    }
    return out;
}
`;

const batch = new Batch({ number: 38, title: 'tags for PCB holes and standard content; curves extended at their start, trimmed by planes and lines; elliptical arcs extended', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const curve1 = createdBy('curve1');
const probe = returns({ edges: `wireProbe(context, ${curve1})` });

// ----- tags -----

batch.add('tag', 'a block tagged as standard content', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('tag', 'tag1', { tagPurpose: 'TagPurpose.STANDARD_CONTENT', standardContentPart: base }),
  returns({ attribute: `getAttribute(context, { "entity" : ${base}, "name" : "bodyPurposeAttribute" })` }),
]);
const holeFace = (x, y) => `qContainsPoint(qOwnedByBody(${base}, EntityType.FACE), vector(${x + 2}, ${y}, 1) * millimeter)`;
const hole = (x, y, designator, purpose) =>
  `{ "interiorFaces" : ${holeFace(x, y)}, "holeId" : "", "designator" : "${designator}", "platedPurpose" : PcbPlatedHolePurpose.${purpose}, "nonPlatedPurpose" : PcbNonPlatedHolePurpose.NONE, "customPurpose" : "", "componentDesignator" : "", "holeOwner" : PcbHoleOwner.UNOWNED }`;
batch.add('tag', 'a 2 mm plate\'s two 4 mm holes tagged as plated PCB holes, a via and a thermal via', [
  box('base', [0, 0, 0], [40, 30, 2]),
  sketch('holes', PLANES.xy),
  `skCircle(holes, "h1", { "center" : vector(10, 15) * millimeter, "radius" : 2 * millimeter });`,
  `skCircle(holes, "h2", { "center" : vector(30, 15) * millimeter, "radius" : 2 * millimeter });`,
  solve('holes'),
  call('extrude', 'cut', {
    entities: `qSketchRegion(id + "holes")`, endBound: 'BoundingType.THROUGH_ALL', operationType: 'NewBodyOperationType.REMOVE',
    defaultScope: 'false', booleanScope: base,
  }),
  call('tag', 'tag1', {
    tagPurpose: 'TagPurpose.PCB_HOLES', pcbEntityType: 'PcbEntityType.HOLE', plated: 'true', basePlane: 'qTopPlane(EntityType.FACE)',
    holes: `[${hole(10, 15, 'H1', 'VIA')}, ${hole(30, 15, 'H2', 'THERMAL_VIA')}]`,
  }),
  returns({ faces: `faceAttributes(context, ${base}, "pcbStudioHole")` }),
]);

// ----- curves extended at their start -----

const quarter = [
  sketch('s1', PLANES.xy), arc('s1', 'arc1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('s1'),
  call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
];
const extendStart = (shape) =>
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, [20, 0, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: `CurveExtensionShape.${shape}`,
  });
batch.add('trim-curve', 'a quarter circle extended 10 mm at its start, straight on', [quarter, extendStart('TANGENT'), probe]);
batch.add('trim-curve', 'a quarter circle extended 10 mm at its start, curving on', [quarter, extendStart('CURVATURE'), probe]);
const spline = [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fitSpline', 'curve1', { vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 10])}, ${vertexAt(base, [30, 20, 0])}])` }),
];
batch.add('trim-curve', 'a fit spline through three corners of a block extended 10 mm at its start, straight on', [
  spline,
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, [0, 0, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: 'CurveExtensionShape.TANGENT',
  }),
  probe,
]);

// ----- trims -----

batch.add('trim-curve', 'a fit spline through three corners of a block trimmed by the plane x = 15, keeping its end', [
  spline, planeBody('limit', [15, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.TRIM', wires: curve1, trimTo: createdBy('limit', 'FACE'), helpPoint: vertexAt(curve1, [30, 20, 0]),
    flipHeuristics: 'false',
  }),
  probe,
]);
batch.add('trim-curve', 'a quarter circle trimmed by a line from (0, 0) to (20, 10), keeping its end', [
  quarter, sketch('s2', PLANES.xy), line('s2', 'line1', [0, 0], [20, 10]), solve('s2'),
  call('compositeCurve', 'limit', { edges: wireEdges('s2') }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.TRIM', wires: curve1, trimTo: createdBy('limit'), helpPoint: vertexAt(curve1, [0, 20, 0]),
    flipHeuristics: 'false',
  }),
  probe,
]);

// ----- an elliptical arc -----

batch.add('trim-curve', 'a quarter of a 20 × 10 ellipse extended 5 mm at its end, straight on', [
  sketch('s1', PLANES.xy),
  `skEllipticalArc(s1, "e1", { "center" : vector(0, 0) * millimeter, "majorAxis" : vector(1, 0), "majorRadius" : 20 * millimeter, "minorRadius" : 10 * millimeter, "startParameter" : 0, "endParameter" : 0.25 });`,
  solve('s1'), call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, [0, 10, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(5), extensionShape: 'CurveExtensionShape.TANGENT',
  }),
  probe,
]);

// More elliptical arcs, extended 5 mm straight on, to find how Onshape makes them B-splines: [major, minor, start,
// end] (parameters as fractions of a turn).
const ellipseArc = (major, minor, t0, t1) => {
  const at = (t) => [major * Math.cos(2 * Math.PI * t), minor * Math.sin(2 * Math.PI * t), 0];
  return [
    sketch('s1', PLANES.xy),
    `skEllipticalArc(s1, "e1", { "center" : vector(0, 0) * millimeter, "majorAxis" : vector(1, 0), "majorRadius" : ${major} * millimeter, "minorRadius" : ${minor} * millimeter, "startParameter" : ${t0}, "endParameter" : ${t1} });`,
    solve('s1'), call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
    call('trimCurve', 'trim1', {
      moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, at(t1)),
      endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(5), extensionShape: 'CurveExtensionShape.TANGENT',
    }),
    probe,
  ];
};
for (const [major, minor, t0, t1] of [[20, 10, 0, 0.5], [20, 10, 0.25, 0.5], [30, 10, 0, 0.25], [15, 10, 0, 0.25], [40, 20, 0, 0.25], [10, 5, 0, 0.25]]) {
  batch.add('trim-curve', `an elliptical arc ${major} × ${minor} from ${t0} to ${t1} of a turn extended 5 mm at its end, straight on`, [ellipseArc(major, minor, t0, t1)]);
}

batch.write('tools/fixtures/gen-batch-038.mjs');
