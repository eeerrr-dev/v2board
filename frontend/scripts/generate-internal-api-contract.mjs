import assert from 'node:assert/strict';
import { createClient } from '@hey-api/openapi-ts';
import { readFile, mkdir, writeFile, rm, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import process from 'node:process';

const arguments_ = process.argv.slice(2);
const check = arguments_.includes('--check');
const rootArgument = arguments_.find((value) => value.startsWith('--root='));
const root = path.resolve(rootArgument?.slice('--root='.length) ?? process.cwd());
const specPath = path.join(root, 'packages/api-client/openapi/internal-api.openapi.json');
const typesPath = path.join(root, 'packages/types/src/generated/internal-api.ts');
const runtimePath = path.join(root, 'packages/api-client/src/generated/internal-api.ts');
const heyApiOutputDir = path.join(root, 'packages/types/src/generated/hey-api');

const spec = JSON.parse(await readFile(specPath, 'utf8'));
assert.equal(spec.openapi, '3.1.0', 'the internal contract must use OpenAPI 3.1');
assert.ok(spec.components?.schemas, 'the internal contract has no component schemas');

const schemas = spec.components.schemas;
const httpMethods = ['get', 'put', 'post', 'delete', 'options', 'head', 'patch', 'trace'];
const parameterLocations = ['path', 'query', 'header', 'cookie'];
const emptyObjectType = 'Record<string, never>';

function decodePointerSegment(segment) {
  return segment.replaceAll('~1', '/').replaceAll('~0', '~');
}

function resolveLocalReference(reference, expectedComponent) {
  const prefix = '#/';
  assert.ok(reference.startsWith(prefix), `unsupported non-local reference: ${reference}`);
  const segments = reference.slice(prefix.length).split('/').map(decodePointerSegment);
  if (expectedComponent) {
    assert.deepEqual(
      segments.slice(0, 2),
      ['components', expectedComponent],
      `expected a components/${expectedComponent} reference: ${reference}`,
    );
  }
  const resolved = segments.reduce((current, segment) => current?.[segment], spec);
  assert.notEqual(resolved, undefined, `unresolved local reference: ${reference}`);
  return resolved;
}

function resolveComponent(value, component) {
  if (!value?.$ref) return value;
  const resolved = resolveLocalReference(value.$ref, component);
  const siblings = Object.fromEntries(Object.entries(value).filter(([name]) => name !== '$ref'));
  return Object.keys(siblings).length === 0 ? resolved : { ...resolved, ...siblings };
}

function refName(reference) {
  const prefix = '#/components/schemas/';
  assert.ok(reference.startsWith(prefix), `unsupported schema reference: ${reference}`);
  return decodePointerSegment(reference.slice(prefix.length));
}

function isObjectSchema(schema) {
  return (
    schema?.type === 'object' ||
    (Array.isArray(schema?.type) && schema.type.includes('object')) ||
    schema?.properties !== undefined
  );
}

/**
 * OpenAPI's implicit object policy is permissive, which is too easy to emit by
 * accident when a Rust DTO is `deny_unknown_fields`. Require the source
 * document to make every object decision explicit: `false` for transport DTOs,
 * `true`/a schema only for reviewed map and JSON-extension islands.
 */
function assertExplicitObjectPolicies(schema, context) {
  if (schema === true || schema === false || schema === undefined) return;
  assert.ok(schema && typeof schema === 'object', `invalid OpenAPI schema at ${context}`);
  if (isObjectSchema(schema)) {
    assert.ok(
      Object.hasOwn(schema, 'additionalProperties'),
      `${context} is an object schema without an explicit additionalProperties policy`,
    );
  }
  if (schema.items) assertExplicitObjectPolicies(schema.items, `${context}/items`);
  if (schema.additionalProperties && schema.additionalProperties !== true) {
    assertExplicitObjectPolicies(schema.additionalProperties, `${context}/additionalProperties`);
  }
  for (const keyword of ['oneOf', 'anyOf', 'allOf']) {
    for (const [index, member] of (schema[keyword] ?? []).entries()) {
      assertExplicitObjectPolicies(member, `${context}/${keyword}/${index}`);
    }
  }
  for (const [name, property] of Object.entries(schema.properties ?? {})) {
    assertExplicitObjectPolicies(property, `${context}/properties/${name}`);
  }
}

for (const [name, schema] of Object.entries(schemas)) {
  assertExplicitObjectPolicies(schema, `#/components/schemas/${name}`);
}

/**
 * hey-api's own schema/IR normalization drops a `properties` entry whose
 * value is the literal boolean `false` (JSON Schema's "this key must never be
 * present" idiom — used across `ProblemDetails`'s discriminator arms to
 * forbid `errors` outside `validation_failed`) before any `$resolvers` hook
 * sees the schema, so a resolver has nothing left to react to. Move each
 * forbidden name onto a vendor-extension array on the same schema object
 * (which hey-api passes through untouched, like `x-v2board-max-bytes`
 * elsewhere) so `heyApiObjectResolver` below can still recreate the
 * rejection. Registered as a `parser.patch.schemas` hook (see
 * `generateHeyApiOutput`), which hey-api runs on its own bundled copy of the
 * spec before parsing — the retained `typeExpression`/`zodExpression`
 * functions read the original `spec` and already handle a literal `false`
 * property correctly, so nothing needs to stay in sync here.
 */
function extractForbiddenProperties(schema) {
  if (schema === true || schema === false || schema === undefined) return;
  if (schema.properties) {
    const forbidden = Object.entries(schema.properties)
      .filter(([, property]) => property === false)
      .map(([propertyName]) => propertyName);
    for (const propertyName of forbidden) delete schema.properties[propertyName];
    if (forbidden.length > 0) schema['x-v2board-forbidden-properties'] = forbidden;
    for (const property of Object.values(schema.properties)) extractForbiddenProperties(property);
  }
  if (schema.items) extractForbiddenProperties(schema.items);
  if (schema.additionalProperties && typeof schema.additionalProperties === 'object') {
    extractForbiddenProperties(schema.additionalProperties);
  }
  if (schema.propertyNames && typeof schema.propertyNames === 'object') {
    extractForbiddenProperties(schema.propertyNames);
  }
  for (const keyword of ['oneOf', 'anyOf', 'allOf']) {
    for (const member of schema[keyword] ?? []) extractForbiddenProperties(member);
  }
}

function generatedTypeName(name) {
  return `InternalApi${name}`;
}

function generatedSchemaName(name) {
  return `internalApi${name[0].toUpperCase()}${name.slice(1)}Schema`;
}

function semanticRefSiblings(schema) {
  const annotationKeys = new Set([
    '$ref',
    'description',
    'title',
    'deprecated',
    'examples',
    'example',
    'default',
    'readOnly',
    'writeOnly',
    'externalDocs',
    'xml',
  ]);
  return Object.fromEntries(Object.entries(schema).filter(([name]) => !annotationKeys.has(name)));
}

function unionType(expressions) {
  const unique = [...new Set(expressions)];
  return unique.length === 1 ? unique[0] : unique.join(' | ');
}

function literalType(value) {
  assert.ok(
    value === null || ['string', 'number', 'boolean'].includes(typeof value),
    `unsupported non-primitive const: ${JSON.stringify(value)}`,
  );
  return JSON.stringify(value);
}

/**
 * Operation-level parameter/response schemas are today a small closed set of
 * scalars, arrays, enums, and plain `$ref`s — see the comment above
 * `heyApiObjectResolver` below. `nullable`/`const`/`oneOf`/`anyOf`/`allOf`/
 * OpenAPI-3.1 `type`-array unions and inline object shapes never occur at
 * this level (component-schema recursion, discriminated unions, and allOf
 * merging are entirely hey-api's job now); assert that rather than carrying
 * unreachable rendering branches for them.
 */
function typeExpression(schema) {
  if (schema === true || schema === undefined) return 'unknown';
  if (schema === false) return 'never';
  assert.ok(schema && typeof schema === 'object', `invalid OpenAPI schema: ${schema}`);
  assert.equal(schema.nullable, undefined, 'legacy OpenAPI 3.0 nullable is unsupported here');

  if (schema.$ref) {
    assert.equal(
      Object.keys(semanticRefSiblings(schema)).length,
      0,
      'operation-level $ref schemas with semantic siblings are unsupported here',
    );
    return generatedTypeName(refName(schema.$ref));
  }
  assert.equal(Object.hasOwn(schema, 'const'), false, 'operation-level const schemas are unsupported here');
  assert.equal(schema.oneOf, undefined, 'operation-level oneOf schemas are unsupported here');
  assert.equal(schema.anyOf, undefined, 'operation-level anyOf schemas are unsupported here');
  assert.equal(schema.allOf, undefined, 'operation-level allOf schemas are unsupported here');
  assert.ok(!Array.isArray(schema.type), 'operation-level OpenAPI 3.1 type-array unions are unsupported here');
  if (schema.enum) return unionType(schema.enum.map(literalType));

  switch (schema.type) {
    case 'null':
      return 'null';
    case 'boolean':
      return 'boolean';
    case 'integer':
    case 'number':
      return 'number';
    case 'string':
      return schema.format === 'binary' ? 'Blob' : 'string';
    case 'array':
      return `Array<${typeExpression(schema.items)}>`;
    case undefined:
      assert.equal(
        Object.keys(semanticRefSiblings(schema)).length,
        0,
        'operation-level typeless object schemas are unsupported here',
      );
      return 'unknown';
    default:
      throw new Error(`unsupported OpenAPI type schema: ${JSON.stringify(schema)}`);
  }
}

function zodUnion(expressions) {
  const unique = [...new Set(expressions)];
  if (unique.length === 1) return unique[0];
  return `z.union([${unique.join(', ')}])`;
}

function zodLiteral(value) {
  assert.ok(
    value === null || ['string', 'number', 'boolean'].includes(typeof value),
    `unsupported non-primitive const: ${JSON.stringify(value)}`,
  );
  return value === null ? 'z.null()' : `z.literal(${JSON.stringify(value)})`;
}

/** Mirrors typeExpression's narrowed scope — see the comment above it. */
function zodExpression(schema) {
  if (schema === true || schema === undefined) return 'z.unknown()';
  if (schema === false) return 'z.never()';
  assert.ok(schema && typeof schema === 'object', `invalid OpenAPI schema: ${schema}`);
  assert.equal(schema.nullable, undefined, 'legacy OpenAPI 3.0 nullable is unsupported here');

  if (schema.$ref) {
    assert.equal(
      Object.keys(semanticRefSiblings(schema)).length,
      0,
      'operation-level $ref schemas with semantic siblings are unsupported here',
    );
    return generatedSchemaName(refName(schema.$ref));
  }
  assert.equal(Object.hasOwn(schema, 'const'), false, 'operation-level const schemas are unsupported here');
  assert.equal(schema.oneOf, undefined, 'operation-level oneOf schemas are unsupported here');
  assert.equal(schema.anyOf, undefined, 'operation-level anyOf schemas are unsupported here');
  assert.equal(schema.allOf, undefined, 'operation-level allOf schemas are unsupported here');
  assert.ok(!Array.isArray(schema.type), 'operation-level OpenAPI 3.1 type-array unions are unsupported here');
  if (schema.enum) return zodUnion(schema.enum.map(zodLiteral));

  let expression;
  switch (schema.type) {
    case 'null':
      expression = 'z.null()';
      break;
    case 'boolean':
      expression = 'z.boolean()';
      break;
    case 'integer':
      expression = 'z.number().int()';
      if (schema.format === 'int32') {
        expression += '.min(-2147483648).max(2147483647)';
      }
      break;
    case 'number':
      expression = 'z.number()';
      break;
    case 'string':
      if (schema.format === 'date-time') expression = 'z.iso.datetime({ offset: true })';
      else if (schema.format === 'binary') expression = 'z.instanceof(Blob)';
      else expression = 'z.string()';
      break;
    case 'array':
      expression = `z.array(${zodExpression(schema.items)})`;
      break;
    case undefined:
      assert.equal(
        Object.keys(semanticRefSiblings(schema)).length,
        0,
        'operation-level typeless object schemas are unsupported here',
      );
      expression = 'z.unknown()';
      break;
    default:
      throw new Error(`unsupported OpenAPI zod schema: ${JSON.stringify(schema)}`);
  }
  // JSON Schema validation keywords apply only to instances of their own
  // type. This matters for OpenAPI 3.1 unions such as
  // `type: ["integer", "null"]`: numeric bounds must never be projected onto
  // the `null` member (which would generate the invalid `z.null().min(...)`).
  if (schema.type === 'integer' || schema.type === 'number') {
    if (schema.minimum !== undefined) expression += `.min(${schema.minimum})`;
    if (schema.maximum !== undefined) expression += `.max(${schema.maximum})`;
    if (schema.exclusiveMinimum !== undefined) expression += `.gt(${schema.exclusiveMinimum})`;
    if (schema.exclusiveMaximum !== undefined) expression += `.lt(${schema.exclusiveMaximum})`;
    if (schema.multipleOf !== undefined) expression += `.multipleOf(${schema.multipleOf})`;
  }
  if (schema.type === 'string') {
    if (schema.minLength !== undefined) expression += `.min(${schema.minLength})`;
    if (schema.maxLength !== undefined) expression += `.max(${schema.maxLength})`;
    if (schema['x-v2board-max-bytes'] !== undefined) {
      const maximumBytes = schema['x-v2board-max-bytes'];
      assert.ok(
        Number.isInteger(maximumBytes) && maximumBytes >= 0,
        `x-v2board-max-bytes must be a non-negative integer: ${maximumBytes}`,
      );
      expression += `.refine((value) => new TextEncoder().encode(value).length <= ${maximumBytes}, { message: "Must be at most ${maximumBytes} UTF-8 bytes" })`;
    }
    if (schema.pattern !== undefined) {
      expression += `.regex(new RegExp(${JSON.stringify(schema.pattern)}))`;
    }
  }
  if (schema.type === 'array') {
    if (schema.minItems !== undefined) expression += `.min(${schema.minItems})`;
    if (schema.maxItems !== undefined) expression += `.max(${schema.maxItems})`;
  }
  return expression;
}

function contentEntries(content) {
  return (
    Object.entries(content ?? {})
      .sort(([left], [right]) => left.localeCompare(right))
      // A declared media type without a schema permits an arbitrary body. It is
      // materially different from a response with no `content` member.
      .map(([mediaType, media]) => ({ mediaType, schema: media?.schema ?? true }))
  );
}

function parameterSchema(parameter) {
  if (parameter.schema) return parameter.schema;
  const contents = contentEntries(parameter.content);
  assert.ok(contents.length <= 1, `parameter ${parameter.name} declares multiple media types`);
  return contents[0]?.schema;
}

function operationParameters(pathItem, operation) {
  const merged = new Map();
  for (const rawParameter of [...(pathItem.parameters ?? []), ...(operation.parameters ?? [])]) {
    const parameter = resolveComponent(rawParameter, 'parameters');
    assert.ok(parameter?.name && parameter?.in, 'operation parameter has no name or location');
    assert.ok(
      parameterLocations.includes(parameter.in),
      `unsupported parameter location ${parameter.in}`,
    );
    if (parameter.in === 'path') {
      assert.equal(parameter.required, true, `path parameter ${parameter.name} must be required`);
    }
    const schema = parameterSchema(parameter);
    const style =
      parameter.style ??
      (parameter.in === 'query' || parameter.in === 'cookie' ? 'form' : 'simple');
    const explode = parameter.explode ?? style === 'form';
    if (parameter.in === 'query' && schema?.type === 'array') {
      assert.equal(style, 'form', `query array parameter ${parameter.name} must use style=form`);
      assert.equal(
        explode,
        true,
        `query array parameter ${parameter.name} must use explode=true repeated-key encoding`,
      );
    }
    merged.set(`${parameter.in}:${parameter.name}`, {
      name: parameter.name,
      location: parameter.in,
      required: parameter.in === 'path' || parameter.required === true,
      schema,
      style,
      explode,
    });
  }
  return [...merged.values()].sort(
    (left, right) =>
      parameterLocations.indexOf(left.location) - parameterLocations.indexOf(right.location) ||
      left.name.localeCompare(right.name),
  );
}

function responseHeaders(response) {
  return Object.entries(response.headers ?? {})
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, rawHeader]) => {
      const header = resolveComponent(rawHeader, 'headers');
      return { name, schema: parameterSchema({ ...header, name }) };
    });
}

