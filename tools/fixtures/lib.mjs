// Shared pieces for fixture batches from 002 on: helpers that emit FeatureScript, and the writer for a batch's
// cases.fs, generator.fs and cases.json.
//
// A case is a FeatureScript function `cNNN(context, id)` that builds its geometry at the origin and may return a
// map of extra values to check. The generator (Onshape only, but our engine runs it too) builds every case, moves
// what it made to the case's grid slot, names its bodies `cNNN.<n>` in a geometric order both engines agree on,
// and prints one probe line per case:
//
//   probe cNNN { bodies : [ ... ] , construction : [ ... ] , values : ... }
//   probe cNNN failed <error>
//
// Each body's probe has its type, face/edge/vertex counts, tight bounding box (mm), and volume (mm^3), area (mm^2)
// or length (mm). The STEP export carries the exact shapes; the probe lines carry what STEP can't, such as wires,
// points, construction planes, mate connectors and computed values.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export const STD_VERSION = '3112.0';

export const num = (value) => {
  const rounded = Math.round(value * 1e6) / 1e6;
  return Object.is(rounded, -0) ? '0' : String(rounded);
};
export const mm = (value) => `${num(value)} * millimeter`;
export const deg = (value) => `${num(value)} * degree`;
export const vec2 = (x, y) => `vector(${num(x)}, ${num(y)}) * millimeter`;
export const vec3 = (x, y, z) => `vector(${num(x)}, ${num(y)}, ${num(z)}) * millimeter`;
export const dir3 = (x, y, z) => `vector(${num(x)}, ${num(y)}, ${num(z)})`;

// Planes written out explicitly, so neither engine depends on default plane conventions.
export const PLANES = {
  xy: 'plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0))',
  xz: 'plane(vector(0, 0, 0) * millimeter, vector(0, -1, 0), vector(1, 0, 0))',
  yz: 'plane(vector(0, 0, 0) * millimeter, vector(1, 0, 0), vector(0, 1, 0))',
  tilted: 'plane(vector(0, 0, 0) * millimeter, vector(0, -0.5, 0.8660254037844386), vector(1, 0, 0))',
};
export const planeAt = (origin, normal, x) => `plane(${vec3(...origin)}, ${dir3(...normal)}, ${dir3(...x)})`;

// Statements. `s` is a sketch's name, such as "sketch1"; ids are made from it.
export const sketch = (s, plane) => `const ${s} = newSketchOnPlane(context, id + "${s}", { "sketchPlane" : ${plane} });`;
export const solve = (s) => `skSolve(${s});`;
export const rect = (s, name, [x1, y1], [x2, y2]) =>
  `skRectangle(${s}, "${name}", { "firstCorner" : ${vec2(x1, y1)}, "secondCorner" : ${vec2(x2, y2)} });`;
export const circle = (s, name, [cx, cy], r) =>
  `skCircle(${s}, "${name}", { "center" : ${vec2(cx, cy)}, "radius" : ${mm(r)} });`;
export const line = (s, name, [x1, y1], [x2, y2], construction = false) =>
  `skLineSegment(${s}, "${name}", { "start" : ${vec2(x1, y1)}, "end" : ${vec2(x2, y2)}${construction ? ', "construction" : true' : ''} });`;
export const arc = (s, name, [x1, y1], [xm, ym], [x2, y2]) =>
  `skArc(${s}, "${name}", { "start" : ${vec2(x1, y1)}, "mid" : ${vec2(xm, ym)}, "end" : ${vec2(x2, y2)} });`;
export const polygon = (s, points) => points.map((p, i) => line(s, `line${i + 1}`, p, points[(i + 1) % points.length]));

// A FeatureScript map literal from a JS object whose values are FeatureScript expressions.
export const fsMap = (fields) =>
  `{ ${Object.entries(fields)
    .map(([key, value]) => `"${key}" : ${value}`)
    .join(', ')} }`;
// A call of a std feature or operation: call("extrude", "extrude1", {...}) → extrude(context, id + "extrude1", {...});
export const call = (fn, name, fields) => `${fn}(context, id + "${name}", ${fsMap(fields)});`;

