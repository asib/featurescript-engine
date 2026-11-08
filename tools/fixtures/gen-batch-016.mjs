// Generates fixture batch 016: a study of sheet metal reliefs. Bend reliefs at the ends of partial flanges and
// corner reliefs where two flanges meet, across thicknesses, radii, styles and scales, to pin down their sizes.
//
// Usage: node tools/fixtures/gen-batch-016.mjs
//
// Writes fixtures/batch-016/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, deg, mm, planeAt, rect, sketch, solve } from './lib.mjs';

const helpers = `
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

function atFraction(context is Context, part is Query, entityType is EntityType, fx is number, fy is number, fz is number) returns Query
{
    return qContainsPoint(qOwnedByBody(part, entityType), pointAtFraction(context, part, fx, fy, fz));
}
`;

const batch = new Batch({ number: 16, title: 'sheet metal relief study: bend reliefs and corner reliefs', helpers });

const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;

const plate = (fields = {}) => [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()', ...fields,
  }),
];
const flange = (edges, fields = {}) =>
  call('sheetMetalFlange', 'flange1', {
    edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', ...fields,
  });
const middle = {
  isPartialFlange: 'true', chainType: 'SMPartialFlangeChainType.PER_EDGE', flipFlangeBounds: 'false', holdAdjacentEdges: 'false',
  firstBoundType: 'SMFlangeBoundingType.BLIND', firstBoundOffset: mm(5), hasSecondBound: 'true',
  secondBoundType: 'SMFlangeBoundingType.BLIND', secondBoundOffset: mm(5),
};
const partialFlange = (model, description) =>
  batch.add('bend-relief', description, [plate(model), flange(at('EDGE', 1, 0.5, 1), middle)]);
const corner = (model, description) =>
  batch.add('corner-relief', description, [plate(model), flange(`qUnion([${at('EDGE', 1, 0.5, 1)}, ${at('EDGE', 0.5, 1, 1)}])`)]);

// ----- bend reliefs: the default obround across thickness and radius, then other styles and scales -----

partialFlange({ thickness: mm(1), radius: mm(1) }, 'obround, 1 mm thick, 1 mm radius');
partialFlange({ thickness: mm(2), radius: mm(3) }, 'obround, 2 mm thick, 3 mm radius');
partialFlange({ thickness: mm(3), radius: mm(1) }, 'obround, 3 mm thick, 1 mm radius');
partialFlange({ defaultBendReliefStyle: 'SMBendStrategyType.RECTANGLE' }, 'rectangular, 2 mm thick, 1 mm radius');
partialFlange({ defaultBendReliefStyle: 'SMBendStrategyType.TEAR' }, 'tear, 2 mm thick, 1 mm radius');
partialFlange({ defaultBendReliefDepthScale: '3', defaultBendReliefScale: '2' }, 'obround, twice as wide and twice as deep');

// ----- corner reliefs where two flanges meet -----

corner({ defaultCornerStyle: 'SMCornerStrategyType.ROUND', defaultCornerReliefScale: '1.5' }, 'round, scale 1.5, 2 mm thick, 1 mm radius');
corner({ defaultCornerStyle: 'SMCornerStrategyType.ROUND', defaultCornerReliefScale: '1.5', thickness: mm(1), radius: mm(3) }, 'round, scale 1.5, 1 mm thick, 3 mm radius');
corner({ defaultCornerStyle: 'SMCornerStrategyType.ROUND', defaultCornerReliefScale: '2' }, 'round, scale 2, 2 mm thick, 1 mm radius');
corner({ defaultCornerStyle: 'SMCornerStrategyType.RECTANGLE', defaultCornerReliefScale: '1.5' }, 'rectangular, scale 1.5');
corner({ defaultCornerStyle: 'SMCornerStrategyType.SIZED_ROUND', defaultRoundReliefDiameter: mm(6) }, 'round, 6 mm across');
corner({ defaultCornerStyle: 'SMCornerStrategyType.SIZED_RECTANGLE', defaultSquareReliefWidth: mm(5) }, 'square, 5 mm wide');
corner({ defaultCornerStyle: 'SMCornerStrategyType.CLOSED' }, 'closed');

batch.write('tools/fixtures/gen-batch-016.mjs');
