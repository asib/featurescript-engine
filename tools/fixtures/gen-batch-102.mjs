// Generates fixture batch 102: what std's internal API functions do when a feature calls them (their builtins no
// std feature reaches): a feature's name, a parameter's tolerance info, a compressed query, and a booleaned pattern
// of a block. (A tolerance schema's validation calls its builtin directly, which a Feature Studio can't.)
//
// Usage: node tools/fixtures/gen-batch-102.mjs
//
// Writes fixtures/batch-102/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, vec3 } from './lib.mjs';

const batch = new Batch({ number: 102, title: "std's internal API functions called from a feature" });

const caught = (statement) => [`try { ${statement} } catch (error) { return { "error" : error }; }`];
const block = call('fCuboid', 'base', { corner1: vec3(0, 0, 0), corner2: vec3(10, 20, 30) });

batch.add('internal', "the running feature's name (getFeatureName)", [caught('return { "name" : getFeatureName(context, id) };')]);
batch.add('internal', "a parameter's tolerance info (getParameterToleranceInfo)", [
  caught('return { "info" : getParameterToleranceInfo(context, id, { "parameterId" : "depth" }) };'),
]);
batch.add('internal', 'a compressed query (qCompressed) evaluated', [
  block,
  caught('return { "count" : evaluateQueryCount(context, qCompressed(1, "", id)) };'),
]);
batch.add('internal', "a block patterned 40 mm along x by opBooleanedPattern", [
  block,
  caught(`opBooleanedPattern(context, id + "pattern1", { "entities" : qCreatedBy(id + "base", EntityType.BODY), "transforms" : [transform(${vec3(40, 0, 0)})], "instanceNames" : ["1"] }); return { "solids" : evaluateQueryCount(context, qBodyType(qCreatedBy(id + "pattern1", EntityType.BODY), BodyType.SOLID)) };`),
]);

batch.add('internal', "a 4 mm cube patterned 10 mm along y by opBooleanedPattern into a block it sits on (target, tool bodies and a union given)", [
  block,
  call('fCuboid', 'cube', { corner1: vec3(2, 2, 30), corner2: vec3(6, 6, 34) }),
  caught(`opBooleanedPattern(context, id + "pattern1", { "entities" : qCreatedBy(id + "cube", EntityType.BODY), "targetBody" : qCreatedBy(id + "base", EntityType.BODY), "toolBodies" : qCreatedBy(id + "cube", EntityType.BODY), "operationType" : BooleanOperationType.UNION, "transforms" : [transform(${vec3(0, 10, 0)})], "instanceNames" : ["1"] }); return { "solids" : evaluateQueryCount(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)) };`),
]);

batch.write('tools/fixtures/gen-batch-102.mjs');
