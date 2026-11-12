// Generates fixture batch 138: frames locked to curved faces along paths other than a cylinder's rim (batch 070) and
// a sphere's great circle (batch 076): a square tube, a flat bar and a round tube round the closed curve a circle
// beside a cylinder splits its side along, locked to the side; a round tube round the small circle a tilted plane
// splits a sphere along, locked to the sphere; a square tube round the four edges a rectangle beside a cylinder
// splits its side along (two lines up it, two arcs round it), locked to the side. Each returns its parts' faces.
//
// Usage: node tools/fixtures/gen-batch-138.mjs
//
// Writes fixtures/batch-138/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, cylinder, faceAt, mm, planeAt, rect, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
// A frame profile, the only sketch of its own context, as a profile from an element library would be.
const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-1, -1) * millimeter, "secondCorner" : vector(1, 1) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-0.8, -0.8) * millimeter, "secondCorner" : vector(0.8, 0.8) * millimeter });
    skSolve(s);
    return context;
};

const flatBar = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "bar", { "firstCorner" : vector(-0.5, -1.5) * millimeter, "secondCorner" : vector(0.5, 1.5) * millimeter });
    skSolve(s);
    return context;
};

const roundTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skCircle(s, "outer", { "center" : vector(0, 0) * millimeter, "radius" : 1 * millimeter });
    skCircle(s, "inner", { "center" : vector(0, 0) * millimeter, "radius" : 0.8 * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}

// The part's faces: each one's surface type and area.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType;
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({ number: 138, title: 'frames locked to curved faces along other paths', helpers });
const base = solidsOf('base');
const wires = (name) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const side = faceAt(base, [10, 0, 15]);
const splitByCircle = [
  cylinder('base', [0, 0], 10, 0, 30),
  sketch('cut', planeAt([15, 0, 0], [1, 0, 0], [0, 1, 0])), circle('cut', 'circle1', [0, 15], 4), solve('cut'),
  call('splitPart', 'split1', { splitType: 'SplitType.FACE', faceTargets: side, faceTools: wires('cut'), projectionType: 'ProjectionType.NORMAL_TO_TARGET' }),
];
const framed = (profile, path, lock) => [
  call('frame', 'frame1', { profileSketch: `profileOf(${profile})`, selections: path, lockProfile: 'true', profileLockFaces: lock }),
  `return { "faces" : faceProbe(context, ${solidsOf('frame1')}) };`,
];

batch.add('frame', 'a square tube (2 × 2, 0.2 thick) round the curve a circle beside a cylinder splits its side along, locked to the side', [
  splitByCircle, framed('squareTube', `qCreatedBy(id + "split1", EntityType.EDGE)`, faceAt(base, [-10, 0, 15])),
]);
batch.add('frame', 'a flat bar (1 × 3) round the curve a circle beside a cylinder splits its side along, locked to the side', [
  splitByCircle, framed('flatBar', `qCreatedBy(id + "split1", EntityType.EDGE)`, faceAt(base, [-10, 0, 15])),
]);
batch.add('frame', 'a round tube (radius 1, 0.2 thick) round the curve a circle beside a cylinder splits its side along, locked to the side', [
  splitByCircle, framed('roundTube', `qCreatedBy(id + "split1", EntityType.EDGE)`, faceAt(base, [-10, 0, 15])),
]);
batch.add('frame', 'a round tube round the small circle a plane 5 from a sphere\'s centre, tilted 30°, splits it along, locked to the sphere', [
  call('fSphere', 'base', { radius: mm(10) }),
  call('opPlane', 'tilted', { plane: `plane(${vec3(0, -2.5, 5 * Math.cos(Math.PI / 6))}, vector(0, ${-Math.sin(Math.PI / 6)}, ${Math.cos(Math.PI / 6)}))`, width: mm(40), height: mm(40) }),
  call('opSplitFace', 'split1', { faceTargets: `qOwnedByBody(${base}, EntityType.FACE)`, faceTools: `qCreatedBy(id + "tilted", EntityType.FACE)` }),
  framed('roundTube', `qCreatedBy(id + "split1", EntityType.EDGE)`, faceAt(base, [0, 0, -10])),
]);
batch.add('frame', 'a square tube round the four edges a 8 × 10 rectangle beside a cylinder splits its side along, locked to the side', [
  cylinder('base', [0, 0], 10, 0, 30),
  sketch('cut', planeAt([15, 0, 0], [1, 0, 0], [0, 1, 0])), rect('cut', 'rect1', [-4, 10], [4, 20]), solve('cut'),
  call('splitPart', 'split1', { splitType: 'SplitType.FACE', faceTargets: side, faceTools: wires('cut'), projectionType: 'ProjectionType.NORMAL_TO_TARGET' }),
  framed('squareTube', `qCreatedBy(id + "split1", EntityType.EDGE)`, faceAt(base, [-10, 0, 15])),
]);

batch.write('tools/fixtures/gen-batch-138.mjs');
