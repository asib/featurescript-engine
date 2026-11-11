// Generates fixture batch 101: the derivative of a face's second fundamental form (evFaceCurvatureDerivative, which
// batch 100 c008 gave a direction square to the face: NO_TANGENT_PLANE) along its tangent plane's axes on a
// cylinder, a torus, a sphere and a cone, and along a direction leaning out of the plane; the internal evaluations
// evMaxTolerance and evFaults alone (batch 100 c007 failed on evTolerances); and a cylinder's side approximated as
// a B-spline surface in full. (A first build indexed the matrices with units directly, which fails in both.)
//
// Usage: node tools/fixtures/gen-batch-101.mjs
//
// Writes fixtures/batch-101/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, cylinder, line, mm, planeAt, regions, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
function roundAll(a is array) returns array
{
    return mapArray(a, function(x) { return x is number ? roundToPrecision(x, 6) : (x is array ? roundAll(x) : x); });
}

// A matrix with units of 1/length², in 1/mm², rounded.
function perSquareMillimetre(m is MatrixWithUnits) returns array
{
    return mapArray([0, 1, 2], function(i) { return mapArray([0, 1, 2], function(j) { return roundToPrecision(m.value[i][j] * 1e-6, 9); }); });
}

// The derivative of a face's second fundamental form at a parameter along its tangent plane's x and y axes, in
// 1/mm².
function derivatives(context is Context, face is Query, parameter is Vector) returns array
{
    const plane = evFaceTangentPlane(context, { "face" : face, "parameter" : parameter });
    return mapArray([plane.x, yAxis(plane)], function(direction) {
            const m = evFaceCurvatureDerivative(context, { "face" : face, "parameter" : parameter, "direction" : direction });
            return { "direction" : roundAll([direction[0], direction[1], direction[2]]), "matrix" : perSquareMillimetre(m) };
        });
}
`;

const batch = new Batch({ number: 101, title: 'curvature derivatives, internal tolerances, a B-spline approximation', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = 'qCreatedBy(id + "base", EntityType.BODY)';
const faceOf = (type) => `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.${type})`;

batch.add('curvature', "a cylinder's (radius 10) derivatives at two parameters", [
  cylinder('base', [0, 0], 10, 0, 30),
  returns({ a: `derivatives(context, ${faceOf('CYLINDER')}, vector(0.25, 0.5))`, b: `derivatives(context, ${faceOf('CYLINDER')}, vector(0.1, 0.3))` }),
]);
batch.add('curvature', "a torus's (radii 20 and 5) derivatives at a parameter", [
  sketch('p1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), circle('p1', 'c1', [20, 0], 5), solve('p1'),
  sketch('axis', planeAt([0, 0, -50], [0, 1, 0], [0, 0, 1])), line('axis', 'l1', [0, 0], [100, 0]), solve('axis'),
  call('revolve', 'base', { entities: regions('p1'), axis: 'qCreatedBy(id + "axis", EntityType.EDGE)', fullRevolve: 'true' }),
  returns({ a: `derivatives(context, ${faceOf('TORUS')}, vector(0.3, 0.2))` }),
]);
batch.add('curvature', "a sphere's (radius 10) derivatives at a parameter", [
  call('fSphere', 'base', { radius: mm(10) }),
  returns({ a: `derivatives(context, ${faceOf('SPHERE')}, vector(0.3, 0.6))` }),
]);
batch.add('curvature', "a cone frustum's (radius 10 at z = 0 to 5 at z = 20) derivatives at a parameter", [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [5, 20]), line('s1', 'l3', [5, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1'),
  call('revolve', 'base', { entities: regions('s1'), axis: `sketchEntityQuery(id + "s1", EntityType.EDGE, "l4")`, fullRevolve: 'true' }),
  returns({ a: `derivatives(context, ${faceOf('CONE')}, vector(0.3, 0.5))` }),
]);
batch.add('curvature', "a cylinder's derivative along a direction leaning half out of its tangent plane", [
  cylinder('base', [0, 0], 10, 0, 30),
  `const plane = evFaceTangentPlane(context, { "face" : ${faceOf('CYLINDER')}, "parameter" : vector(0.1, 0.3) });`,
  `const m = evFaceCurvatureDerivative(context, { "face" : ${faceOf('CYLINDER')}, "parameter" : vector(0.1, 0.3), "direction" : normalize(yAxis(plane) + 0.5 * plane.normal) });`,
  returns({ matrix: 'perSquareMillimetre(m)' }),
]);
batch.add('tolerance', "a block's greatest tolerance (evMaxTolerance alone)", [
  call('fCuboid', 'base', { corner1: vec3(0, 0, 0), corner2: vec3(10, 20, 30) }),
  returns({ max: `evMaxTolerance(context, { "entities" : ${base} })` }),
]);
batch.add('tolerance', "a block's faults (evFaults alone)", [
  call('fCuboid', 'base', { corner1: vec3(0, 0, 0), corner2: vec3(10, 20, 30) }),
  returns({ faults: `evFaults(context, { "entities" : ${base} })` }),
]);
batch.add('approximate', "a cylinder's side (radius 10, z 0 to 30) approximated as a B-spline surface, in full", [
  cylinder('base', [0, 0], 10, 0, 30),
  `const s = evApproximateBSplineSurface(context, { "face" : ${faceOf('CYLINDER')} }).bSplineSurface;`,
  returns({ poles: 'mapArray(s.controlPoints, function(row) { return mapArray(row, function(p) { return roundAll([p[0] / millimeter, p[1] / millimeter, p[2] / millimeter]); }); })', weights: 'roundAll(s.weights)' }),
]);

batch.write('tools/fixtures/gen-batch-101.mjs');
