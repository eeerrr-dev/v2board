import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const arguments_ = process.argv.slice(2);
const check = arguments_.includes('--check');
const rootArgument = arguments_.find((value) => value.startsWith('--root='));
const root = path.resolve(rootArgument?.slice('--root='.length) ?? process.cwd());
const specPath = path.join(root, 'packages/api-client/openapi/internal-api.openapi.json');
const typesPath = path.join(root, 'packages/types/src/generated/internal-api.ts');
const runtimePath = path.join(root, 'packages/api-client/src/generated/internal-api.ts');

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

function withNullable(schema, render) {
  if (schema?.nullable !== true) return null;
  const nonNullable = { ...schema };
  delete nonNullable.nullable;
  return render(nonNullable);
}

function unionType(expressions) {
  const unique = [...new Set(expressions)];
  return unique.length === 1 ? unique[0] : unique.join(' | ');
}

function intersectionType(expressions) {
  const unique = [...new Set(expressions)];
  return unique.length === 1
    ? unique[0]
    : unique.map((expression) => `(${expression})`).join(' & ');
}

function literalType(value) {
  assert.ok(
    value === null || ['string', 'number', 'boolean'].includes(typeof value),
    `unsupported non-primitive const: ${JSON.stringify(value)}`,
  );
  return JSON.stringify(value);
}

function objectTypeExpression(schema) {
  const required = new Set(schema.required ?? []);
  const properties = Object.entries(schema.properties ?? {})
    .sort(([left], [right]) => left.localeCompare(right))
    .map(
      ([name, property]) =>
        `${JSON.stringify(name)}${required.has(name) ? '' : '?'}: ${typeExpression(property)}`,
    );
  const additional = schema.additionalProperties;
  if (properties.length === 0) {
    if (additional === false) return emptyObjectType;
    return `Record<string, ${additional && additional !== true ? typeExpression(additional) : 'unknown'}>`;
  }
  // TypeScript cannot precisely express JSON Schema's "known properties plus
  // a differently typed additional-property set". Keep extensions available
  // without incorrectly constraining the declared properties.
  if (additional !== false) properties.push('[key: string]: unknown');
  return `{ ${properties.join('; ')} }`;
}

function typeExpression(schema) {
  if (schema === true || schema === undefined) return 'unknown';
  if (schema === false) return 'never';
  assert.ok(schema && typeof schema === 'object', `invalid OpenAPI schema: ${schema}`);

  const nullable = withNullable(schema, (nonNullable) => typeExpression(nonNullable));
  if (nullable) return `${nullable} | null`;

  if (schema.$ref) {
    const reference = generatedTypeName(refName(schema.$ref));
    const siblings = semanticRefSiblings(schema);
    return Object.keys(siblings).length === 0
      ? reference
      : intersectionType([reference, typeExpression(siblings)]);
  }
  if (Object.hasOwn(schema, 'const')) return literalType(schema.const);
  if (schema.oneOf) return unionType(schema.oneOf.map(typeExpression));
  if (schema.anyOf) return unionType(schema.anyOf.map(typeExpression));
  if (schema.allOf) return intersectionType(schema.allOf.map(typeExpression));
  if (Array.isArray(schema.type)) {
    return unionType(schema.type.map((type) => typeExpression({ ...schema, type })));
  }
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
    case 'object':
      return objectTypeExpression(schema);
    case undefined:
      return Object.keys(semanticRefSiblings(schema)).length === 0
        ? 'unknown'
        : objectTypeExpression(schema);
    default:
      throw new Error(`unsupported OpenAPI type schema: ${JSON.stringify(schema)}`);
  }
}

function zodUnion(expressions) {
  const unique = [...new Set(expressions)];
  if (unique.length === 1) return unique[0];
  return `z.union([${unique.join(', ')}])`;
}

function zodIntersection(expressions) {
  const unique = [...new Set(expressions)];
  if (unique.length === 1) return unique[0];
  return unique.slice(1).reduce((left, right) => `${left}.and(${right})`, unique[0]);
}

function zodLiteral(value) {
  assert.ok(
    value === null || ['string', 'number', 'boolean'].includes(typeof value),
    `unsupported non-primitive const: ${JSON.stringify(value)}`,
  );
  return value === null ? 'z.null()' : `z.literal(${JSON.stringify(value)})`;
}

function zodObjectExpression(schema) {
  const required = new Set(schema.required ?? []);
  const properties = Object.entries(schema.properties ?? {})
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, property]) => {
      const validator = zodExpression(property);
      return `  ${JSON.stringify(name)}: ${validator}${required.has(name) ? '' : '.optional()'},`;
    });
  const shape = `{
${properties.join('\n')}
}`;
  const additional = schema.additionalProperties;
  if (properties.length === 0 && additional !== false) {
    return `z.record(z.string(), ${additional && additional !== true ? zodExpression(additional) : 'z.unknown()'})`;
  }
  if (additional === false) return `z.strictObject(${shape})`;
  if (additional && additional !== true) {
    return `z.object(${shape}).catchall(${zodExpression(additional)})`;
  }
  return `z.looseObject(${shape})`;
}

