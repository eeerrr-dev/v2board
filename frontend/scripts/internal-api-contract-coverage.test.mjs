import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import test from 'node:test';

import { routeMap } from '../tests/lib/dialect/route-map.mjs';

const spec = JSON.parse(
  await readFile('packages/api-client/openapi/internal-api.openapi.json', 'utf8'),
);

const apiPrefix = '/api/v1';
const operationMethods = ['get', 'post', 'put', 'patch', 'delete'];

// Native security/audit operations did not replace a legacy route, so they do
// not belong in the two-world dialect map. They are still first-class internal
// operations and therefore must be present in the generated contract.
const nativeOperations = [
  ['GET', '/{secure_path}/account/mfa'],
  ['POST', '/{secure_path}/account/mfa/totp'],
  ['POST', '/{secure_path}/account/mfa/totp/confirm'],
  ['POST', '/{secure_path}/account/mfa/totp/disable'],
  ['GET', '/{secure_path}/system/audit-logs'],
  ['GET', '/staff/account/mfa'],
  ['POST', '/staff/account/mfa/totp'],
  ['POST', '/staff/account/mfa/totp/confirm'],
  ['POST', '/staff/account/mfa/totp/disable'],
];

function operationKey(method, path) {
  return `${method.toUpperCase()} ${apiPrefix}${path}`;
}

function openApiOperations() {
  const operations = [];
  for (const [path, pathItem] of Object.entries(spec.paths ?? {})) {
    for (const method of operationMethods) {
      if (pathItem[method]) operations.push([method.toUpperCase(), path, pathItem[method]]);
    }
  }
  return operations;
}

function resolveLocalReference(value) {
  if (!value?.$ref) return value;
  const prefix = '#/';
  assert.ok(
    value.$ref.startsWith(prefix),
    `only local OpenAPI references are supported: ${value.$ref}`,
  );
  return value.$ref
    .slice(prefix.length)
    .split('/')
    .reduce(
      (current, segment) => current?.[segment.replaceAll('~1', '/').replaceAll('~0', '~')],
      spec,
    );
}

function parametersAt(operation, location) {
  return (operation.parameters ?? []).filter((parameter) => parameter.in === location);
}

function registryId(operation) {
  return operation['x-v2board-operation-id'];
}

function isObjectSchema(schema) {
  return (
    schema?.type === 'object' ||
    (Array.isArray(schema?.type) && schema.type.includes('object')) ||
    schema?.properties !== undefined
  );
}

function objectPolicies(schema, path, result) {
  if (schema === true || schema === false || !schema || typeof schema !== 'object') return;
  if (isObjectSchema(schema)) {
    result.push({
      path: path.join('/'),
      policy: Object.hasOwn(schema, 'additionalProperties')
        ? schema.additionalProperties
        : undefined,
    });
  }
  if (schema.items) objectPolicies(schema.items, [...path, 'items'], result);
  if (schema.additionalProperties && schema.additionalProperties !== true) {
    objectPolicies(schema.additionalProperties, [...path, 'additionalProperties'], result);
  }
  for (const keyword of ['oneOf', 'anyOf', 'allOf']) {
    for (const [index, member] of (schema[keyword] ?? []).entries()) {
      objectPolicies(member, [...path, keyword, String(index)], result);
    }
  }
  for (const [name, property] of Object.entries(schema.properties ?? {})) {
    objectPolicies(property, [...path, 'properties', name], result);
  }
}

const reviewedOpenObjects = new Map([
  [
    'AdminPaymentItem/properties/config',
    'Imported installations can contain an unknown historical provider; reads preserve its fully redacted string-valued manifest keys, while create uses a closed provider union and PATCH cannot mutate configuration.',
  ],
  [
    'KnowledgeGroups',
    'Operator-authored knowledge category names are dynamic; every value is a typed article array.',
  ],
  [
    'NodeSortRequest',
    'The first map key is a protocol name; every value is the typed per-protocol node sort map.',
  ],
  [
    'NodeSortRequest/additionalProperties',
    'The second map key is a database node id; every value is an integer sort position.',
  ],
  [
    'PaymentProviderFormResponse',
    'The field name comes from the selected provider manifest; every value is a typed form-field DTO.',
  ],
  [
    'QueueStatsView/properties/last_failure_at',
    'Queue names are deployment-defined; every value is an RFC 3339 timestamp.',
  ],
  [
    'QueueStatsView/properties/last_run_at',
    'Queue names are deployment-defined; every value is an RFC 3339 timestamp.',
  ],
  [
    'QueueStatsView/properties/last_success_at',
    'Queue names are deployment-defined; every value is an RFC 3339 timestamp.',
  ],
  [
    'QueueStatsView/properties/wait',
    'Queue names are deployment-defined; every value is an integer wait count.',
  ],
  [
    'ServerTransportHeaders',
    'HTTP transport header names are protocol-defined and dynamic; every value is a closed string-or-string-array union.',
  ],
  [
    'VmessDnsSettings/properties/hosts',
    'DNS hostnames are configuration-defined map keys; every value is a closed string-or-string-array union.',
  ],
]);

