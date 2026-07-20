# Backend architecture

The backend uses inward-facing dependency boundaries rather than treating the
historical crate layout as the design authority.

```text
generated TypeScript/Zod <--- OpenAPI <--- transport contract (`api-contract`)
                                               ^
                                               |
HTTP adapter (`api`) -- maps DTOs <--> application commands/views
                                               |
                                               v
                         application services (`domain`, historical path)
                                               |
                                               v
                                  pure domain model (`domain-model`)
```

- `domain-model` owns infrastructure-free value objects and business policy.
  It must not depend on SQL, Redis, HTTP, async runtimes, configuration, or API
  serialization. Its direct normal, build, and dev allowlists are empty.
- `api-contract` owns generated-wire schemas and OpenAPI metadata. It must not
  depend on application services or persistence, and application services must
  not depend on it. Its direct normal allowlist is limited to `anyhow`,
  `chrono`, `serde`, `serde_json`, and `utoipa`; its build and dev allowlists
  are empty. The HTTP adapter performs the explicit conversion.
- `domain` is the historical path for the application layer. It coordinates
  use cases and depends inward on the pure model; it is not a domain-entity
  bucket and may not import HTTP DTOs or server-transport crates such as Axum,
  Tower, Hyper, or Utoipa.
- `db`, lifecycle import code, API handlers, workers, and external providers
  remain integration boundaries. Legacy source fields and public wire names
  are translated there instead of leaking into the native schema or pure
  model. Existing direct infrastructure calls from the historical application
  crate are a bounded migration surface, not a model for new code.

The dependency-direction tests in `domain-model/tests` inspect Cargo's resolved
dependency graph (rather than grepping manifests), so dependency aliases cannot
bypass these boundaries. Direct normal/build dependencies use exact allowlists;
the current dev allowlists for `domain-model` and `api-contract` are empty, and
the `domain` transport exclusion is checked across normal, build, and dev edges.
Adding a test-only or build-script dependency therefore requires the same
explicit architecture review as a runtime dependency. New business concepts
should enter `domain-model`; new wire DTOs should enter `api-contract`;
orchestration stays in the application layer.