function responseVariants(successResponses, { jsonOnly = false } = {}) {
  const variants = [];
  for (const response of successResponses) {
    if (response.content.length === 0) {
      if (!jsonOnly) variants.push({ empty: true });
      continue;
    }
    const content = jsonOnly
      ? response.content.filter(({ mediaType }) => mediaType === 'application/json')
      : response.content;
    for (const { schema } of content) variants.push({ empty: false, schema });
  }
  return variants;
}

function responseVariantsType(variants) {
  if (variants.length === 0) return 'undefined';
  return unionType(
    variants.map((variant) => (variant.empty ? 'undefined' : typeExpression(variant.schema))),
  );
}

function responseVariantsRuntime(variants) {
  if (variants.length === 0) return 'z.undefined()';
  return zodUnion(
    variants.map((variant) => (variant.empty ? 'z.undefined()' : zodExpression(variant.schema))),
  );
}

function operations() {
  const found = [];
  for (const [operationPath, pathItem] of Object.entries(spec.paths ?? {})) {
    for (const method of httpMethods) {
      const operation = pathItem[method];
      if (!operation) continue;
      assert.ok(
        operation.operationId,
        `${method.toUpperCase()} ${operationPath} has no operationId`,
      );

      const requestBody = operation.requestBody
        ? resolveComponent(operation.requestBody, 'requestBodies')
        : undefined;
      const requestContent = contentEntries(requestBody?.content);
      const request = requestContent.find(
        ({ mediaType }) => mediaType === 'application/json',
      )?.schema;

      const responseEntries = Object.entries(operation.responses ?? {});
      const ambiguousSuccessStatuses = responseEntries
        .map(([status]) => status)
        .filter((status) => /^[23]xx$/i.test(status));
      assert.deepEqual(
        ambiguousSuccessStatuses,
        [],
        `${method.toUpperCase()} ${operationPath} must pin exact success statuses; ` +
          `ambiguous OpenAPI response keys are unsupported: ${ambiguousSuccessStatuses.join(', ')}`,
      );
      const successResponses = responseEntries
        .filter(([status]) => /^[23]\d\d$/.test(status))
        .map(([status, rawResponse]) => {
          const response = resolveComponent(rawResponse, 'responses');
          return {
            status: Number(status),
            content: contentEntries(response?.content),
            headers: responseHeaders(response ?? {}),
          };
        })
        .sort((left, right) => left.status - right.status);
      assert.ok(
        successResponses.length > 0,
        `${method.toUpperCase()} ${operationPath} must declare at least one exact 2xx or 3xx response`,
      );

      const successStatus = successResponses.length === 1 ? successResponses[0].status : undefined;
      const responses = responseVariants(successResponses);
      const jsonResponses = responseVariants(successResponses, { jsonOnly: true });
      const adminPrefix = '/api/v1/{secure_path}';
      found.push({
        id: operation.operationId,
        method: method.toUpperCase(),
        path: operationPath,
        adminPath: operationPath.startsWith(adminPrefix)
          ? operationPath.slice(adminPrefix.length)
          : null,
        parameters: operationParameters(pathItem, operation),
        requestRequired: requestBody?.required === true,
        requestContent,
        request,
        successStatus,
        successResponses,
        responses,
        jsonResponses,
      });
    }
  }
  return found.sort((left, right) => left.id.localeCompare(right.id));
}