export const regions = (s) => `qSketchRegion(id + "${s}")`;
export const createdBy = (name, type = 'BODY') => `qCreatedBy(id + "${name}", EntityType.${type})`;
export const solidsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;

// A box from (x1, y1, z1) to (x2, y2, z2) made with a sketch on the xy plane at z1 and a blind extrude: the base
// shape most cases start from. Returns statements; the solid is created by `name`.
export function box(name, [x1, y1, z1], [x2, y2, z2]) {
  const s = `${name}Sketch`;
  return [
    sketch(s, planeAt([0, 0, z1], [0, 0, 1], [1, 0, 0])),
    rect(s, 'rect1', [x1, y1], [x2, y2]),
    solve(s),
    call('extrude', name, {
      entities: regions(s),
      endBound: 'BoundingType.BLIND',
      depth: mm(z2 - z1),
    }),
  ];
}

// A cylinder along z from z1 to z2, centred at (cx, cy).
export function cylinder(name, [cx, cy], r, z1, z2) {
  const s = `${name}Sketch`;
  return [
    sketch(s, planeAt([0, 0, z1], [0, 0, 1], [1, 0, 0])),
    circle(s, 'circle1', [cx, cy], r),
    solve(s),
    call('extrude', name, { entities: regions(s), endBound: 'BoundingType.BLIND', depth: mm(z2 - z1) }),
  ];
}