function isReviewedOpenObject(path) {
  if (reviewedOpenObjects.has(path)) return true;
  return (
    path === 'ProblemDetails/allOf/0' ||
    path === 'ProblemDetails/allOf/0/properties/errors' ||
    /^ProblemDetails\/allOf\/1\/oneOf\/(?:[0-9]|[1-9][0-9]|100)$/.test(path)
  );
}

async function filesBelow(root) {
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) files.push(...(await filesBelow(path)));
    else files.push(path);
  }
  return files;
}

test('OpenAPI covers the exact 158-operation internal runtime surface', () => {
  const expected = new Set([
    ...routeMap.map(({ modern }) => operationKey(modern.method, modern.path)),
    ...nativeOperations.map(([method, path]) => operationKey(method, path)),
  ]);
  const actualOperations = openApiOperations();
  const actual = new Set(actualOperations.map(([method, path]) => `${method} ${path}`));

  assert.equal(routeMap.length, 155, 'the compatibility map semantic-row count drifted');
  assert.equal(expected.size, 158, 'the unique internal operation inventory drifted');
  assert.equal(
    actualOperations.length,
    actual.size,
    'the OpenAPI operation set contains duplicates',
  );
  assert.deepEqual(actual, expected);

  const operationIds = actualOperations.map(([, , operation]) => operation.operationId);
  assert.ok(operationIds.every(Boolean), 'every internal operation needs an operationId');
  assert.equal(
    new Set(operationIds).size,
    operationIds.length,
    'operationId values must be unique',
  );
});