function parametersType(parameters) {
  const groups = [];
  for (const location of parameterLocations) {
    const entries = parameters.filter((parameter) => parameter.location === location);
    if (entries.length === 0) continue;
    groups.push(
      `${JSON.stringify(location)}: { ${entries
        .map(
          (parameter) =>
            `${JSON.stringify(parameter.name)}${parameter.required ? '' : '?'}: ${typeExpression(parameter.schema)}`,
        )
        .join('; ')} }`,
    );
  }
  return groups.length === 0 ? emptyObjectType : `{ ${groups.join('; ')} }`;
}

function contentType(content) {
  if (content.length === 0) return emptyObjectType;
  return `{ ${content
    .map(
      ({ mediaType, schema }) =>
        `${JSON.stringify(mediaType)}: ${schema ? typeExpression(schema) : 'undefined'}`,
    )
    .join('; ')} }`;
}

function successResponsesType(responses) {
  return `{ ${responses
    .map((response) => {
      const headers =
        response.headers.length === 0
          ? emptyObjectType
          : `{ ${response.headers
              .map(
                ({ name, schema }) =>
                  `${JSON.stringify(name)}: ${schema ? typeExpression(schema) : 'unknown'}`,
              )
              .join('; ')} }`;
      return `${response.status}: { content: ${contentType(response.content)}; headers: ${headers} }`;
    })
    .join('; ')} }`;
}