// A straight line edge from a to b (3D, mm) for use as an axis or direction: a construction-free sketch line on a
// plane through both points. Returns statements; the edge is `qCreatedBy(id + name, EntityType.EDGE)`.
export function axisLine(name, a, b) {
  const d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const len = Math.hypot(...d);
  const x = d.map((v) => v / len);
  // Any normal perpendicular to the line.
  const helper = Math.abs(x[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0];
  let n = [x[1] * helper[2] - x[2] * helper[1], x[2] * helper[0] - x[0] * helper[2], x[0] * helper[1] - x[1] * helper[0]];
  const nl = Math.hypot(...n);
  n = n.map((v) => v / nl);
  return [
    sketch(name, planeAt(a, n, x)),
    line(name, 'line1', [0, 0], [len, 0]),
    solve(name),
  ];
}
export const edgeOf = (name) => `qCreatedBy(id + "${name}", EntityType.EDGE)`;

// A construction plane through `origin` with `normal` and `x`: its face is `qCreatedBy(id + name, EntityType.FACE)`.
export const planeBody = (name, origin, normal, x) =>
  call('opPlane', name, { plane: planeAt(origin, normal, x), width: mm(100), height: mm(100) });

// Faces, edges and vertices picked by geometry, so neither engine's query order matters.
export const faceAt = (scope, p) => `qContainsPoint(qOwnedByBody(${scope}, EntityType.FACE), ${vec3(...p)})`;
export const edgeAt = (scope, p) => `qContainsPoint(qOwnedByBody(${scope}, EntityType.EDGE), ${vec3(...p)})`;
export const vertexAt = (scope, p) => `qContainsPoint(qOwnedByBody(${scope}, EntityType.VERTEX), ${vec3(...p)})`;

export class Batch {
  // helpers: FeatureScript functions the cases share, written after the imports.
  constructor({ number, title, description, spacing = 0.4, perRow = 10, helpers = '' }) {
    this.number = String(number).padStart(3, '0');
    this.title = title;
    this.helpers = helpers;
    this.description = description;
    this.spacing = spacing;
    this.perRow = perRow;
    this.cases = [];
  }

  // body: an array of statements (strings), or nested arrays of them.
  add(kind, description, body) {
    const index = this.cases.length;
    const name = `c${String(index + 1).padStart(3, '0')}`;
    const slot = [(index % this.perRow) * this.spacing, Math.floor(index / this.perRow) * this.spacing, 0];
    this.cases.push({ name, kind, description, slot, body: body.flat(Infinity) });
    return name;
  }

  write(script) {
    const dir = new URL(`../../fixtures/batch-${this.number}/`, import.meta.url).pathname;
    mkdirSync(dir, { recursive: true });
    let cases = `FeatureScript 3112;\nimport(path : "onshape/std/geometry.fs", version : "${STD_VERSION}");\n\n`;
    cases += `// Fixture batch ${this.number}: ${this.title}. Generated by ${script}; do not edit.\n`;
    cases += `// Each case builds its geometry at the origin and may return a map of values to check.\n`;
    if (this.helpers) cases += `\n${this.helpers.trim()}\n`;
    for (const c of this.cases) {
      cases += `\n// ${c.kind}: ${c.description}\nexport function ${c.name}(context is Context, id is Id)\n{\n`;
      cases += c.body.map((statement) => `    ${statement}\n`).join('');
      cases += `}\n`;
    }
    writeFileSync(join(dir, 'cases.fs'), cases);
    writeFileSync(join(dir, 'generator.fs'), this.generator());
    const manifest = {
      batch: this.number,
      title: this.title,
      std: STD_VERSION,
      cases: this.cases.map(({ name, kind, description, slot }) => ({ name, kind, description, slotMeters: slot })),
    };
    writeFileSync(join(dir, 'cases.json'), JSON.stringify(manifest, null, 2) + '\n');
    console.log(`wrote ${this.cases.length} cases to ${dir}`);
  }

  generator() {
    const n = this.number;
    let fs = `
// ----- Builds every case of batch ${n}: in Onshape for the fixture, and on our engine to compare. -----

annotation { "Feature Type Name" : "Build fixture batch ${n}" }
export const buildFixtureBatch${n} = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        var failed = [];
`;
    for (const c of this.cases) {
      fs += `        if (!runFixtureCase(context, id, "${c.name}", vector(${num(c.slot[0])}, ${num(c.slot[1])}, ${num(c.slot[2])}) * meter,\n`;
      fs += `                function(context is Context, id is Id) { return ${c.name}(context, id); }))\n`;
      fs += `            failed = append(failed, "${c.name}");\n`;
    }
    fs += `        println("batch-${n} failed cases: " ~ toString(failed));
    });

// The fixture document's Part Studio holds one instance of the feature batch 001 defined, so every batch also
// exports its generator under that name: pasting a new batch then rebuilds the same feature instance.
annotation { "Feature Type Name" : "Build fixture batch ${n} (as 001)" }
export const buildFixtureBatch001 = buildFixtureBatch${n};
${GENERATOR_HELPERS}`;
    return fs;
  }
}

const GENERATOR_HELPERS = `
const FIXTURE_BODY_TYPES = [BodyType.SOLID, BodyType.SHEET, BodyType.WIRE, BodyType.POINT, BodyType.MATE_CONNECTOR];

// Builds one case, moves everything it made to the case's slot, names its bodies "<case>.<n>" and prints its probe
// line. Returns false if the case threw, or made no body and returned nothing.
function runFixtureCase(context is Context, id is Id, name is string, slot is Vector, build is function) returns boolean
{
    const caseId = id + name;
    var values;
    try
    {
        values = build(context, caseId);
    }
    catch (error)
    {
        clearFeatureStatus(context, id, { "withDisplayData" : true });
        try silent(opDeleteBodies(context, id + (name ~ "cleanup"), { "entities" : qCreatedBy(caseId, EntityType.BODY) }));
        println("probe " ~ name ~ " failed " ~ fixtureErrorText(error));
        return false;
    }
    const everything = qCreatedBy(caseId, EntityType.BODY);
    if (!isQueryEmpty(context, everything))
    {
        try silent
        {
            opTransform(context, caseId + "move", { "bodies" : everything, "transform" : transform(slot) });
        }
        catch
        {
            // Sheet metal parts can't be moved (SHEET_METAL_PARTS_PROHIBITED): such a case stays where it was built.
            clearFeatureStatus(context, id, { "withDisplayData" : true });
        }
    }
    const notSketch = qSketchFilter(everything, SketchObject.NO);
    const bodies = fixtureSortedBodies(context, qConstructionFilter(notSketch, ConstructionObject.NO));
    var probes = [];
    for (var i, body in bodies)
    {
        const bodyName = name ~ "." ~ (i + 1);
        try silent(setProperty(context, { "entities" : body, "propertyType" : PropertyType.NAME, "value" : bodyName }));
        probes = append(probes, fixtureProbe(context, body, bodyName));
    }
    var construction = [];
    for (var body in fixtureSortedBodies(context, qConstructionFilter(notSketch, ConstructionObject.YES)))
        construction = append(construction, fixtureProbe(context, body, "construction"));
    println("probe " ~ name ~ " " ~ toString({ "bodies" : probes, "construction" : construction, "values" : values }));
    return size(bodies) > 0 || values != undefined;
}

function fixtureErrorText(error) returns string
{
    if (error is map && error.message != undefined)
        return toString(error.message);
    return error == undefined ? "error" : toString(error);
}

function fixtureBodyType(context is Context, body is Query)
{
    for (var bodyType in FIXTURE_BODY_TYPES)
    {
        if (!isQueryEmpty(context, qBodyType(body, bodyType)))
            return bodyType;
    }
    return undefined;
}

function fixtureNumbers(v is Vector, unit) returns array
{
    var out = [];
    for (var x in v)
        out = append(out, x / unit);
    return out;
}

function fixtureBox(context is Context, body is Query) returns array
{
    const bounds = evBox3d(context, { "topology" : body, "tight" : true });
    return concatenateArrays([fixtureNumbers(bounds.minCorner, millimeter), fixtureNumbers(bounds.maxCorner, millimeter)]);
}

// Sorts bodies by type, then by the corners of their bounding boxes rounded to 10 µm.
function fixtureSortedBodies(context is Context, query is Query) returns array
{
    var keyed = [];
    for (var body in evaluateQuery(context, query))
    {
        var key = [indexOf(FIXTURE_BODY_TYPES, fixtureBodyType(context, body))];
        const bounds = try silent(fixtureBox(context, body));
        if (bounds != undefined)
        {
            for (var x in bounds)
                key = append(key, round(x * 100));
        }
        keyed = append(keyed, { "body" : body, "key" : key });
    }
    keyed = sort(keyed, function(a, b) { return fixtureCompareKeys(a.key, b.key); });
    var out = [];
    for (var k in keyed)
        out = append(out, k.body);
    return out;
}

function fixtureCompareKeys(a is array, b is array) returns number
{
    for (var i = 0; i < min(size(a), size(b)); i += 1)
    {
        if (a[i] != b[i])
            return a[i] - b[i];
    }
    return size(a) - size(b);
}

function fixtureProbe(context is Context, body is Query, name is string) returns map
{
    const bodyType = fixtureBodyType(context, body);
    var probe = {
        "name" : name,
        "type" : bodyType,
        "faces" : evaluateQueryCount(context, qOwnedByBody(body, EntityType.FACE)),
        "edges" : evaluateQueryCount(context, qOwnedByBody(body, EntityType.EDGE)),
        "vertices" : evaluateQueryCount(context, qOwnedByBody(body, EntityType.VERTEX))
    };
    probe.bounds = try silent(fixtureBox(context, body));
    if (bodyType == BodyType.SOLID)
        probe.volume = evVolume(context, { "entities" : body }) / millimeter ^ 3;
    if (bodyType == BodyType.SOLID || bodyType == BodyType.SHEET)
        probe.area = evArea(context, { "entities" : qOwnedByBody(body, EntityType.FACE) }) / millimeter ^ 2;
    if (bodyType == BodyType.WIRE)
        probe.length = evLength(context, { "entities" : qOwnedByBody(body, EntityType.EDGE) }) / millimeter;
    if (bodyType == BodyType.SHEET && probe.faces == 1)
    {
        const plane = try silent(evPlane(context, { "face" : qOwnedByBody(body, EntityType.FACE) }));
        if (plane != undefined)
            probe.plane = [fixtureNumbers(plane.origin, millimeter), fixtureNumbers(plane.normal, 1), fixtureNumbers(plane.x, 1)];
    }
    if (bodyType == BodyType.MATE_CONNECTOR)
    {
        const cs = evMateConnector(context, { "mateConnector" : body });
        probe.frame = [fixtureNumbers(cs.origin, millimeter), fixtureNumbers(cs.xAxis, 1), fixtureNumbers(cs.zAxis, 1)];
    }
    return probe;
}
`;