test('production wrappers consume only named generated operations', async () => {
  const endpointFiles = (await filesBelow('packages/api-client/src/endpoints')).filter((path) =>
    path.endsWith('.ts'),
  );
  const endpointSources = await Promise.all(
    endpointFiles.map(async (path) => [path, await readFile(path, 'utf8')]),
  );
  const generatedRuntime = await readFile(
    'packages/api-client/src/generated/internal-api.ts',
    'utf8',
  );
  const generatedNames = new Set(
    [...generatedRuntime.matchAll(/^  "([A-Za-z0-9]+)": \{$/gm)].map((match) => match[1]),
  );

  assert.equal(generatedNames.size, 158, 'the generated runtime operation registry drifted');
  const wrapperNames = [];
  for (const [path, source] of endpointSources) {
    assert.doesNotMatch(
      source,
      /client\.(?:request|requestBinary)\s*\(/,
      `${path} bypasses the generated operation executor`,
    );
    assert.doesNotMatch(
      source,
      /resolveAdminPath\s*\(/,
      `${path} hand-builds a generated admin route`,
    );
    assert.doesNotMatch(
      source,
      /(?:json)?responseSchema\s*:/i,
      `${path} hand-selects a response schema`,
    );
    wrapperNames.push(
      ...[...source.matchAll(/requestInternal(?:Binary)?\(client,\s*['"]([^'"]+)['"]/g)].map(
        (match) => match[1],
      ),
    );
  }

  assert.ok(wrapperNames.length > 0, 'no generated operation wrappers were found');
  for (const name of wrapperNames) {
    assert.ok(generatedNames.has(name), `endpoint wrapper references unknown operation ${name}`);
  }
  await assert.rejects(
    readFile('packages/api-client/src/contracts.ts', 'utf8'),
    /ENOENT/,
    'the retired handwritten root DTO registry returned',
  );
});

test('runtime contracts contain no permissive-object escape hatch', async () => {
  const sourceFiles = (await filesBelow('packages/api-client/src')).filter((path) =>
    path.endsWith('.ts'),
  );
  sourceFiles.push('scripts/generate-internal-api-contract.mjs');
  for (const path of sourceFiles) {
    assert.doesNotMatch(
      await readFile(path, 'utf8'),
      /z\.looseObject|looseObject/,
      `${path} reintroduced an implicit permissive object contract`,
    );
  }
});

test('every object schema is explicitly closed or belongs to the reviewed open-map inventory', () => {
  const policies = [];
  for (const [name, schema] of Object.entries(spec.components?.schemas ?? {})) {
    objectPolicies(schema, [name], policies);
  }

  assert.deepEqual(
    policies.filter(({ policy }) => policy === undefined).map(({ path }) => path),
    [],
    'an object schema omitted additionalProperties and therefore became implicitly open',
  );
  const open = policies.filter(({ policy }) => policy !== false);
  assert.equal(open.length, 114, 'the reviewed open-object inventory drifted');
  assert.deepEqual(
    open.filter(({ path }) => !isReviewedOpenObject(path)).map(({ path }) => path),
    [],
    'an unreviewed open object entered the transport contract',
  );
  assert.equal(
    open.filter(({ path }) => /^ProblemDetails\/allOf\/1\/oneOf\//.test(path)).length,
    101,
    'the RFC 9457 tuple-arm inventory drifted',
  );
  assert.deepEqual(
    open
      .filter(({ path }) => !path.startsWith('ProblemDetails/'))
      .map(({ path }) => path)
      .sort(),
    [...reviewedOpenObjects.keys()].sort(),
    'every non-problem open map needs an exact path and durable rationale',
  );
  assert.ok(
    [...reviewedOpenObjects.values()].every((reason) => reason.length >= 40),
    'every reviewed open map needs a substantive rationale',
  );
  assert.equal(
    spec.components?.schemas?.JsonValue,
    undefined,
    'the recursive arbitrary-JSON transport type must not return',
  );
});

test('generated permissive types and catchalls stay confined to explicit extension islands', async () => {
  const types = await readFile('packages/types/src/generated/internal-api.ts', 'utf8');
  const unknownIndexOwners = types
    .split('\n')
    .filter((line) => line.includes('[key: string]: unknown'))
    .map((line) => line.match(/^export type (InternalApi\w+) =/)?.[1]);
  assert.deepEqual(unknownIndexOwners, ['InternalApiProblemDetails']);
  assert.doesNotMatch(
    types,
    /InternalApiJsonValue|Record<string, unknown>/,
    'business DTOs must not expose a recursive arbitrary-JSON or unknown-valued map',
  );

  const runtime = await readFile('packages/api-client/src/generated/internal-api.ts', 'utf8');
  const problemStart = runtime.indexOf('export const internalApiProblemDetailsSchema');
  assert.notEqual(problemStart, -1, 'the generated RFC 9457 schema is missing');
  const problemEnd = runtime.indexOf('\nexport const ', problemStart + 1);
  const runtimeWithoutProblem =
    runtime.slice(0, problemStart) + runtime.slice(problemEnd === -1 ? runtime.length : problemEnd);
  assert.doesNotMatch(
    runtimeWithoutProblem,
    /\.catchall\(/,
    'a non-RFC transport validator became permissive',
  );
  assert.equal(
    [...runtime.matchAll(/\.catchall\(z\.unknown\(\)\)/g)].length,
    102,
    'only the RFC 9457 base and 101 tuple arms may use unknown catchalls',
  );
});

test('handwritten endpoint adapters cannot reopen generated business DTOs', async () => {
  const endpointFiles = (await filesBelow('packages/api-client/src/endpoints')).filter((file) =>
    file.endsWith('.ts'),
  );
  for (const file of endpointFiles) {
    const source = await readFile(file, 'utf8');
    assert.doesNotMatch(
      source,
      /JsonValue|Record<string, unknown>|z\.(?:unknown|any|looseObject)\s*\(/,
      `${file} reopens a generated request or response DTO`,
    );
  }
});

test('OpenAPI query parameters match the exact 22 Axum Query operations', () => {
  const expected = new Map([
    ['auth.quick-login', ['redirect', 'token']],
    ['user.orders.list', ['status']],
    ['user.knowledge.list', ['keyword', 'language']],
    ['user.knowledge-categories.list', ['language']],
    ['user.notices.list', ['page', 'per_page']],
    ['user.commissions.list', ['page', 'per_page']],
    ['admin.config.get', ['group']],
    ['admin.system.logs', ['filter', 'page', 'per_page', 'sort_by', 'sort_dir']],
    ['admin.system.audit-logs.list', ['filter', 'page', 'per_page', 'sort_by', 'sort_dir']],
    ['admin.coupons.list', ['page', 'per_page', 'sort_by', 'sort_dir']],
    ['admin.gift-cards.list', ['page', 'per_page', 'sort_by', 'sort_dir']],
    ['admin.stats.server-rank', ['window']],
    ['admin.stats.user-rank', ['window']],
    ['admin.stats.user-traffic', ['page', 'per_page', 'user_id']],
    ['admin.stats.records', ['type']],
    ['admin.payment-providers.form', ['payment_id']],
    ['admin.orders.list', ['commission_only', 'filter', 'page', 'per_page', 'sort_by', 'sort_dir']],
    [
      'admin.payment-reconciliations.list',
      ['callback_no', 'page', 'payment_id', 'per_page', 'reason', 'resolved', 'trade_no'],
    ],
    ['admin.tickets.list', ['email', 'page', 'per_page', 'reply_status', 'status']],
    ['admin.users.list', ['filter', 'page', 'per_page', 'sort_by', 'sort_dir']],
    ['admin.server-groups.list', ['group_id']],
    ['staff.tickets.list', ['page', 'per_page', 'status']],
  ]);

  const actualIds = new Set();
  for (const [, , operation] of openApiOperations()) {
    const parameters = parametersAt(operation, 'query');
    if (parameters.length === 0) continue;
    const id = registryId(operation);
    actualIds.add(id);
    assert.deepEqual(
      parameters.map(({ name }) => name).sort(),
      expected.get(id),
      `${id} query parameter set drifted`,
    );
  }
  assert.deepEqual(actualIds, new Set(expected.keys()));

  const byId = new Map(
    openApiOperations().map(([, , operation]) => [registryId(operation), operation]),
  );
  const parameter = (id, name) => {
    const found = parametersAt(byId.get(id), 'query').find((item) => item.name === name);
    assert.ok(found, `${id} is missing query parameter ${name}`);
    return found;
  };
  for (const [id, name] of [
    ['auth.quick-login', 'token'],
    ['admin.stats.server-rank', 'window'],
    ['admin.stats.user-rank', 'window'],
    ['admin.stats.user-traffic', 'user_id'],
  ]) {
    assert.equal(parameter(id, name).required, true, `${id}.${name} must be required`);
  }
  assert.deepEqual(parameter('admin.stats.server-rank', 'window').schema.enum, [
    'today',
    'previous',
  ]);
  assert.deepEqual(parameter('admin.stats.records', 'type').schema.enum, ['d', 'm']);
  assert.equal(parameter('admin.stats.records', 'type').schema.default, 'd');
  assert.equal(parameter('user.notices.list', 'page').schema.minimum, 1);
  assert.equal(parameter('user.notices.list', 'page').schema.default, 1);
  assert.equal(parameter('user.notices.list', 'per_page').schema.minimum, 1);
  assert.equal(parameter('user.notices.list', 'per_page').schema.maximum, 100);
  assert.equal(parameter('user.notices.list', 'per_page').schema.default, 5);

  const replies = parameter('admin.tickets.list', 'reply_status');
  assert.equal(replies.schema.type, 'array');
  assert.equal(replies.schema.items.type, 'integer');
  assert.equal(replies.style, 'form');
  assert.equal(replies.explode, true);
});

test('OpenAPI projects the common locale and exact operation-specific request headers', () => {
  const userAgent = new Set();
  const idempotency = new Set();
  const stepUp = new Set();

  for (const [method, path, operation] of openApiOperations()) {
    const headers = parametersAt(operation, 'header');
    const names = new Set(headers.map(({ name }) => name));
    const id = registryId(operation);
    const language = headers.find(({ name }) => name === 'Accept-Language');
    assert.ok(language, `${id} has no Accept-Language transport parameter`);
    assert.equal(language.required, false);
    if (names.has('User-Agent')) userAgent.add(id);
    if (names.has('Idempotency-Key')) idempotency.add(id);
    if (names.has('X-V2Board-Step-Up')) stepUp.add(id);

    const expectsStepUp =
      ((path.startsWith('/api/v1/{secure_path}/') || path.startsWith('/api/v1/staff/')) &&
        method !== 'GET') ||
      id === 'admin.nodes.list' ||
      id === 'admin.payment-reconciliations.list';
    assert.equal(names.has('X-V2Board-Step-Up'), expectsStepUp, `${id} step-up header drifted`);
    for (const header of headers.filter(({ name }) => name !== 'Accept-Language')) {
      assert.equal(header.required, false, `${id}.${header.name} must remain optional`);
    }
  }

  assert.deepEqual(userAgent, new Set(['auth.login', 'auth.register', 'auth.token-login']));
  assert.deepEqual(idempotency, new Set(['admin.users.mail', 'staff.users.mail']));
  assert.equal(stepUp.size, 67);

  const mail = openApiOperations()
    .map(([, , operation]) => operation)
    .find((operation) => registryId(operation) === 'admin.users.mail');
  assert.equal(
    parametersAt(mail, 'header').find(({ name }) => name === 'Idempotency-Key').schema[
      'x-v2board-max-bytes'
    ],
    512,
  );
});

test('every internal operation declares exact successes and one honest RFC 9457 fallback', () => {
  for (const [method, path, operation] of openApiOperations()) {
    const responses = operation.responses ?? {};
    const statuses = Object.keys(responses);
    const successStatuses = statuses.filter((status) => /^[23]\d\d$/.test(status));
    assert.ok(successStatuses.length > 0, `${method} ${path} has no exact success response`);
    assert.ok(
      statuses.every((status) => status !== '2XX' && status !== '3XX'),
      `${method} ${path} uses an ambiguous success response key`,
    );

    assert.equal(
      responses.default?.$ref,
      '#/components/responses/DefaultProblem',
      `${method} ${path} must use the shared default problem response`,
    );
    assert.equal(
      statuses.filter((status) => /^\d+$/.test(status) && Number(status) >= 400).length,
      0,
      `${method} ${path} must not invent an endpoint-specific error-status set`,
    );
    const problemResponse = resolveLocalReference(responses.default);
    assert.ok(
      problemResponse?.content?.['application/problem+json'],
      `${method} ${path} has no application/problem+json fallback`,
    );
  }
});

test('the shared problem schema pins all 101 code/status/title tuples', () => {
  const problem = spec.components?.schemas?.ProblemDetails;
  assert.ok(problem, 'ProblemDetails component is missing');
  assert.equal(problem.allOf?.length, 2);
  const [base, discriminated] = problem.allOf;
  assert.equal(base.type, 'object');
  assert.deepEqual(new Set(base.required), new Set(['type', 'title', 'status', 'code', 'detail']));
  assert.equal(base.properties?.type?.const, 'about:blank');
  assert.equal(base.properties?.errors?.type, 'object');
  assert.ok(!base.required.includes('errors'), 'validation errors must remain optional');

  const problemCode = resolveLocalReference(base.properties?.code);
  assert.equal(problemCode?.enum?.length, 101);
  assert.equal(discriminated.discriminator?.propertyName, 'code');
  assert.equal(discriminated.oneOf?.length, 101);
  const tuples = discriminated.oneOf.map((variant) => ({
    code: variant.properties?.code?.const,
    status: variant.properties?.status?.const,
    title: variant.properties?.title?.const,
    errors: variant.properties?.errors,
  }));
  assert.equal(new Set(tuples.map(({ code }) => code)).size, 101);
  assert.deepEqual(new Set(tuples.map(({ code }) => code)), new Set(problemCode.enum));
  assert.ok(tuples.every(({ status }) => Number.isInteger(status) && status >= 400));
  assert.ok(tuples.every(({ title }) => typeof title === 'string' && title.length > 0));
  assert.ok(
    tuples.every(({ code, errors }) =>
      code === 'validation_failed' ? errors === undefined : errors === false,
    ),
    'only validation_failed may carry the optional errors bag',
  );

  const responseNames = [
    'BadRequestProblem',
    'UnauthorizedProblem',
    'ForbiddenProblem',
    'NotFoundProblem',
    'ConflictProblem',
    'ValidationProblem',
    'RateLimitedProblem',
    'InternalServerProblem',
    'BadGatewayProblem',
    'ServiceUnavailableProblem',
  ];
  for (const name of responseNames) {
    assert.ok(
      spec.components?.responses?.[name]?.content?.['application/problem+json'],
      `${name} response component is missing`,
    );
  }
  const unauthorized = spec.components?.responses?.UnauthorizedProblem;
  assert.ok(
    unauthorized.headers?.['WWW-Authenticate'],
    '401 problem responses must document the Bearer challenge',
  );
  const fallback = spec.components?.responses?.DefaultProblem;
  assert.ok(fallback?.content?.['application/problem+json']);
  assert.ok(fallback.headers?.['WWW-Authenticate']);
});