// Component-schema type/Zod compilation (recursion, discriminated unions,
// allOf merging, additionalProperties policy) is delegated to
// `@hey-api/openapi-ts`, driven with the resolvers below. Everything above
// this point (`typeExpression`/`zodExpression` and friends) only still fires
// for operation-level inline parameter/response schemas, which today are a
// small closed set of scalars with no recursion or unions — see
// docs/adr/0009-hand-written-openapi-codegen-vs-off-the-shelf.md.

/**
 * hey-api's default object resolver (`additionalPropertiesNode` in its
 * bundled `dist/init-*.mjs`, corresponding to
 * `src/plugins/zod/v4/toAst/object.ts`) skips `additionalProperties`
 * entirely once a schema has named properties. Two distinct bugs follow:
 * `additionalProperties: false` silently produces a permissive
 * `z.object(...)` with no `.strict()` (the source spec's boolean `false` is
 * also normalized into `{ type: "never" }` inside hey-api's own IR before
 * resolvers see it — an internal implementation detail, not part of the
 * `$resolvers` type contract, that could change silently in a future hey-api
 * release without a compile error); and an explicitly open schema with named
 * properties (e.g. `ProblemDetails`) instead gets a plain `z.object(...)`,
 * which zod *strips* unknown keys from by default — silently disagreeing
 * with its own generated `[key: string]: unknown` TypeScript type, which
 * promises they survive. Both are fixed below without touching hey-api's
 * already-correct handling of property-less open maps (`z.record(...)`).
 */
