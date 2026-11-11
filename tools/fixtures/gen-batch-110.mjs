// Generates fixture batch 110: where a sphere's principal directions switch world axis (batches 099, 109: the
// least is the normal crossed with the axis cyclically before the normal's largest component, but not always where
// two components nearly tie). Normals crossing each boundary between two components (every pair, every sign of
// each, the third component small and either sign), the ratio of the two from 0.90 to 1.10: for each, the axis.
//
// Usage: node tools/fixtures/gen-batch-110.mjs
//
// Writes fixtures/batch-110/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, mm } from './lib.mjs';

const helpers = `
// The world axis (x, y or z) whose cross with the normal, or that cross crossed with it, is the least curvature's
// direction at each normal on a sphere of radius 10 about the origin: one letter per normal, "?" if none.
function axesAt(context is Context, face is Query, normals is array) returns string
{
    var out = "";
    for (var n in normals)
    {
        const unit = normalize(vector(n[0], n[1], n[2]));
        const parameter = evDistance(context, { "side0" : face, "side1" : unit * 10 * millimeter }).sides[0].parameter;
        const d = evFaceCurvature(context, { "face" : face, "parameter" : parameter }).minDirection;
        var letter = "?";
        for (var k = 2; k >= 0; k -= 1)
        {
            var axis = vector(0, 0, 0);
            axis[k] = 1;
            const c = cross(unit, axis);
            if (norm(c) < 1e-9)
                continue;
            if (abs(abs(dot(d, normalize(c))) - 1) < 1e-6 || abs(abs(dot(d, normalize(cross(unit, c)))) - 1) < 1e-6)
                letter = ["x", "y", "z"][k];
        }
        out = out ~ letter;
    }
    return out;
}
`;

const batch = new Batch({ number: 110, title: "where a sphere's principal directions switch axis", helpers });

const ratios = Array.from({ length: 21 }, (_, i) => 0.9 + i * 0.01);
const sphere = call('fSphere', 'ball', { radius: mm(10) });
const face = 'qOwnedByBody(qCreatedBy(id + "ball", EntityType.BODY), EntityType.FACE)';
const pairs = [[1, 2], [0, 1], [0, 2]];
const names = 'xyz';
for (const [i, j] of pairs) {
  const k = 3 - i - j;
  const arcs = [];
  for (const si of [1, -1]) for (const sj of [1, -1]) for (const sk of [1, -1]) {
    // Components i and j of magnitude 1 and ratio, the third 0.2: the boundary at ratio 1.
    const normals = ratios.map((r) => {
      const n = [0, 0, 0];
      n[i] = si * 1;
      n[j] = sj * Math.round(r * 100) / 100;
      n[k] = sk * 0.2;
      return n;
    });
    arcs.push(`"${si > 0 ? '+' : '-'}${names[i]} ${sj > 0 ? '+' : '-'}${names[j]} ${sk > 0 ? '+' : '-'}${names[k]}" : axesAt(context, ${face}, [${normals.map((n) => `[${n.join(', ')}]`).join(', ')}])`);
  }
  batch.add('sphere', `normals crossing the ${names[i]}–${names[j]} boundary (|${names[j]}| / |${names[i]}| from 0.90 to 1.10, |${names[k]}| 0.2): each one's axis`, [
    sphere, `return { ${arcs.join(', ')} };`,
  ]);
}

batch.write('tools/fixtures/gen-batch-110.mjs');
