import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

const generator = path.resolve('scripts/generate-internal-api-contract.mjs');

async function runGenerator(spec) {
  const root = await mkdtemp(path.join(tmpdir(), 'v2board-api-contract-'));
  try {
    const specDirectory = path.join(root, 'packages/api-client/openapi');
    await mkdir(specDirectory, { recursive: true });
    await writeFile(path.join(specDirectory, 'internal-api.openapi.json'), JSON.stringify(spec));
    const result = spawnSync(process.execPath, [generator, `--root=${root}`], {
      cwd: process.cwd(),
      encoding: 'utf8',
    });
    return {
      result,
      types:
        result.status === 0
          ? await readFile(path.join(root, 'packages/types/src/generated/internal-api.ts'), 'utf8')
          : undefined,
      runtime:
        result.status === 0
          ? await readFile(
              path.join(root, 'packages/api-client/src/generated/internal-api.ts'),
              'utf8',
            )
          : undefined,
    };
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

function oneOperationSpec(responses) {
  return {
    openapi: '3.1.0',
    components: { schemas: {} },
    paths: {
      '/api/v1/example': {
        get: { operationId: 'exampleGet', responses },
      },
    },
  };
}

test('checked-in internal API bindings match the Rust-exported OpenAPI document', () => {
  const result = spawnSync(
    process.execPath,
    ['scripts/generate-internal-api-contract.mjs', '--check'],
    { cwd: process.cwd(), encoding: 'utf8' },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
});

test('generated Zod integer schemas do not use the deprecated safe modifier', async () => {
  const generated = await readFile('packages/api-client/src/generated/internal-api.ts', 'utf8');

  assert.doesNotMatch(generated, /\.safe\(\)/);
});

test('generator preserves multiple exact success statuses without inventing a primary status', async () => {
  const { result, types, runtime } = await runGenerator(
    oneOperationSpec({
      200: {
        description: 'OK',
        content: { 'application/json': { schema: { type: 'string' } } },
      },
      204: { description: 'No content' },
    }),
  );

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(types, /successStatus: undefined;/);
  assert.match(types, /parameters: Record<string, never>;/);
  assert.match(types, /requestContent: Record<string, never>;/);
  assert.match(types, /204: \{ content: Record<string, never>; headers: Record<string, never> \}/);
  assert.match(types, /200: .*headers: Record<string, never>/);
  assert.doesNotMatch(types, /\{\s*\}/);
  assert.match(types, /successResponses: \{ 200: .* 204:/);
  assert.match(runtime, /successStatus: undefined,/);
  assert.match(runtime, /200: \{/);
  assert.match(runtime, /204: \{/);
  assert.match(runtime, /responseSchema: z\.undefined\(\),/);
});

test('generator rejects an ambiguous OpenAPI 2XX response key', async () => {
  const { result } = await runGenerator(
    oneOperationSpec({
      '2XX': { description: 'Any success' },
    }),
  );

  assert.notEqual(result.status, 0);
  assert.match(result.stderr || result.stdout, /must pin exact success statuses/);
});

test('generator projects unions, intersections, consts, maps, nullable refs, parameters, media, refs, and redirects', async () => {
  const { result, types, runtime } = await runGenerator({
    openapi: '3.1.0',
    components: {
      schemas: {
        Cat: {
          type: 'object',
          additionalProperties: false,
          required: ['kind', 'lives'],
          properties: { kind: { const: 'cat' }, lives: { type: 'integer' } },
        },
        Dog: {
          type: 'object',
          additionalProperties: false,
          required: ['kind', 'good'],
          properties: { kind: { const: 'dog' }, good: { type: 'boolean' } },
        },
        Pet: {
          oneOf: [{ $ref: '#/components/schemas/Cat' }, { $ref: '#/components/schemas/Dog' }],
          discriminator: { propertyName: 'kind' },
        },
        Flexible: { anyOf: [{ type: 'string' }, { type: 'integer' }] },
        Combined: {
          allOf: [
            { type: 'object', properties: { left: { type: 'string' } } },
            { type: 'object', properties: { right: { type: 'boolean' } } },
          ],
        },
        Labels: { type: 'object', additionalProperties: { type: 'string' } },
        NullablePet: {
          oneOf: [{ $ref: '#/components/schemas/Pet' }, { type: 'null' }],
        },
        BoundedNullableInteger: {
          type: ['integer', 'null'],
          minimum: 1,
          maximum: 9,
        },
        EmptyClosed: { type: 'object', additionalProperties: false },
      },
      parameters: {
        Trace: {
          name: 'X-Trace',
          in: 'header',
          schema: { type: 'string', 'x-v2board-max-bytes': 8 },
        },
      },
      requestBodies: {
        PetBody: {
          required: true,
          content: {
            'application/json': { schema: { $ref: '#/components/schemas/Pet' } },
            'text/plain': { schema: { type: 'string' } },
          },
        },
      },
      responses: {
        CsvExport: {
          description: 'CSV export',
          headers: { 'Content-Disposition': { schema: { type: 'string' } } },
          content: { 'text/csv': { schema: { type: 'string' } } },
        },
        Unconstrained: {
          description: 'Arbitrary JSON response',
          content: { 'application/json': {} },
        },
      },
    },
    paths: {
      '/api/v1/pets/{id}': {
        parameters: [{ name: 'id', in: 'path', required: true, schema: { type: 'integer' } }],
        post: {
          operationId: 'petsCreateOrExport',
          parameters: [
            { name: 'verbose', in: 'query', schema: { type: 'boolean' } },
            {
              name: 'reply_status',
              in: 'query',
              style: 'form',
              explode: true,
              schema: { type: 'array', items: { type: 'integer', format: 'int64' } },
            },
            { $ref: '#/components/parameters/Trace' },
          ],
          requestBody: { $ref: '#/components/requestBodies/PetBody' },
          responses: {
            201: {
              description: 'Created',
              content: { 'application/json': { schema: { $ref: '#/components/schemas/Pet' } } },
            },
            200: { $ref: '#/components/responses/CsvExport' },
            302: {
              description: 'See another resource',
              headers: { Location: { schema: { type: 'string' } } },
            },
            203: { $ref: '#/components/responses/Unconstrained' },
          },
        },
      },
    },
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(types, /InternalApiLabels = Record<string, string>/);
  assert.match(types, /InternalApiEmptyClosed = Record<string, never>/);
  assert.match(types, /InternalApiNullablePet = InternalApiPet \| null/);
  assert.match(types, /InternalApiCombined = .* & /);
  assert.match(runtime, /z\.discriminatedUnion\("kind"/);
  assert.match(runtime, /internalApiLabelsSchema = z\.record\(z\.string\(\), z\.string\(\)\)/);
  assert.match(
    runtime,
    /internalApiBoundedNullableIntegerSchema = z\.union\(\[z\.number\(\)\.int\(\)\.min\(1\)\.max\(9\), z\.null\(\)\]\)/,
  );
  assert.doesNotMatch(runtime, /z\.null\(\)\.(?:min|max|gt|lt|multipleOf)\(/);
  assert.match(runtime, /"path": z\.strictObject/);
  assert.match(runtime, /"query": z\.strictObject/);
  assert.match(types, /"reply_status"\?: Array<number>/);
  assert.match(runtime, /"reply_status": z\.array\(z\.number\(\)\.int\(\)\)\.optional\(\)/);
  assert.match(runtime, /"header": z\.strictObject/);
  assert.match(runtime, /new TextEncoder\(\)\.encode\(value\)\.length <= 8/);
  assert.match(runtime, /"text\/plain": z\.string\(\)/);
  assert.match(runtime, /"text\/csv": z\.string\(\)/);
  assert.match(runtime, /203: \{[\s\S]*?"application\/json": z\.unknown\(\)/);
  assert.match(types, /"Content-Disposition": string/);
  assert.doesNotMatch(types, /"Content-Disposition"\?:/);
  assert.match(runtime, /"Content-Disposition": z\.string\(\),/);
  assert.doesNotMatch(runtime, /"Content-Disposition": z\.string\(\)\.optional\(\)/);
  assert.match(runtime, /302: \{/);
  assert.match(types, /"Location": string/);
  assert.match(runtime, /"Location": z\.string\(\),/);
});

test('generator rejects query arrays without repeated-key form encoding', async () => {
  const spec = oneOperationSpec({ 200: { description: 'OK' } });
  spec.paths['/api/v1/example'].get.parameters = [
    {
      name: 'reply_status',
      in: 'query',
      style: 'form',
      explode: false,
      schema: { type: 'array', items: { type: 'integer' } },
    },
  ];
  const { result } = await runGenerator(spec);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr || result.stdout, /explode=true repeated-key encoding/);
});