function heyApiObjectResolver(ctx) {
  const { schema, nodes, $ } = ctx;
  const z = ctx.plugin.imports.z;
  let base = nodes.base(ctx);
  const additional = schema.additionalProperties;
  const closed =
    additional === false || (additional && typeof additional === 'object' && additional.type === 'never');
  const hasProperties = Object.keys(schema.properties ?? {}).length > 0;
  // `extractForbiddenProperties` above moved every literal-`false`-valued
  // property (hey-api would otherwise drop it before this resolver ever
  // runs) onto this vendor-extension array. Re-add each as an explicit
  // optional-never key so the key is rejected instead of silently passed
  // through by `.catchall()` below (types.gen.ts stays imprecise for these
  // keys — typed `unknown` via the surrounding index signature rather than
  // `never` — since no consumer reads a forbidden key on a specific
  // discriminator arm).
  const forbiddenNames = schema['x-v2board-forbidden-properties'] ?? [];
  if (forbiddenNames.length > 0) {
    let extension = $.object();
    for (const name of forbiddenNames) {
      let never = $(z).attr('never').call();
      never = never.attr('optional').call();
      extension = extension.prop(name, never);
    }
    base = base.attr('extend').call(extension);
  }
  // A closed, property-less object (`{}`) already renders as hey-api's own
  // `z.record(z.string(), z.never())` (no key can validly exist), which has
  // no `.strict()` method; only a real `z.object(...)` base needs it.
  if (closed) return hasProperties ? base.attr('strict').call() : base;
  if (!hasProperties) return base;
  const open =
    additional === true || (additional && typeof additional === 'object' && additional.type === 'unknown');
  assert.ok(
    open,
    'a schema-typed additionalProperties combined with named properties has no hey-api $resolvers catchall support here',
  );
  return base.attr('catchall').call($(z).attr('unknown').call());
}