function zodExpression(schema) {
  if (schema === true || schema === undefined) return 'z.unknown()';
  if (schema === false) return 'z.never()';
  assert.ok(schema && typeof schema === 'object', `invalid OpenAPI schema: ${schema}`);

  const nullable = withNullable(schema, (nonNullable) => zodExpression(nonNullable));
  if (nullable) return `${nullable}.nullable()`;

  if (schema.$ref) {
    const reference = generatedSchemaName(refName(schema.$ref));
    const siblings = semanticRefSiblings(schema);
    return Object.keys(siblings).length === 0
      ? reference
      : `${reference}.and(${zodExpression(siblings)})`;
  }
  if (Object.hasOwn(schema, 'const')) return zodLiteral(schema.const);
  if (schema.oneOf) {
    const members = schema.oneOf.map(zodExpression);
    if (schema.discriminator?.propertyName && members.length > 1) {
      return `z.discriminatedUnion(${JSON.stringify(schema.discriminator.propertyName)}, [${members.join(', ')}])`;
    }
    return zodUnion(members);
  }
  if (schema.anyOf) return zodUnion(schema.anyOf.map(zodExpression));
  if (schema.allOf) return zodIntersection(schema.allOf.map(zodExpression));
  if (Array.isArray(schema.type)) {
    return zodUnion(schema.type.map((type) => zodExpression({ ...schema, type })));
  }
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
    case 'object':
      expression = zodObjectExpression(schema);
      break;
    case undefined:
      expression =
        Object.keys(semanticRefSiblings(schema)).length === 0
          ? 'z.unknown()'
          : zodObjectExpression(schema);
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

function schemaDependencies(schema, output = new Set()) {
  if (!schema || typeof schema !== 'object') return output;
  if (schema.$ref) output.add(refName(schema.$ref));
  if (schema.items) schemaDependencies(schema.items, output);
  if (schema.additionalProperties && schema.additionalProperties !== true) {
    schemaDependencies(schema.additionalProperties, output);
  }
  for (const member of [
    ...(schema.oneOf ?? []),
    ...(schema.anyOf ?? []),
    ...(schema.allOf ?? []),
  ]) {
    schemaDependencies(member, output);
  }
  for (const property of Object.values(schema.properties ?? {})) {
    schemaDependencies(property, output);
  }
  return output;
}

function topologicalSchemaNames() {
  const ordered = [];
  const visiting = new Set();
  const visited = new Set();
  function visit(name) {
    assert.ok(schemas[name], `unknown component schema: ${name}`);
    if (visited.has(name)) return;
    assert.ok(
      !visiting.has(name),
      `recursive schema requires an explicit generator policy: ${name}`,
    );
    visiting.add(name);
    for (const dependency of [...schemaDependencies(schemas[name])].sort()) visit(dependency);
    visiting.delete(name);
    visited.add(name);
    ordered.push(name);
  }
  for (const name of Object.keys(schemas).sort()) visit(name);
  return ordered;
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

function primaryResponseSchema(successResponses) {
  if (successResponses.length !== 1) return undefined;
  const [{ content }] = successResponses;
  const json = content.find(({ mediaType }) => mediaType === 'application/json');
  if (json) return json.schema;
  return content.length === 1 ? content[0].schema : undefined;
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
        response: primaryResponseSchema(successResponses),
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

function renderTypes() {
  const blocks = [
    '// @generated by scripts/generate-internal-api-contract.mjs; do not edit.',
    '// Source: backend/rust/crates/api-contract.',
    '',
  ];
  const schemaNames = Object.keys(schemas).sort();
  for (const name of schemaNames) {
    blocks.push(`export type ${generatedTypeName(name)} = ${typeExpression(schemas[name])};`, '');
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
      blocks.push(
        `    response: ${operation.response ? typeExpression(operation.response) : 'undefined'};`,
      );
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
  const blocks = [
    '// @generated by scripts/generate-internal-api-contract.mjs; do not edit.',
    '// Source: backend/rust/crates/api-contract.',
    "import { z } from 'zod';",
    '',
  ];
  for (const name of topologicalSchemaNames()) {
    blocks.push(`export const ${generatedSchemaName(name)} = ${zodExpression(schemas[name])};`, '');
  }
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
    blocks.push(
      `    responseSchema: ${operation.response ? zodExpression(operation.response) : 'z.undefined()'},`,
    );
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

await emit(typesPath, renderTypes());
await emit(runtimePath, renderRuntime());
