# Backend architecture

The native backend follows an inward-facing application/ports-and-adapters
architecture. Crate placement is part of the design: transport, persistence,
Redis, cryptography, SMTP, upstream HTTP, clocks, and runtime configuration do
not enter the application core.

```text
generated TypeScript/Zod <--- OpenAPI <--- transport contract (`api-contract`)
                                                ^
                                                |
HTTP inbound adapter (`api`) -- DTO <--> command/view mapping
                                                |
                                                v
                         use cases + outbound ports (`application`)
                                                |
                                                v
                               pure policy/value objects (`domain-model`)
                                                ^
                                                |
 PostgreSQL (`db`) + production `*-adapters` implement the outbound ports

RFC 9457 runtime (`compat`) <--- code registry (`problem-code`)
                                      ---> OpenAPI projection (`api-contract`)
```

## Inward core

- `domain-model` owns infrastructure-free business value objects and policies.
  Its normal, build, and dev dependency sets are empty. It cannot know about
  SQL rows, Redis keys, HTTP DTOs, async runtimes, configuration files, or wire
  serialization.
- `application` owns use-case orchestration, commands, views, business errors,
  and outbound port traits. Its exact normal dependency allowlist is
  `thiserror` plus `v2board-domain-model`; it has no build or dev dependencies.
  Application source is also checked for transport and infrastructure imports.
  Transactions and atomicity required by a use case are expressed through a
  port operation, not reconstructed across several calls by an inbound handler.
- `api-contract` owns the internal HTTP transport vocabulary and the canonical
  158-operation registry. It is independent of application and persistence;
  `application` is likewise independent of it. The HTTP adapter performs the
  explicit conversion between transport DTOs and application commands/views.
- `problem-code` is a zero-dependency registry for all 101 internal RFC 9457
  problem codes and their status/title assignments. `compat` projects it into
  runtime responses while `api-contract` projects the same registry into
  OpenAPI, preventing runtime and generated clients from drifting apart.

## Ports and adapters

- `db` is the PostgreSQL adapter. Repository implementations satisfy
  application ports with typed parameters and transaction-scoped operations;
  SQL does not live in Axum handlers or application services.
- The production adapter crates own integration-specific behavior:
  `auth-adapters`, `configuration-adapters`, `http-adapters`, `mail-adapters`,
  `order-adapters`, `payment-adapters`, `redis-adapters`, `server-adapters`, and
  `subscription-adapters`. Examples include password hashing, session caches,
  operator-configuration activation, bounded upstream bodies, SMTP/outbox,
  clocks and identifiers, encrypted provider secrets, Redis admission, node
  credentials, and subscription-token minting.
- `api` is the HTTP composition root and inbound adapter; `workers` is the
  background-process composition root. They construct concrete adapters and
  call application services. Frozen external namespaces keep their wire bytes
  at the inbound boundary while still using the same application ports behind
  that boundary.
- `config` loads and validates typed boot/runtime configuration. It does not own
  integration algorithms. Provider calls and derived integration secrets stay
  in their production adapter crates.
- `provision` and `lifecycle` form the separate, one-shot legacy MySQL import
  boundary. They are not a second runtime architecture and cannot leak MySQL
  into the API or worker dependency graphs.

## Complete internal transport contract

The registry drives both Axum method/path registration and OpenAPI 3.1. It pins
path/query parameters, shared and operation-specific headers, request-body
presence, authentication, exact success status/media sets, and default RFC 9457
responses for all 158 internal operations.

All 64 JSON request bodies and all 95 JSON success representations have named,
field-level DTO roots. The recursive `JsonValue` schema is absent. Structural
objects are explicitly closed by default; the only open structures are the
reviewed, typed dynamic-map islands (for example provider manifest fields,
queue names, transport headers, and DNS host keys) plus RFC 9457 extension
members. The generated TypeScript types and Zod validators preserve those same
closed/open policies. Endpoint wrappers may consume only named generated
operations and cannot reopen a business DTO with `unknown`, `any`, or a
handwritten permissive schema.

## Enforced boundaries

Architecture is executable rather than aspirational:

- `application/tests/dependency_direction.rs` checks the exact dependency
  allowlist, rejects infrastructure imports, and verifies that business inbound
  handlers use application services while PostgreSQL/external adapters implement
  their ports.
- `domain-model/tests/dependency_direction.rs` inspects Cargo's resolved graph,
  including normal, build, and dev edges, for the pure model, problem registry,
  contract crate, and HTTP/application boundary.
- `api-contract` and frontend coverage tests pin the 158-operation set, the
  64/95 named JSON roots, closed-object policy, local reference resolution,
  problem registry, generated artifacts, and route drift.
- The contract SQL inventory and live prepare gates account for every dynamic
  SQL builder in runtime adapter crates. Real PostgreSQL repository tests cover
  transaction- and constraint-sensitive behavior.

New business policy belongs in `domain-model`; orchestration and ports belong in
`application`; transport DTOs belong in `api-contract`; infrastructure belongs
in `db` or a focused outer-adapter crate.