/**
 * hey-api coerces every `format: int64`/`uint64` number to `z.coerce.bigint()`
 * with no plugin-level opt-out (`shouldCoerceToBigInt` in its bundled source
 * is a hard-coded format check). The rest of this codebase — hand-written
 * entity types, app code, JSON (de)serialization — assumes `number` for every
 * integer field including int64, matching the old generator's behavior, so
 * bigint coercion must be suppressed explicitly. int32 and unformatted
 * numbers are left to hey-api's own (already-correct, non-bigint) default.
 */
function heyApiNumberResolver(ctx) {
  const { schema, $ } = ctx;
  if (schema.format !== 'int64' && schema.format !== 'uint64') return undefined;
  const z = ctx.plugin.imports.z;
  let node = $(z).attr('number').call();
  if (schema.type === 'integer') node = node.attr('int').call();
  if (schema.minimum !== undefined) node = node.attr('min').call($.literal(schema.minimum));
  if (schema.maximum !== undefined) node = node.attr('max').call($.literal(schema.maximum));
  if (schema.exclusiveMinimum !== undefined) node = node.attr('gt').call($.literal(schema.exclusiveMinimum));
  if (schema.exclusiveMaximum !== undefined) node = node.attr('lt').call($.literal(schema.exclusiveMaximum));
  if (schema.multipleOf !== undefined) node = node.attr('multipleOf').call($.literal(schema.multipleOf));
  return node;
}

/**
 * `x-v2board-max-bytes` (a v2board vendor extension: JSON Schema's
 * `maxLength` counts Unicode characters, never UTF-8 bytes, and the Rust
 * backend's real limits are byte-based) has no hey-api extension point today
 * because none of the 207 component schemas currently carry it — every
 * occurrence in the live spec is on an inline operation parameter, compiled
 * by the retained `zodExpression` above instead. This resolver exists so a
 * future component-schema field carrying the extension is still enforced
 * rather than silently unvalidated.
 */
function heyApiStringResolver(ctx) {
  const { schema, nodes, $ } = ctx;
  const constNode = nodes.const(ctx);
  if (constNode) {
    ctx.chain.current = constNode;
  } else {
    const baseNode = nodes.base(ctx);
    if (baseNode) ctx.chain.current = baseNode;
    const formatNode = nodes.format(ctx);
    if (formatNode) ctx.chain.current = formatNode;
    const lengthNode = nodes.length(ctx);
    if (lengthNode) ctx.chain.current = lengthNode;
    else {
      const minLengthNode = nodes.minLength(ctx);
      if (minLengthNode) ctx.chain.current = minLengthNode;
      const maxLengthNode = nodes.maxLength(ctx);
      if (maxLengthNode) ctx.chain.current = maxLengthNode;
    }
    const patternNode = nodes.pattern(ctx);
    if (patternNode) ctx.chain.current = patternNode;
  }
  const maxBytes = schema['x-v2board-max-bytes'];
  if (typeof maxBytes === 'number') {
    const predicate = $.func((f) => {
      f.param('value');
      f.do(
        $.binary(
          $.new('TextEncoder').args().attr('encode').call($('value')).attr('length'),
          '<=',
          $.literal(maxBytes),
        ).return(),
      );
    });
    ctx.chain.current = ctx.chain.current
      .attr('refine')
      .call(predicate, $.object().prop('error', $.literal(`Must be at most ${maxBytes} UTF-8 bytes`)));
  }
  return ctx.chain.current;
}

async function generateHeyApiOutput() {
  const outputDir = check ? await mkdtemp(path.join(tmpdir(), 'v2board-hey-api-')) : heyApiOutputDir;
  await createClient({
    // A clone, not `spec` itself: `extractForbiddenProperties` mutates the
    // schema it's given in place, and the retained `typeExpression`/
    // `zodExpression` functions must keep seeing the original spec shape.
    input: structuredClone(spec),
    output: outputDir,
    // hey-api's documented `parser.patch.schemas` hook runs on its own
    // bundled copy of the input, before parsing/IR construction — the same
    // point a hand-rolled preprocessing pass would need to run at, without a
    // scratch spec file on disk.
    parser: { patch: { schemas: (_name, schema) => extractForbiddenProperties(schema) } },
    plugins: [
      '@hey-api/typescript',
      {
        name: 'zod',
        requests: false,
        responses: false,
        $resolvers: {
          object: heyApiObjectResolver,
          number: heyApiNumberResolver,
          string: heyApiStringResolver,
        },
      },
    ],
  });
  await rm(path.join(outputDir, 'index.ts'), { force: true });
  const typesSource = await readFile(path.join(outputDir, 'types.gen.ts'), 'utf8');
  const zodSource = await readFile(path.join(outputDir, 'zod.gen.ts'), 'utf8');
  if (check) await rm(outputDir, { recursive: true, force: true });
  return { typesSource, zodSource };
}

function normalizedKey(name) {
  return name.toLowerCase().replaceAll(/[^a-z0-9]/g, '');
}

/**
 * hey-api applies its own acronym-casing normalization to component schema
 * names (for example `AlipayF2FConfig` becomes `AlipayF2fConfig`), so its
 * generated export names cannot be predicted by a fixed string transform.
 * Match them by normalized (case- and punctuation-insensitive) identity
 * instead, so `internalApi<Name>Schema`/`InternalApi<Name>` keep the OpenAPI
 * document's own naming convention regardless of hey-api's internal choice.
 */
function buildGeneratedNameIndex(source, pattern, unwrap) {
  const byNormalizedName = new Map();
  for (const match of source.matchAll(pattern)) {
    const exported = match[1];
    const key = normalizedKey(unwrap(exported));
    assert.ok(
      !byNormalizedName.has(key),
      `ambiguous generated export name collision at ${exported}`,
    );
    byNormalizedName.set(key, exported);
  }
  return function lookup(schemaName) {
    const found = byNormalizedName.get(normalizedKey(schemaName));
    assert.ok(found, `no hey-api generated export found for schema ${schemaName}`);
    return found;
  };
}

const { typesSource: heyApiTypesSource, zodSource: heyApiZodSource } = await generateHeyApiOutput();
const heyApiTypeName = buildGeneratedNameIndex(heyApiTypesSource, /^export type (\w+) =/gm, (name) => name);
const heyApiZodName = buildGeneratedNameIndex(
  heyApiZodSource,
  /^export const (\w+) =/gm,
  (name) => name.replace(/^z/, ''),
);

function renderTypes() {
  const schemaNames = Object.keys(schemas).sort();
  const blocks = [
    '// @generated by scripts/generate-internal-api-contract.mjs; do not edit.',
    '// Source: backend/rust/crates/api-contract.',
    schemaNames.length === 0
      ? ''
      : `import type { ${schemaNames.map(heyApiTypeName).join(', ')} } from './hey-api/types.gen';`,
    // Component-schema Zod validators are re-exported from here (not just
    // types) so `@v2board/api-client`'s generated runtime file can reach them
    // through the existing root `@v2board/types` import, without a deep
    // subpath export.
    schemaNames.length === 0
      ? ''
      : `import { ${schemaNames.map(heyApiZodName).join(', ')} } from './hey-api/zod.gen';`,
    '',
  ];
  for (const name of schemaNames) {
    blocks.push(`export type ${generatedTypeName(name)} = ${heyApiTypeName(name)};`, '');
  }
  for (const name of schemaNames) {
    blocks.push(`export const ${generatedSchemaName(name)} = ${heyApiZodName(name)};`, '');
  }
  if (schemaNames.length === 0) {
    blocks.push(`export type InternalApiSchemaMap = ${emptyObjectType};`, '');
  } else {
    blocks.push('export interface InternalApiSchemaMap {');
    for (const name of schemaNames) {
      blocks.push(`  ${JSON.stringify(name)}: ${generatedTypeName(name)};`);
    }
    blocks.push('}', '');
  }
  const operationDefinitions = operations();
  if (operationDefinitions.length === 0) {
    blocks.push(`export type InternalApiOperationMap = ${emptyObjectType};`);
  } else {
    blocks.push('export interface InternalApiOperationMap {');
    for (const operation of operationDefinitions) {
      blocks.push(`  ${JSON.stringify(operation.id)}: {`);
      blocks.push(`    method: ${JSON.stringify(operation.method)};`);
      blocks.push(`    path: ${JSON.stringify(operation.path)};`);
      blocks.push(
        `    successStatus: ${operation.successStatus === undefined ? 'undefined' : operation.successStatus};`,
      );
      blocks.push(`    parameters: ${parametersType(operation.parameters)};`);
      blocks.push(`    requestRequired: ${operation.requestRequired};`);
      blocks.push(`    requestContent: ${contentType(operation.requestContent)};`);
      blocks.push(
        `    request: ${operation.request ? typeExpression(operation.request) : 'undefined'};`,
      );
      blocks.push(`    successResponses: ${successResponsesType(operation.successResponses)};`);
      blocks.push(`    response: ${responseVariantsType(operation.responses)};`);
      blocks.push(`    jsonResponse: ${responseVariantsType(operation.jsonResponses)};`);
      blocks.push('  };');
    }
    blocks.push('}');
  }
  return `${blocks.join('\n')}\n`;
}

function parametersRuntime(parameters) {
  const groups = [];
  for (const location of parameterLocations) {
    const entries = parameters.filter((parameter) => parameter.location === location);
    if (entries.length === 0) continue;
    groups.push(
      `      ${JSON.stringify(location)}: z.strictObject({\n${entries
        .map(
          (parameter) =>
            `        ${JSON.stringify(parameter.name)}: ${zodExpression(parameter.schema)}${parameter.required ? '' : '.optional()'},`,
        )
        .join('\n')}\n      }),`,
    );
  }
  return groups.length === 0 ? '{}' : `{\n${groups.join('\n')}\n    }`;
}

function contentRuntime(content, indentation) {
  if (content.length === 0) return '{}';
  const spaces = ' '.repeat(indentation);
  const closing = ' '.repeat(Math.max(0, indentation - 2));
  return `{\n${content
    .map(
      ({ mediaType, schema }) =>
        `${spaces}${JSON.stringify(mediaType)}: ${schema ? zodExpression(schema) : 'z.undefined()'},`,
    )
    .join('\n')}\n${closing}}`;
}

function successResponsesRuntime(responses) {
  return `{\n${responses
    .map(
      (response) =>
        `      ${response.status}: {\n        content: ${contentRuntime(response.content, 10)},\n        headers: ${
          response.headers.length === 0
            ? '{}'
            : `{\n${response.headers
                .map(
                  ({ name, schema }) =>
                    `          ${JSON.stringify(name)}: ${schema ? zodExpression(schema) : 'z.unknown()'},`,
                )
                .join('\n')}\n        }`
        },\n      },`,
    )
    .join('\n')}\n    }`;
}

function renderRuntime() {
  const schemaNames = Object.keys(schemas).sort();
  const blocks = [
    '// @generated by scripts/generate-internal-api-contract.mjs; do not edit.',
    '// Source: backend/rust/crates/api-contract.',
    "import { z } from 'zod';",
    // Component-schema Zod validators live in @v2board/types (their
    // hey-api-generated source sits under packages/types/src/generated/hey-api,
    // alongside the compile-time types) and are re-exported through the
    // package's existing root entry point. Operation schemas below reference
    // them by their generatedSchemaName identifier, and this file re-exports
    // the same identifiers so existing direct consumers of
    // `./generated/internal-api` keep working unchanged.
    schemaNames.length === 0
      ? ''
      : `import { ${schemaNames.map(generatedSchemaName).join(', ')} } from '@v2board/types';`,
    schemaNames.length === 0 ? '' : `export { ${schemaNames.map(generatedSchemaName).join(', ')} };`,
    '',
  ];
  blocks.push('export const internalApiOperations = {');
  for (const operation of operations()) {
    blocks.push(`  ${JSON.stringify(operation.id)}: {`);
    blocks.push(`    method: ${JSON.stringify(operation.method)},`);
    blocks.push(`    path: ${JSON.stringify(operation.path)},`);
    blocks.push(
      `    successStatus: ${operation.successStatus === undefined ? 'undefined' : operation.successStatus},`,
    );
    if (operation.adminPath !== null) {
      blocks.push(`    adminPath: ${JSON.stringify(operation.adminPath)},`);
    }
    blocks.push(`    parameters: ${parametersRuntime(operation.parameters)},`);
    blocks.push(`    requestRequired: ${operation.requestRequired},`);
    blocks.push(`    requestContent: ${contentRuntime(operation.requestContent, 6)},`);
    blocks.push(
      `    requestSchema: ${operation.request ? zodExpression(operation.request) : 'z.undefined()'},`,
    );
    blocks.push(`    successResponses: ${successResponsesRuntime(operation.successResponses)},`);
    blocks.push(`    responseSchema: ${responseVariantsRuntime(operation.responses)},`);
    blocks.push(`    jsonResponseSchema: ${responseVariantsRuntime(operation.jsonResponses)},`);
    blocks.push('  },');
  }
  blocks.push(
    '} as const;',
    '',
    '/** Expand a generated OpenAPI path without accepting missing parameters. */',
    'export function internalApiPath(',
    '  template: string,',
    '  parameters: Readonly<Record<string, string | number>> = {},',
    '): string {',
    '  return template.replace(/\\{([^}]+)\\}/g, (_match, name: string) => {',
    '    const value = parameters[name];',
    '    if (value === undefined) throw new TypeError(`Missing internal API path parameter: ${name}`);',
    '    return encodeURIComponent(String(value));',
    '  });',
    '}',
  );
  return `${blocks.join('\n')}\n`;
}

async function emit(file, content) {
  if (check) {
    const existing = await readFile(file, 'utf8');
    assert.equal(existing, content, `${path.relative(root, file)} is stale; regenerate it`);
    return;
  }
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, content);
}

await emit(path.join(heyApiOutputDir, 'types.gen.ts'), heyApiTypesSource);
await emit(path.join(heyApiOutputDir, 'zod.gen.ts'), heyApiZodSource);
await emit(typesPath, renderTypes());
await emit(runtimePath, renderRuntime());
