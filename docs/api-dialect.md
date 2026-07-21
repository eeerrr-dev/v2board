# Internal API Dialect v2 — Design Specification

Status: **implemented and normative.** Every migration wave in Appendix A
(W0–W14) has landed; the Rust backend, the React frontend,
`@v2board/api-client`, fixtures, goldens, and the parity harness all speak
this dialect and nothing else. This document remains the single source of
truth for the internal dialect that replaced the legacy-inherited one (audit
items R1–R30): contract changes land as revisions to this specification in
the same commit series that ships them (notably the error-code registry).

Owner decisions fixed before this design (not open questions): all internal
namespaces modernize, including passport/auth (breaking third-party in-app
login is accepted); external namespaces stay byte-frozen (§2); RFC 9457
errors keyed by a stable `code` slug; JSON bodies + `Authorization: Bearer`;
no `{data}` envelope; resource-style routes; a JSON filter DSL; explicit
null-clear/absent-retain semantics; history routing with a
`legacy_hash_redirect_enable` client-side hash translator; `custom_html`
removed and CSP tightened; one canonical locale-persistence key; parity by
canonical semantics through per-world adapters; no dual-dialect compatibility
branches ship — each endpoint family switches atomically.

The current executable route inventory is the single 158-operation registry in
`backend/rust/crates/api-contract/src/operations.rs`. Axum mounts every
internal route by operation id and derives its method/path from that registry;
route-coverage, OpenAPI-generation, and golden-wire gates require every entry
to be bound exactly once. The initial draft was historically enumerated from
the then-current `crates/api` dispatchers and the now-retired
`crates/domain/src/admin.rs`; those paths are provenance, not current
architecture. No standalone R1–R30 checklist is retained.

---

## 1. Terminology and shape classes

Request shape classes used in the route tables:

| Class | Meaning |
| --- | --- |
| `none` | No body, no significant query params. |
| `query` | GET query parameters (scalars; `filter`/`sort_by`/`sort_dir`/`page`/`per_page` per §7–8). |
| `json` | `application/json` object body (§4). |

Response shape classes:

| Class | Meaning |
| --- | --- |
| `bare` | A single bare JSON object (no envelope). |
| `array` | A bare JSON array (non-paginated list). |
| `page` | `{ "items": [...], "total": n }` (paginated list, §8). |
| `union` | Discriminated union object (checkout, §9.3). |
| `csv/json` | `text/csv` attachment, or a bare JSON object when no file is produced. |
| `redirect` | `302 Found` with `Location`. |
| `empty` | `204 No Content`. |

All internal error responses are `application/problem+json` (§3). The `bare`
class replaces `{"data": ...}`; the `page` class replaces
`{"data": [...], "total": n}`; `array` replaces enveloped arrays.

Success statuses are pinned (fixtures and goldens compare real HTTP
semantics, so an unstated 200-vs-201 choice would drift between waves):

- Reads and body-returning actions are **200 OK** (`bare`, `array`, `page`,
  `union`, and `csv/json` — including the bulk generates, whose created
  identities travel inside the generated CSV/JSON itself).
- A `POST` that creates a resource the client subsequently addresses by
  identity returns **201 Created** with that identity in the body
  (`{"id": n}` / `{"trade_no": "..."}` at minimum; richer bodies where the
  row says so). No `Location` header — the SPA consumes the body, and the
  admin prefix is dynamic. Rows returning 201 are marked `(201)`. This
  replaces the legacy `{data: true}`-then-refetch flow for admin CRUD
  creates: the returned `{id}` feeds the follow-up `PATCH …/{id}` without a
  racy list refetch (recorded decision).
- `empty` responses are **204 No Content** (bodiless actions, updates,
  deletes — plus the one create deliberately kept on the refetch flow,
  `POST /user/invite-codes`, whose resource is never addressed
  individually afterwards).
- `redirect` is **302 Found**.
- The only **202 Accepted** in the dialect is admin `PATCH config`
  activation-pending (§6.1), with the committed operator `revision` in its
  body; it is a success outcome, not a problem+json error.

---

## 2. Frozen external namespaces (byte-frozen, out of scope)

These routes, their request/response bytes, auth mechanisms, and error bodies
(legacy `{message}` JSON, legacy statuses) do **not** change. They keep the
legacy `{data}` envelope where they have one, form/query parsing, epoch
timestamps, and `ApiError::legacy()` 500 semantics. The dialect migration
must leave every row below untouched:

| Method | Path | Consumer |
| --- | --- | --- |
| GET | `/api/v1/client/subscribe` | Subscription clients (token in query). |
| GET/HEAD | operator `subscribe_path` alias (fallback.rs) | Subscription clients. |
| GET | `/api/v1/client/app/getConfig` | Third-party client apps. |
| GET | `/api/v1/client/app/getVersion` | Third-party client apps. |
| GET/POST | `/api/v1/guest/payment/notify/{method}/{uuid}` | Payment providers (HMAC over raw bytes for several). |
| POST | `/api/v1/guest/telegram/webhook` | Telegram Bot API. |
| GET/POST | `/api/v1/server/{class}/{action}` | Node agents (`x-v2board-server-token`). |
| GET/POST | `/api/v2/server/config` | Node agents. |
| GET | `/healthz`, `/readyz` | Loopback-only probes. |
| GET | `/assets/user/*`, `/assets/admin/*` | Hashed static assets. |

Also frozen (non-route contracts):

- The subscribe-URL/token/flag scheme and everything the subscription
  responses encode.
- Stripe PaymentIntent metadata/webhook payloads and reCAPTCHA verification
  payloads.
- The user app's `localStorage` **`authorization`** persistence key and its
  stored value format (the raw `auth_data` token — the `Bearer` scheme in
  §4.2 is added on the wire only, never persisted). The admin app uses its
  own independently pinned `v2board.admin_auth_data` key instead (see
  `docs/adr/0003-opaque-session-tokens-redis-session-epoch.md`).
- Legacy locale keys `umi_locale` / the i18n cookie / `window.g_lang` as
  **one-time migration reads only** (§11).
- The MySQL import contract (docs/mysql-import*.md) — unaffected. (The
  importer has no `frontend_custom_html` mapping to drop, and none may be
  added; §12.)

Because these error bodies are frozen, the response-rewrite localization
middleware and its zh-CN message catalog remain in service for these
namespaces (§3.1); deleting them globally would change frozen external error
bytes.

Out of scope by owner decision: the vmess camelCase protocol-settings keys
(`networkSettings`, `tlsSettings`, `ruleSettings`, `dnsSettings` in server
save payloads and node rows) stay as-is for now (audit R22 deferred).

---

## 3. Error model — RFC 9457 `application/problem+json`

### 3.1 Shape

Every internal-route error response has status ≥ 400,
`Content-Type: application/problem+json`, and this body:

```json
{
  "type": "about:blank",
  "title": "Bad Request",
  "status": 400,
  "code": "plan_sold_out",
  "detail": "当前产品已售罄",
  "errors": { "email": ["邮箱格式不正确"] }
}
```

- `type` — always `"about:blank"` for now (RFC 9457 default). We do not mint
  a documentation URI space; `code` is the machine key.
- `title` — the generic English reason phrase for `status`.
- `status` — mirrors the HTTP status code.
- `code` — **the frontend's only discriminator.** A stable snake_case slug
  from the registry in §3.4. Never localized, never renamed once shipped.
- `detail` — human-readable message, localized server-side from
  `Accept-Language` (§4.3). Presentation only; no client logic may match it.
- `errors` — present only for `validation_failed`: an ordered
  `{field: [localized messages]}` bag (first entry is the primary display,
  matching today's Laravel MessageBag consumption).

Retired with this model:

- Frontend exact-English-string matching (`PERMISSION_DENIED_MESSAGE`,
  `STEP_UP_REQUIRED_MESSAGE`, `isStepUpRequiredError` in
  `frontend/packages/api-client/src/client.ts`) — replaced by `code`.
- The backend response-rewrite localization middleware
  (`crates/api/src/localization.rs` body rewriting keyed on
  `Content-Language` + the message-literal catalog in `crates/api/src/i18n`)
  — retired **for internal routes only**. The middleware and catalog stay in
  service for the §2 external namespaces (still keyed on `Content-Language`,
  still defaulting to zh-CN), because their error bytes are frozen.
  Invariant that must hold from W1 through W14: problem+json bodies carry no
  `message` key, so the middleware never rewrites a modern response — it
  acts only on legacy `{message}` bodies. Internal-route localization moves
  to error construction time, keyed by `code`, driven by `Accept-Language`
  (§4.3). W14 deletes only catalog entries that no external route can emit.
- `ApiError`'s Laravel-compat constructors on internal routes
  (`legacy()`/`business()`/`validation_field()` bodies). External routes
  (§2) keep the legacy error type.

### 3.2 Status mapping

| Situation | Status | `code` |
| --- | --- | --- |
| Missing/expired/invalid session (was 403 「未登录或登陆已过期」) | **401** | `session_expired` |
| Authenticated but wrong role (`Permission denied`) | 403 | `permission_denied` |
| Privileged write without step-up (`Recent password verification is required`) | 403 | `step_up_required` |
| Deterministic business rejection (was `business()` 400) | 400 | per-registry |
| Field validation (was 422 `{message, errors}`) | 422 | `validation_failed` |
| Unknown route/resource | 404 | `endpoint_not_found` / `{resource}_not_found` |
| Concurrent-update / idempotency conflicts | 409 | per-registry |
| Rate limits | 429 | per-registry |
| Internal errors (was `Uh-oh, we've had some problems…`) | 500 | `internal_error` |
| Transient unavailability | 503 | `service_unavailable` |

Config accepted-but-not-yet-active is **202 Accepted** (§6.1), a success
outcome, not an error — the earlier-draft `config_activation_pending` error
code does not exist.

The frontend session-teardown hook keys on **401 + `session_expired`** only.
403 `permission_denied` / `step_up_required` must never tear down the
session (this preserves the current 403-keep-token behavior, but by code
instead of message text).

Every 401 response also carries `WWW-Authenticate: Bearer
error="invalid_token"` (bare `WWW-Authenticate: Bearer` when the request
carried no credentials at all), per RFC 9110 §15.5.2 and RFC 6750 §3. The
SPA keys on status + `code` and ignores this header; it exists for HTTP
conformance and is pinned in the golden wire tests.

Legacy `legacy()` 500s that are actually deterministic rejections (e.g.
checkout `gate is not found`, `Stripe payment binding is invalid`) are
re-classified to 4xx with registry codes during their family's wave; the
wave records the reclassification in §3.4.

### 3.3 Code slug rules

1. snake_case ASCII, `[a-z][a-z0-9_]*`, ≤ 40 chars.
2. Shape: `{domain_noun}_{condition}` (`plan_sold_out`, `coupon_expired`,
   `order_not_pending`) or a bare well-known condition
   (`session_expired`, `validation_failed`, `internal_error`).
3. One code per *distinguishable client behavior*, not per call site. Call
   sites that today emit the same message (or messages the client treats the
   same) share one code.
4. Codes are append-only. Never rename, renumber, or re-purpose a shipped
   code; deprecate by ceasing to emit it.
5. Every new code lands in §3.4 in the same commit series that first emits
   it, with its status and the legacy message(s) it replaces.
6. **Exactly one HTTP status per code** — a code's status never varies by
   endpoint (generic HTTP middleware must be able to key on it). Where
   legacy sites shared one message across genuinely different situations
   (path-lookup miss vs body-referenced business rejection), the registry
   splits them into two codes (e.g. `user_not_found` 404 vs
   `user_not_registered` 400).

### 3.4 Initial code registry

Historically derived from the legacy `ApiError::business/legacy/bad_request/
not_found/validation_field/unauthorized` call sites in `crates/api` and the
now-retired `crates/domain` (internal routes only). The current code/status
registry lives in `crates/problem-code/src/lib.rs`, its RFC 9457 transport DTO
lives in `crates/api-contract/src/problem.rs`, and
`crates/compat/src/problem.rs` is the HTTP response adapter. W14 deleted the
legacy internal constructors; the gates verify these sources match the tables
slug-for-slug and status-for-status, with every code reachable from a live
emitter. "Legacy anchor" is the message literal the code replaced; it is also
the key the parity error canonicalizer (§13.3) uses to map oracle-world errors
onto codes.

Transport / generic:

| Code | Status | Legacy anchor(s) |
| --- | --- | --- |
| `validation_failed` | 422 | every `validation_field` bag (`邮箱格式不正确`, `备注不能为空`, `匹配值不能为空`, `动作类型不能为空/参数有误`, pagination messages, …) |
| `invalid_parameter` | 400 | `Invalid parameter`, `参数有误`, `参数错误`, malformed bodies (`Invalid admin request body`) |
| `endpoint_not_found` | 404 | `Not Found`, `Admin endpoint does not exist`, `Staff endpoint does not exist` |
| `rate_limited` | 429 | generic 429s without a more specific code |
| `internal_error` | 500 | `Uh-oh, we've had some problems, we're working on it.` |
| `service_unavailable` | 503 | transient dependency failures |

Auth / session:

| Code | Status | Legacy anchor(s) |
| --- | --- | --- |
| `session_expired` | 401 | `未登录或登陆已过期` (was 403) |
| `permission_denied` | 403 | `Permission denied` |
| `step_up_required` | 403 | `Recent password verification is required` |
| `invalid_credentials` | 400 | `Incorrect email or password` |
| `account_suspended` | 400 | `Your account has been suspended` |
| `registration_closed` | 400 | `Registration has closed` |
| `register_ip_rate_limited` | 429 | register-limit-by-IP message (registration.rs:250 format!) — unconditionally 429 (was a 400 business rejection) |
| `password_attempts_rate_limited` | 400 | `There are too many password errors, please try again after N minutes.` (minutes stay in localized `detail`) |
| `mfa_code_required` | 401 | — (native addition, §6.10: privileged login needs a TOTP second phase) |
| `mfa_code_invalid` | 401 | — (native addition, §6.10: wrong/replayed TOTP code) |
| `mfa_already_enabled` | 400 | — (native addition, §6.10) |
| `mfa_setup_missing` | 400 | — (native addition, §6.10) |
| `mfa_not_enabled` | 400 | — (native addition, §6.10) |
| `mfa_enrollment_required` | 403 | — (native addition, §6.10: `admin_mfa_force` demands an enabled factor before any privileged route outside the caller's own `account/mfa` family) |
| `email_already_registered` | 400 | `Email already exists`, `This email is registered` |
| `email_not_registered` | 400 | `This email is not registered in the system` |
| `invalid_email_code` | 400 | `Incorrect email verification code` |
| `invalid_invite_code` | 400 | `Invalid invitation code` |
| `email_suffix_not_allowed` | 400 | `Email suffix is not in the Whitelist` |
| `gmail_alias_not_supported` | 400 | `Gmail alias is not supported` |
| `recaptcha_failed` | 400 | `Invalid code is incorrect` |
| `email_send_rate_limited` | 429 | sendEmailVerify per-address/IP limiter (verification.rs:48) |
| `invalid_token` | 400 | `Token error` (token2Login/quick-login exchange) |
| `old_password_incorrect` | 400 | `The old password is wrong` |
| `password_reset_failed` | 400 | `Reset failed`, `Reset failed, Please try again later` |
| `user_not_found` | 404 | `The user does not exist`, `该用户不存在` — path-identified lookups (`GET users/{id}`, staff view) |
| `user_not_registered` | 400 | same anchors on body-referenced email lookups (set-inviter, admin order assign, staff mail targets) |

Commerce (user):

| Code | Status | Legacy anchor(s) |
| --- | --- | --- |
| `plan_not_found` | 404 | `Subscription plan does not exist`, `该订阅(ID)不存在` — path-identified lookups (`GET /user/plans/{id}`, admin `plans/{id}`) |
| `plan_unavailable` | 400 | same anchors when the plan is body-referenced (order create/change with a missing plan) |
| `plan_sold_out` | 400 | `Current product is sold out` |
| `plan_period_unavailable` | 400 | `Wrong plan period`, `This payment period cannot be purchased, please choose another period` |
| `plan_change_disabled` | 400 | plan-change disabled rejections (lifecycle.rs) |
| `pending_order_exists` | 400 | unpaid/pending order guard (order.rs:489/928) |
| `order_not_found` | 404 | `订单不存在`, `Order does not exist or has been paid` — always 404: every modern order route carries `trade_no` in the path |
| `order_not_pending` | 400 | `只能对待支付的订单进行操作` |
| `payment_method_unavailable` | 400 | `Payment method is not available` |
| `payment_config_invalid` | 400 | `Payment config is invalid` |
| `payment_gateway_unsupported` | 400 | `gate is not found`, unknown checkout gateway result |
| `payment_amount_out_of_range` | 400 | `Payment amount is outside the supported range`, `Order amount is outside the supported range` |
| `handling_fee_out_of_range` | 400 | `Payment handling fee is outside the supported range` |
| `stripe_binding_invalid` | 400 | `Stripe payment binding is invalid`, `payment_changed` binding-state rejections |
| `insufficient_balance` | 400 | `Insufficient balance` |
| `coupon_invalid` | 400 | `Invalid coupon`, `Invalid coupon discount value` |
| `coupon_unavailable` | 400 | `This coupon is no longer available` |
| `coupon_not_started` | 400 | `This coupon has not yet started` |
| `coupon_expired` | 400 | `This coupon has expired` |
| `coupon_exhausted` | 400 | `Coupon failed` (use / per-user limits) |
| `coupon_not_applicable` | 400 | plan/period restriction messages (lifecycle.rs:608/615/632) |
| `gift_card_invalid` | 400 | `礼品卡不存在` + redeem rejections on `POST /user/gift-card-redemptions` (body-referenced code); the admin path-identified miss is `gift_card_not_found` 404 |
| `subscription_value_out_of_range` | 400 | `Subscription surplus/traffic/expiry … exceeds the supported range` family |
| `renewal_not_allowed` | 400 | `Renewal is not allowed`, `You do not allow to renew the subscription` |
| `reset_period_invalid` | 400 | `Invalid reset period` |

Profile / invite / ticket / content (user):

| Code | Status | Legacy anchor(s) |
| --- | --- | --- |
| `transfer_amount_invalid` | 422 | `The transfer amount parameter is wrong` |
| `insufficient_commission_balance` | 400 | `Insufficient commission balance` |
| `balance_out_of_range` | 400 | `Balance exceeds the supported range` |
| `invite_code_limit_reached` | 400 | `The maximum number of creations has been reached` |
| `telegram_not_configured` | 400 | `Telegram bot is not configured`, `telegram bot token is null` |
| `telegram_unbind_failed` | 400 | `Unbind telegram failed` |
| `ticket_not_found` | 404 | `Ticket does not exist`, `工单不存在` |
| `ticket_invalid_state` | 400 | `未知的工单状态`, closed-ticket reply rejections |
| `unresolved_ticket_exists` | 400 | `There are other unresolved tickets`, `用户存在其他未解决工单，无法重新打开该工单` |
| `ticket_requires_plan` | 400 | `请先购买套餐`, `当前套餐不允许发起工单` |
| `withdraw_method_unsupported` | 400 | `Unsupported withdrawal method`; `Unsupported withdrawal` (W8 reclassified the deterministic legacy-500 `user.ticket.withdraw.not_support_withdraw` close-enable gate here) |
| `withdraw_below_minimum` | 400 | minimum-withdrawal format! (ticket.rs:245; limit stays in `detail`) |
| `article_not_found` | 404 | `Article does not exist` — path-identified (`GET /user/knowledge/{id}`) |
| `notice_not_found` | 404 | `Notice not found`, `公告不存在` |
| `knowledge_not_found` | 404 | `知识不存在` |

Admin:

| Code | Status | Legacy anchor(s) |
| --- | --- | --- |
| `config_revision_conflict` | 409 | `配置已被其他请求更新，请刷新后重试` |
| `config_validation_failed` | 400 | `配置校验失败: …`, `配置安全校验失败: …` (detail keeps the specific reason); also emitted by the new reserved-segment admin-path rule (§10.2) |
| `payment_method_not_found` | 404 | `支付方式不存在` |
| `payment_method_in_use` | 400 | payment delete/update guards (commerce/payments.rs) |
| `reconciliation_not_found` | 404 | `付款核对记录不存在` |
| `reconciliation_already_processed` | 409 | `付款核对记录已处理` |
| `order_assign_conflict` | 400 | `该用户还有待支付的订单，无法分配` |
| `order_update_conflict` | 409 | `订单状态正在被其他请求修改，请重试` |
| `order_update_failed` | 400 | `更新失败` |
| `plan_in_use` | 400 | `该订阅下存在订单无法删除` / `…存在用户无法删除` / `该订阅仍被礼品卡使用，无法删除` (which dependency stays in `detail`) |
| `plan_update_conflict` | 409 | a concurrent plan move won before a `force_update`; retrying preserves the group → user → plan lock order |
| `plan_force_update_limit_exceeded` | 400 | `该订阅用户过多，单次最多强制更新 10000 个用户` (native `force_update` cap) |
| `coupon_not_found` | 404 | `优惠券不存在` |
| `gift_card_not_found` | 404 | `礼品卡不存在` (admin `DELETE gift-cards/{id}`, already `not_found` in the anchor) |
| `server_not_found` | 404 | `该服务器不存在`, `路由不存在`(→`route_not_found`), `该服务器组不存在`(→`server_group_not_found`) |
| `route_not_found` | 404 | `路由不存在` |
| `server_group_not_found` | 404 | `该服务器组不存在` |
| `server_group_in_use` | 400 | `该组已被节点所使用，无法删除` / `该组已被订阅所使用，无法删除` / `该组已被用户所使用，无法删除` (which dependency stays in `detail`; W13 `DELETE server-groups/{id}`) |
| `invalid_server_type` | 400 | `Invalid server type` |
| `app_url_not_configured` | 400 | `请在站点配置中配置站点地址` |
| `mail_sender_not_configured` | 400 | `Email sender is not configured`, `Email host is not configured` |
| `mail_invalid` | 400 | `Email sender/recipient/content is invalid`, `Invalid email sender`, `Invalid recipient email` |
| `mail_send_failed` | 502 | `Send mail failed/timed out`, `Email send failed/timed out`, `Build mail failed` |
| `mail_idempotency_conflict` | 409 | `Mail idempotency key was reused with a different payload` |
| `mail_idempotency_key_invalid` | 400 | `Mail idempotency key is invalid/too long` |
| `telegram_request_failed` | 502 | `Telegram request failed…`, `Telegram bot response is invalid` |
| `telegram_token_invalid` | 400 | `Telegram token is invalid`, `Telegram bot token cannot be empty` |
| `telegram_webhook_failed` | 502 | `Telegram webhook failed` |

Frontend display: the api-client surfaces `{status, code, detail, errors}`;
apps map `code → i18n` for styled copy and fall back to `detail` verbatim for
unknown codes. No client behavior may branch on `detail`.

---

## 4. Transport conventions

### 4.1 JSON bodies

- Every internal non-GET request sends `Content-Type: application/json` with
  a JSON object body. The form-urlencoded encoder, bracket-array shapes
  (`limit_plan_ids[0]`, `filter[0][key]`), and the `serializeForm` /
  `nullFormValue` machinery in `client.ts` are retired for internal routes.
- Booleans are JSON `true`/`false` — both directions. All `0|1` flags in
  requests and responses become booleans: `banned`, `show`, `renew`,
  `enable`, `is_admin`, `is_staff`, `is_login`, `auto_renewal`,
  `remind_expire`, `remind_traffic`, `is_online`, `allow_insecure`,
  `insecure`, `disable_sni`, `zero_rtt_handshake`, `current`,
  `is_email_verify`, `is_invite_force`, `is_recaptcha`, `is_telegram`,
  `withdraw_close`, `commission_distribution_enable`, every
  `configFlagSchema` field, `isforget` (renamed `is_forget`), etc.
  True enums stay numeric: order `status`/`type`/`commission_status`, ticket
  `level`, coupon/giftcard `type`, `reset_traffic_method`,
  `commission_type`, `available_status`.
- Arrays are JSON arrays: `group_id`, `route_id`, `tags`, `match`,
  `limit_plan_ids`, `limit_period`, `plan_ids`/`knowledge_ids`/`ids` (all
  become `ids`), `filter`, `deposit_bounus`, `email_whitelist_suffix`,
  `withdraw_methods`, `used_user_ids`. Comma-joined strings, `0`-sentinel
  unions (`email_whitelist_suffix: string[] | 0`), and stringified-JSON
  array params die; disabled lists are `[]`.
- Numbers: the string-vs-number splits unify to JSON numbers:
  `handling_fee_percent` (user + admin), `server_rate`, `rate`, `port`,
  `server_port`, `email_port`, `server_pull_interval`,
  `server_push_interval`, `server_node_report_min_traffic`,
  `server_device_online_min_traffic`, `commission_distribution_l1/2/3`,
  `try_out_hour`. Exception (recorded rationale): admin config decimal
  fields backed by exact PostgreSQL NUMERIC round-trips
  (`commission_withdraw_limit`) keep their decimal-string form so the admin
  form preserves lexical value; this is a deliberate keep, not an oversight.
- Money stays integer minor units (cents) — already standard, with one
  legacy exception: the admin stat series (`stat/getOrder`,
  `stat/getStatRecord`) ship yuan **floats** today (`paid_total as f64 /
  100.0`); they convert to integer cents in their wave (§6.8). The
  api-client keeps `decimalToCents`/`decimalToScaledInteger` at its boundary
  and the admin cents conversions (e.g. coupon `type===1 → value*100`)
  continue unchanged in value, now as JSON numbers.

### 4.2 Authentication

- `Authorization: Bearer <auth_data>` on every authenticated internal
  request. The bare legacy header value (no scheme) is no longer accepted on
  internal routes. The `localStorage` `authorization` key and its raw stored
  value are unchanged (§2); the client prepends `Bearer ` on the wire.
- **Flip mechanism** (the auth extractor `select_auth_data` in
  `crates/api/src/auth.rs` is shared by every internal route, so a
  per-family flip is impossible and an accept-both window would be a shipped
  dual-dialect branch): the scheme is a cross-cutting family flipped
  atomically in W2 — the api-client prepends `Bearer ` on **every** internal
  request (including not-yet-migrated legacy-dialect families, whose body
  dialects are unaffected) and `select_auth_data` simultaneously requires
  and strips the `Bearer ` prefix on internal routes, in the same commit
  series. §2 external routes are unaffected: they authenticate via the
  `token` query parameter or `x-v2board-server-token`, never
  `Authorization`.
- The step-up token keeps riding `x-v2board-step-up` (unchanged mechanism,
  now signalled by `code: "step_up_required"` instead of message text).
- CORS: `Access-Control-Allow-Methods` extends to
  `GET, POST, PATCH, PUT, DELETE, OPTIONS, HEAD` (routes.rs `cors_layer`).

### 4.3 Locale

- `Accept-Language` replaces `Content-Language` as the request locale
  signal, standard HTTP list syntax (`ja-JP,ja;q=0.9,en;q=0.5`). The backend
  resolves against the enabled locale registry and localizes problem
  `detail`/`errors` text; default remains `zh-CN`.
- The enabled locale registry is the existing anchor `ENABLED_LOCALES`
  (`crates/api/src/frontend.rs`, injected as the runtime-config `i18n` key),
  which `make deploy-contract-audit` keeps set-equal to
  `frontend/packages/i18n/src/locale-registry.ts`. The `Accept-Language`
  resolver and the `v2board_locale` value vocabulary (§11) both derive from
  it, so the three lists cannot drift.
- The api-client sends `Accept-Language: <active-locale>`.
- Transition footnote (W1, 2026-07-17): the spec is silent on the client
  transition window, so from W1 the api-client sends **both**
  `Accept-Language` and the legacy `Content-Language` (same value). Legacy
  internal routes localize `message` bodies through the Content-Language
  response-rewrite middleware until their family migrates; the transitional
  `Content-Language` line in `packages/api-client/src/client.ts` is deleted
  together with that middleware at the end of the wave series.

### 4.4 Null semantics (clear vs retain)

The legacy tri-state form convention (present-but-empty string = clear,
absent = retain, `nullFormValue: 'empty'`) is replaced by explicit JSON
semantics on **update-class endpoints** (`PATCH`):

| JSON state | Meaning |
| --- | --- |
| field absent | retain current value |
| field `null` | clear (set NULL / disable) |
| field value | set |

Fields that are not clearable reject `null` explicitly in their endpoint
contract. In particular, admin config `secure_path` always requires a
non-empty explicit replacement; absent retains it, while `null`/empty is a
422 validation failure.

- Rust: `#[serde(default, with = "double_option")] Option<Option<T>>` per
  clearable field (serde double-Option); `deny_unknown_fields` on every
  request struct so typos become 422s instead of silent retains.
- Zod (api-client request contracts, new in this dialect): clearable fields
  are `z.union([T, z.null()]).optional()`; the client never converts `null`
  to `''` or drops it.
- **Create-class endpoints** (`POST` collections): absent means the
  documented per-field default; `null` means explicit NULL where the column
  is nullable. No field means "retain" on create.
- **Full-replace endpoints**: none. Legacy upsert `save` actions split into
  `POST` (create) + `PATCH` (partial update) per resource (§6.3), so the
  double-Option table above covers every mutation.
- Per-endpoint-class notes appear in the route tables (`notes` column) where
  a legacy endpoint had load-bearing present-gating. The current transport
  shapes live in `crates/api-contract/src/admin_servers.rs`, are translated by
  `crates/api/src/server_management_adapters.rs`, and reach the application
  command without reviving the retired `param_present` convention.

### 4.5 Timestamps — RFC 3339 UTC

All epoch-second integers in internal request/response bodies become RFC
3339 UTC strings (`"2026-07-17T08:30:00Z"`), or `null`. The current inventory
is enforced by the Rust DTOs in `crates/api-contract` and their generated
TypeScript/Zod projections (the former hand-written `contracts.ts` was
deleted):

- `created_at`, `updated_at` (every entity)
- `expired_at` (user, subscribe info, admin user; `null` = never expires)
- `last_login_at`, `login_at` (sessions)
- `paid_at` (orders)
- `started_at`, `ended_at` (coupons, gift cards; nullable on gift cards)
- `record_at` (traffic logs — remains the period-start marker, now as an
  RFC 3339 instant)
- `last_check_at` (servers/nodes)

Request-direction fields converted the same way: admin user update
`expired_at`, user generate `expired_at`, coupon/giftcard generate
`started_at`/`ended_at`.

Storage stays epoch seconds; conversion happens at the API serde boundary.
Durations (`expires_in` seconds, plan durations) stay integers.

### 4.6 Declared already-at-standard (kept, with rationale)

- **snake_case JSON field names** — already consistent; matches Rust serde
  defaults and PostgreSQL columns.
- **Integer minor-unit money** — exact, matches DB storage and Stripe.
- **TanStack `refetchInterval` polling** — 3 s order-status and 5 s ticket
  cadences stay; server push is out of scope.
- **Runtime-config injection** — the `__V2BOARD_RUNTIME_CONFIG__` script-data
  token mechanism (frontend.rs + index.html) stays; it gains new keys in
  §10.3.
- **`/node` page fetch ordering** — the subscribe-first *serial* fetch is
  replaced by parallel fetch of subscription + servers with
  subscription-gated *rendering*; the preserved outcome is "no node list
  shown without knowing subscription state", which is the Tier-1 behavior.

---

## 5. Route map — passport, user, guest

Admin/staff follow in §6. Old paths are relative to `/api/v1`; new paths
keep the `/api/v1` prefix. Every new mutation takes `json` bodies; every new
response is `problem+json` on error.

### 5.1 Public (was guest comm)

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `/guest/comm/config` | GET `/public/config` | none | bare | Flags→bool; `email_whitelist_suffix` always an array. |
| POST `/passport/comm/pv` | POST `/public/invite-views` | json `{invite_code}` | empty | Unauthenticated telemetry; was `{data:true}`. |

### 5.2 Auth (was passport)

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| POST `/passport/auth/register` | POST `/auth/register` | json `{email, password, invite_code?, email_code?, recaptcha_data?}` | bare `{is_admin: bool, auth_data}` (201) | `is_admin` becomes bool; 201 — account creation. |
| POST `/passport/auth/login` | POST `/auth/login` | json `{email, password, totp_code?}` | bare (same as register) | `totp_code` is a native addition (§6.10): required as a second phase only for privileged accounts with an enabled TOTP factor — absent → 401 `mfa_code_required`, wrong/replayed → 401 `mfa_code_invalid`. Neither 401 tears a session down; non-privileged logins ignore the field. |
| GET `/passport/auth/token2Login?token=` | GET `/auth/quick-login?token=` | query | redirect | 302 to `{app_url}/login?verify=…&redirect=…` (path-style, §10.4). Browser-facing. |
| GET `/passport/auth/token2Login?verify=` | POST `/auth/token-login` | json `{verify}` | bare auth data (or 400 `invalid_token`, matching the legacy `Token error` business status) | GET-with-side-effect becomes POST; the SPA exchange call. The legacy "neither param → empty 200" branch dies (422). |
| POST `/passport/auth/forget` | POST `/auth/password-reset` | json `{email, email_code, password}` | empty | |
| POST `/passport/auth/stepUp` | POST `/auth/step-up` | json `{password}` | bare `{step_up_token, expires_in}` | |
| POST `/passport/auth/getQuickLoginUrl` | POST `/auth/quick-login-url` | json `{redirect?}` | bare `{url}` | Consolidates with the duplicate `/user/getQuickLoginUrl` (both require the same user auth; one endpoint remains). |
| POST `/passport/comm/sendEmailVerify` | POST `/auth/email-codes` | json `{email, is_forget?: bool, recaptcha_data?}` | empty | `isforget: 0/1` → `is_forget: bool`. |
| GET `/user/checkLogin` | GET `/auth/session` | none | bare `{is_login: bool, is_admin?: bool, is_staff?: bool, admin_permissions?: string[]}` | Session probe moves out of `/user`. The staff pair is a §6.12 native addition: present together exactly for staff (non-admin) sessions — `admin_permissions` may be `[]` — so the admin SPA can gate its navigation without a second round trip. Admin sessions carry only `is_admin: true`. |
| POST `/user/logout` | DELETE `/auth/session` | none | empty | Dead/absent bearer stays a successful no-op. |

### 5.3 User account & profile

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `/user/info` | GET `/user/profile` | none | bare | Flags→bool, timestamps→RFC 3339. |
| POST `/user/update` | PATCH `/user/profile` | json `{auto_renewal?, remind_expire?, remind_traffic?}` (bool) | empty | §4.4 semantics. |
| POST `/user/changePassword` | PUT `/user/password` | json `{old_password, new_password}` | empty | Success still invalidates sessions → client redirects to login (Tier-1 outcome). |
| GET `/user/getStat` | GET `/user/stats` | none | bare `{pending_order_count, pending_ticket_count, invited_user_count}` | Tuple `[a,b,c]` → named object. |
| GET `/user/getActiveSession` | GET `/user/sessions` | none | array of `{session_id, ip, ua, login_at, current}` | Keyed map → array; the map key / `auth_data` digest becomes `session_id`. |
| POST `/user/removeActiveSession` | DELETE `/user/sessions/{session_id}` | none | empty | |
| POST `/user/transfer` | POST `/user/commission-transfers` | json `{transfer_amount}` (cents) | empty | |
| POST `/user/redeemgiftcard` | POST `/user/gift-card-redemptions` | json `{giftcard}` | bare `{type, value}` | Envelope extras → named object. |
| GET `/user/unbindTelegram` | DELETE `/user/telegram-binding` | none | empty | GET-with-side-effect → DELETE. |
| GET `/user/telegram/getBotInfo` | GET `/user/telegram-bot` | none | bare `{username}` | |
| GET `/user/comm/config` | GET `/user/config` | none | bare | `withdraw_methods` always array; distribution rates → numbers. |

### 5.4 Subscription & service usage

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `/user/getSubscribe` | GET `/user/subscription` | none | bare | `subscribe_url`/token scheme inside is frozen (§2); flags→bool. |
| POST `/user/newPeriod` | POST `/user/subscription/new-period` | none (json `{}` allowed) | empty | True non-CRUD action verb. |
| GET `/user/resetSecurity` | POST `/user/subscription/reset-token` | none | bare `{subscribe_url}` | GET-with-side-effect → POST; bare string → named object. Token rotation outcome is Tier-1. |
| GET `/user/server/fetch` | GET `/user/servers` | none | array | `is_online`→bool, `rate`→number, `port`→number. |
| GET `/user/stat/getTrafficLog` | GET `/user/traffic-logs` | none | array | `server_rate`→number, `record_at`→RFC 3339. |

### 5.5 Commerce

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `/user/plan/fetch` | GET `/user/plans` | none | array | |
| GET `/user/plan/fetch?id=` | GET `/user/plans/{id}` | none | bare | Sold-out (`capacity_limit`) representation unchanged. |
| POST `/user/order/save` | POST `/user/orders` | json union: `{kind: "plan", plan_id, period, coupon_code?}` \| `{kind: "deposit", deposit_amount}` | bare `{trade_no}` (201) | Bare string → named object. Discriminated request body replaces the legacy deposit sentinel (`plan_id: 0` + magic `period: "deposit"` from the wallet card, cents conversion in `endpoints/user.ts`) — serde internally-tagged enum + zod discriminated union, both arms `deny_unknown_fields`; `deposit_amount` stays integer cents. Empty-coupon rule (omit the field, never `""`) is Tier-1 and stays. W4 owns both arms, including the profile wallet-card payload construction. |
| GET `/user/order/fetch` | GET `/user/orders` | query `?status=` | array | |
| GET `/user/order/detail?trade_no=` | GET `/user/orders/{trade_no}` | none | bare | |
| GET `/user/order/check?trade_no=` | GET `/user/orders/{trade_no}/status` | none | bare `{status}` | Bare number → named object. 3 s polling cadence stays. |
| POST `/user/order/cancel` | POST `/user/orders/{trade_no}/cancel` | none | empty | `{trade_no}` body → path segment. |
| POST `/user/order/checkout` | POST `/user/orders/{trade_no}/checkout` | json `{method_id}` | union (§9.3) | `method` → `method_id`. |
| POST `/user/order/stripe/intent` | POST `/user/orders/{trade_no}/stripe-intent` | json `{method_id}` | bare `{public_key, client_secret, amount, currency}` | Stripe external payloads frozen (§2). |
| GET `/user/order/getPaymentMethod` | GET `/user/payment-methods` | none | array | `handling_fee_percent` → number (was decimal string). |
| POST `/user/coupon/check` | POST `/user/coupons/check` | json `{code, plan_id}` | bare coupon | Read-shaped action kept as POST (carries a payload). |

### 5.6 Invite & commission

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `/user/invite/save` | POST `/user/invite-codes` | none | empty | GET-with-side-effect → POST; boolean body → 204/problem. Deliberately kept on the 204+refetch flow (§1): invite codes are never individually addressed afterwards. |
| GET `/user/invite/fetch` | GET `/user/invite` | none | bare `{codes: [...], stat: {...}}` | 5-tuple → `{registered_count, valid_commission, pending_commission, commission_rate, available_commission}` (commissions in cents, rate percent). Frontend `amount/100` display math keeps reading cents. |
| GET `/user/invite/details` | GET `/user/commissions` | query `?page=&per_page=` | page | `current/page_size` → `page/per_page`; envelope → `{items,total}`. |

### 5.7 Tickets

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `/user/ticket/fetch` | GET `/user/tickets` | none | array | |
| GET `/user/ticket/fetch?id=` | GET `/user/tickets/{id}` | none | bare | Includes `message[]` thread; 5 s reply polling stays. |
| POST `/user/ticket/save` | POST `/user/tickets` | json `{subject, level, message}` | bare `{id}` (201) | Created id lets the UI open the thread without a list refetch. |
| POST `/user/ticket/reply` | POST `/user/tickets/{id}/replies` | json `{message}` | empty | `id` moves to path. |
| POST `/user/ticket/close` | POST `/user/tickets/{id}/close` | none | empty | |
| POST `/user/ticket/withdraw` | POST `/user/withdrawal-tickets` | json `{withdraw_method, withdraw_account}` | bare `{id}` (201) | Creates the withdrawal ticket resource. |

### 5.8 Knowledge & notices

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `/user/knowledge/fetch` | GET `/user/knowledge` | query `?language=&keyword=` | bare record `{category: [...]}` | Category-grouped record kept (documented shape). |
| GET `/user/knowledge/fetch?id=` | GET `/user/knowledge/{id}` | query `?language=` | bare | Body stays non-idempotent (re-substituted per request) — Tier-1 refetch behavior. |
| GET `/user/knowledge/getCategory` | GET `/user/knowledge-categories` | none | array | |
| GET `/user/notice/fetch` (query `current`/`pageSize`, default page size 5; page envelope) | GET `/user/notices` | query `?page=&per_page=` | page | Paginated in the legacy anchor too (the draft's no-params/bare-array mapping was wrong). `per_page` default pinned at **5**, matching legacy — the `弹窗` auto-popup tag scan keeps operating over exactly the first page the client fetches (Tier-1 universe unchanged). The legacy `?id=` single-notice branch is **dropped** (no frontend consumer exists; recorded decision). |

---

## 6. Route map — admin and staff

All admin routes stay under the dynamic prefix `/api/v1/{secure_path}/…`
(the mechanism is kept), with resource-style paths beneath. Staff routes
stay under `/api/v1/staff/…`.

**Dynamic prefix × method routing.** The admin prefix is resolved per
request from the live config snapshot (a `secure_path` config save changes
the live admin route immediately — `dynamic_fallback` +
`route_paths.rs`, pinned by
`current_admin_route_match_uses_latest_config_path`). That behavior is
Tier-1 and survives the method split: the admin resources bind as a nested,
method-aware Axum router that `dynamic_fallback` re-dispatches into for
**all** methods (GET/POST/PATCH/PUT/DELETE) behind a per-request
live-prefix check — extending today's GET/POST-only re-dispatch, not a
boot-time literal route. Admin PATCH/DELETE must keep working across a
`secure_path` change without a process restart. §10.2 rule 4's 404 applies
only after this admin dispatch has declined the path.

**Step-up policy.** Every admin and staff **mutation** (POST, PATCH, PUT,
DELETE) under the `{secure_path}`/`staff` prefixes requires a valid
`x-v2board-step-up` token (403 `step_up_required` otherwise), exactly as
every legacy POST does today (`require_privileged_step_up` on the whole
POST dispatch path). This is enforced **structurally** — shared middleware
on the admin/staff sub-routers, not per-handler — so a new route cannot
silently ship ungated. Sensitive reads additionally require step-up on GET:
`nodes` (was `server/manage/getNodes`) and `payment-reconciliations` (was
`order/reconciliation/fetch`). Two legacy POSTs become GETs and thereby
deliberately leave the blanket mutation gate (recorded owner-level
decisions, not side effects): `GET orders/{trade_no}` (was POST
`order/detail`; exposes nothing beyond the ungated list) and
`GET payment-providers/{code}/form` (was POST `payment/getPaymentForm`; the
response is server-redacted via `redact_payment_config`). An
interaction-parity scenario must cover a PATCH/DELETE step-up rejection.

**Admin-namespace RBAC (§6.12).** The shared admin guard authorizes every
request through the fixed permission registry: `is_admin` accounts have
full access; staff accounts enter the admin namespace with per-family
grants (GET/HEAD needs `{family}:read`, mutations need `{family}:write`,
`write` implies `read`; the caller's own `account/mfa` family needs no
grant). Everyone else — and every ungranted or registry-unmapped path — is
the 403 `permission_denied`, never a session teardown.

Legacy `save`-style upserts split: `POST` creates, `PATCH …/{id}` updates
(with §4.4 double-Option semantics). Legacy toggle actions (`…/show`,
`plan/update`, `server/{type}/update`) merge into the same `PATCH` with a
boolean field.

### 6.1 Config & system

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `config/fetch?key=` | GET `config` `?group=` | query | bare | Grouped object stays; flags→bool per §4.1. The top level always carries the positive active operator `revision`; `?group=site` therefore returns `{revision, site: {...}}`, not a revision-less fragment. `frontend_custom_html` field removed (§12). |
| POST `config/save` | PATCH `config` | json `{expected_revision, ...changes}` | empty / bare (202) | `expected_revision` is required, positive, and copied from the GET projection on which the draft is based. The server validates it against the authenticated PostgreSQL active revision; changed writes repeat that comparison atomically inside the commit. Missing/wrong-typed tokens are malformed requests, non-positive tokens fail validation, and stale tokens return 409 `config_revision_conflict`. The server merges omitted fields against that authenticated authoritative revision rather than a process-local snapshot. `'[]'`-string array hack dies; arrays are arrays; §4.4 null-clear. When the write persists but the API cannot yet activate the new config (legacy anchor: `配置已提交…服务将自动重试` — the service auto-retries), the response is **202 Accepted** with `{"activation": "pending", "revision": n}`, where `n` is the revision actually committed by PostgreSQL, not the previously active or requested value. This is not an error: the write is durable, so retrying the PATCH would 409; the admin UI must refetch, never resubmit. Full activation and unchanged writes remain empty 204 responses. |
| GET `config/getEmailTemplate` | GET `email-templates` | none | array | |
| POST `config/setTelegramWebhook` | POST `telegram-webhook` | json `{telegram_bot_token?}` | empty | |
| POST `config/testSendMail` | POST `test-mail` | none | bare `{sent: bool, log}` | Envelope `{data:true, log}` → named object. |
| GET `system/getSystemStatus` | GET `system/status` | none | bare | |
| GET `system/getQueueStats` | GET `system/queue-stats` | none | bare | |
| GET `system/getQueueWorkload` | GET `system/queue-workload` | none | array | |
| GET `system/getQueueMasters` | GET `system/queue-masters` | none | array | |
| GET `system/getSystemLog` | GET `system/logs` | query + filter DSL (§7) | page | Log `level` filter only. |

### 6.2 Plans, payments

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `plan/fetch` | GET `plans` | none | array | Prices stay signed integer cents; `null` still means that period is unavailable. |
| POST `plan/save` | POST `plans` / PATCH `plans/{id}` | json | bare `{id}` (201) / empty | Upsert split; signed 32-bit price cents remain accepted; `force_update` stays a body flag on PATCH. |
| POST `plan/update` | PATCH `plans/{id}` | json `{show?: bool, renew?: bool}` | empty | Toggle merged into PATCH. |
| POST `plan/drop` | DELETE `plans/{id}` | none | empty | |
| POST `plan/sort` | POST `plans/sort` | json `{ids: [...]}` | empty | `plan_ids` → `ids`; a non-empty body is the unique, complete current plan-id permutation and is applied atomically, otherwise `plan_update_conflict`. |
| GET `payment/fetch` | GET `payments` | none | array | `handling_fee_percent` → number (unifies with user side). |
| GET `payment/getPaymentMethods` | GET `payment-providers` | none | array | Provider codes. |
| POST `payment/getPaymentForm` | GET `payment-providers/{code}/form` `?payment_id=` | query | bare | Read moves to GET. |
| POST `payment/save` | POST `payments` / PATCH `payments/{id}` | json | bare `{id}` (201) / empty | §4.4 replaces the present-but-empty=clear convention documented in admin.ts `serializePaymentForSave`. |
| POST `payment/show` | PATCH `payments/{id}` | json `{enable: bool}` | empty | |
| POST `payment/drop` | DELETE `payments/{id}` | none | empty | |
| POST `payment/sort` | POST `payments/sort` | json `{ids}` | empty | |

### 6.3 Content: notices, knowledge, coupons, gift cards

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `notice/fetch` | GET `notices` | none | array | Legacy returns **all** rows unpaginated (`admin/content.rs notice_fetch`); kept — no invented pagination. |
| POST `notice/save` / `notice/update` | POST `notices` / PATCH `notices/{id}` | json | bare `{id}` (201) / empty | Upsert + legacy `update` merge into PATCH. |
| POST `notice/show` | PATCH `notices/{id}` | json `{show: bool}` | empty | |
| POST `notice/drop` | DELETE `notices/{id}` | none | empty | |
| GET `knowledge/fetch` (+`?id=`) | GET `knowledge`, GET `knowledge/{id}` | query | array / bare | |
| GET `knowledge/getCategory` | GET `knowledge-categories` | none | array | |
| POST `knowledge/save` | POST `knowledge` / PATCH `knowledge/{id}` | json | bare `{id}` (201) / empty | |
| POST `knowledge/show` | PATCH `knowledge/{id}` | json `{show: bool}` | empty | |
| POST `knowledge/drop` | DELETE `knowledge/{id}` | none | empty | |
| POST `knowledge/sort` | POST `knowledge/sort` | json `{ids}` | empty | |
| GET `coupon/fetch` | GET `coupons` | query (pagination + §7.2 sort only) | page | **No filter DSL** — the legacy list has no filter support (`coupon_fetch`: pagination + sort clause only); none is invented. |
| POST `coupon/generate` | POST `coupons` | json | csv/json | `limit_plan_ids` real array; `started_at/ended_at` RFC 3339; cents rule (`type===1 → value*100`) unchanged; multi-generate returns CSV. |
| POST `coupon/show` | PATCH `coupons/{id}` | json `{show: bool}` | empty | |
| POST `coupon/drop` | DELETE `coupons/{id}` | none | empty | |
| GET `giftcard/fetch` | GET `gift-cards` | query (pagination + §7.2 sort only) | page | **No filter DSL** — same as coupons. |
| POST `giftcard/generate` | POST `gift-cards` | json | csv/json | Same conventions as coupons. |
| POST `giftcard/drop` | DELETE `gift-cards/{id}` | none | empty | |

### 6.4 Orders & reconciliation

Admin order actions standardize on `trade_no` as the resource identifier
(legacy mixed numeric `id` for detail with `trade_no` for mutations; the
backend detail lookup switches to `trade_no`).

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `order/fetch` | GET `orders` | query + DSL, `?is_commission=` → `?commission_only=bool` | page | |
| POST `order/detail` | GET `orders/{trade_no}` | none | bare | Read moves to GET; id → trade_no. Leaves the blanket POST step-up gate (recorded decision, §6 preamble). |
| POST `order/update` (status/commission arm) | PATCH `orders/{trade_no}` | json `{status?, commission_status?}` | empty | **Exactly one** of the two fields must be present; both or neither → 422 `validation_failed` (the legacy client only ever sends one). |
| POST `order/update` (with `reconciliation_id`) | POST `payment-reconciliations/{id}/resolve` | json `{resolution}` | empty | Demultiplexed: the legacy arm rode `order/update` behind a `reconciliation_id` param. 404 `reconciliation_not_found`, 409 `reconciliation_already_processed`. |
| POST `order/paid` | POST `orders/{trade_no}/mark-paid` | none | empty | |
| POST `order/cancel` | POST `orders/{trade_no}/cancel` | none | empty | |
| POST `order/assign` | POST `orders` | json `{email, plan_id, period, total_amount}` | bare `{trade_no}` (201) | Creates an order for a user. |
| GET `order/reconciliation/fetch` | GET `payment-reconciliations` | query `?resolved=&payment_id=&reason=&trade_no=&callback_no=` + pagination | page | Step-up-gated read (unchanged policy). Keeps its **dedicated named scalar params** (already clean; `trade_no`/`callback_no` are hashed server-side before matching, which the DSL cannot express) — not the §7 DSL. |

### 6.5 Tickets (admin)

The operation registry mounts the ticket family under the dynamic
`{secure_path}` prefix (`crates/api/src/admin.rs` operation ids backed by
`admin/support.rs`), consumed by the admin app's ticket pages — distinct from
the §6.9 staff mirror. Both inbound adapters call the shared
`application/src/ticket.rs` use cases but retain separate prefix,
authorization, and filter policies. The draft omitted this family entirely;
it migrated in W14 alongside the staff mirror.

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `ticket/fetch` | GET `tickets` | query `?status=&reply_status=&email=` + pagination | page | `reply_status` is a repeatable query key (real array; the legacy JSON-stringified array param dies). Email scoping keeps the legacy outcome: present + known user → scope to that user; present-but-unknown or absent → no scope (the Laravel `if ($user)` guard). Ordered by `updated_at`, unchanged. `per_page` default 10 (pinned by W14 per §15). |
| GET `ticket/fetch?id=` | GET `tickets/{id}` | none | bare | Includes the `message[]` thread with `is_me` semantics unchanged. |
| POST `ticket/reply` | POST `tickets/{id}/replies` | json `{message}` | empty | `id` moves to path. |
| POST `ticket/close` | POST `tickets/{id}/close` | none | empty | |

### 6.6 Users

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `user/fetch` | GET `users` | query + DSL (§7) | page | |
| GET `user/getUserInfoById?id=` | GET `users/{id}` | none | bare | |
| POST `user/update` | PATCH `users/{id}` | json | empty | `id` moves to path; §4.4 for nullable fields (`plan_id`, `expired_at`, `device_limit`, …); scaled cents/bytes stay integers. `admin_permissions?: string[]` (§6.12) is a full-replacement grant array validated against the fixed registry (422 on unregistered entries; `[]` revokes all; absent retains — the column is NOT NULL, so there is no null-clear arm). |
| POST `user/setInviteUser` | POST `users/{id}/set-inviter` | json `{invite_user_email}` | empty | Named non-CRUD action. |
| POST `user/generate` | POST `users` | json | csv/json | Bulk generate; `expired_at` RFC 3339. |
| POST `user/dumpCSV` | POST `users/export` | json `{filter?}` (DSL) | csv/json | Export stays POST (filter payload). |
| POST `user/sendMail` | POST `users/mail` | json `{subject, content, filter?}` | empty | `Idempotency-Key` header contract unchanged. |
| POST `user/ban` | POST `users/ban` | json `{filter?}` | empty | Bulk flag. |
| POST `user/resetSecret` | POST `users/{id}/reset-secret` | none | empty | |
| POST `user/delUser` | DELETE `users/{id}` | none | empty | |
| POST `user/allDel` | POST `users/bulk-delete` | json `{filter?}` | empty | Kept as a POST action (filter body; DELETE-with-body is hostile to proxies). |

### 6.7 Servers (nodes, groups, routes, protocol CRUD)

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `server/manage/getNodes` | GET `nodes` | none | array | Step-up-gated. `show`/`is_online`→bool, `rate`→number. vmess camelCase settings keys stay (R22 deferred). |
| POST `server/manage/sort` | POST `nodes/sort` | json `{<type>: {<id>: sort}}` | empty | Already JSON today; keeps shape. |
| GET `server/group/fetch` | GET `server-groups` | query | array | |
| POST `server/group/save` | POST `server-groups` / PATCH `server-groups/{id}` | json `{name}` | bare `{id}` (201) / empty | |
| POST `server/group/drop` | DELETE `server-groups/{id}` | none | empty | |
| GET `server/route/fetch` | GET `server-routes` | none | array | `match` always array. |
| POST `server/route/save` | POST `server-routes` / PATCH `server-routes/{id}` | json | bare `{id}` (201) / empty | `ROUTE_ACTIONS` vocabulary unchanged. |
| POST `server/route/drop` | DELETE `server-routes/{id}` | none | empty | |
| POST `server/{type}/save` | POST `servers/{type}` / PATCH `servers/{type}/{id}` | json | bare `{id}` (201) / empty | `{type}` ∈ shadowsocks, vmess, trojan, tuic, hysteria, vless, anytls, v2node. Legacy `param_present` gates map 1:1 to §4.4 double-Option. |
| POST `server/{type}/update` | PATCH `servers/{type}/{id}` | json `{show: bool}` | empty | Toggle merged. |
| POST `server/{type}/drop` | DELETE `servers/{type}/{id}` | none | empty | |
| POST `server/{type}/copy` | POST `servers/{type}/{id}/copy` | none | bare `{id}` (201) | Returns the new copy's id. |

### 6.8 Stats

| Old | New | Req | Resp | Notes |
| --- | --- | --- | --- | --- |
| GET `stat/getStat`, `stat/getOverride`, `stat/getRanking` | GET `stats/summary` | none | bare | Three legacy aliases → one route. |
| GET `stat/getServerTodayRank` / `getServerLastRank` | GET `stats/server-rank` `?window=today\|previous` | query | array | |
| GET `stat/getUserTodayRank` / `getUserLastRank` | GET `stats/user-rank` `?window=today\|previous` | query | array | |
| GET `stat/getOrder` | GET `stats/orders` | none | array of `{series, date, value}` | See series re-spec below. |
| GET `stat/getStatUser` | GET `stats/user-traffic` `?user_id=&page=&per_page=` | query | page | `server_rate` → number. |
| GET `stat/getStatRecord` | GET `stats/records` `?type=` | query | array of `{series, date, value}` | See series re-spec below. |

**Series re-spec** (`stats/orders`, `stats/records`): the legacy rows embed
Chinese literals as machine series keys (`"type": "注册人数"`,
`"佣金金额(已发放)"`, …) and ship money as yuan **floats**
(`paid_total as f64 / 100.0`) — both violate this dialect's own rules (the
data-embedded Chinese token is the same class the `模糊` operator was killed
for). The modern rows are `{series, date, value}` with stable snake_case
series slugs and **integer-cent** money; the client maps `series → i18n`
for display:

| Legacy `type` literal | `series` slug | `value` |
| --- | --- | --- |
| `注册人数` | `register_count` | integer count |
| `收款金额` | `paid_total` | integer cents (was yuan float) |
| `收款笔数` | `paid_count` | integer count |
| `佣金金额(已发放)` | `commission_paid_total` | integer cents (was yuan float) |
| `佣金笔数(已发放)` | `commission_paid_count` | integer count |

### 6.9 Staff namespace

`/api/v1/staff/…` keeps its own prefix and allow-list, with paths mirroring
the admin resources:

| Old (`/api/v1/staff/…`) | New (`/api/v1/staff/…`) | Notes |
| --- | --- | --- |
| GET `ticket/fetch` | GET `tickets`, GET `tickets/{id}` | page / bare |
| POST `ticket/reply` | POST `tickets/{id}/replies` | |
| POST `ticket/close` | POST `tickets/{id}/close` | |
| GET `user/getUserInfoById` | GET `users/{id}` | staff-redacted view |
| POST `user/update` | PATCH `users/{id}` | staff field allow-list unchanged |
| POST `user/sendMail` | POST `users/mail` | idempotency header unchanged |
| POST `user/ban` | POST `users/ban` | |
| GET `plan/fetch` | GET `plans` | |
| GET `notice/fetch` | GET `notices` | |
| POST `notice/save` / `notice/update` | POST `notices` / PATCH `notices/{id}` | |
| POST `notice/drop` | DELETE `notices/{id}` | |

### 6.10 Account MFA (native addition)

Privileged-account TOTP two-factor management. These routes are a native
addition with no legacy counterpart, so they carry no "Old" column and are
deliberately absent from the §13.1 two-world route map (there is no
oracle-world shape to compare against). They exist identically under the
dynamic admin prefix (`/api/v1/{secure_path}/…`) and the staff prefix
(`/api/v1/staff/…`); each caller manages only their own account's factor.

| Route | Req | Resp | Notes |
| --- | --- | --- | --- |
| GET `account/mfa` | none | bare `{totp_enabled: bool, totp_enabled_at: rfc3339\|null, totp_required: bool}` | `totp_required` mirrors the `admin_mfa_force` operator flag. |
| POST `account/mfa/totp` | none | bare `{secret, otpauth_url}` | Starts (or restarts) a **pending** enrollment. The base32 secret is returned exactly once and never readable again; an enabled factor must be disabled first (400 `mfa_already_enabled`). |
| POST `account/mfa/totp/confirm` | json `{code}` | empty 204 | Proves possession with a live code and flips the pending enrollment to enabled. 400 `mfa_setup_missing` without a pending enrollment; 401 `mfa_code_invalid` on a wrong code. |
| POST `account/mfa/totp/disable` | json `{code}` | empty 204 | Requires a live code (not just the step-up password): a hijacked session cannot silently remove the factor. 400 `mfa_not_enabled`; 401 `mfa_code_invalid`. |

Semantics:

- TOTP per RFC 6238 over HMAC-SHA1, 6 digits, 30-second steps, ±1-step
  verification window. Every accepted code consumes its time-step through a
  monotonic compare-and-set, so a code is one-time-use even under concurrent
  logins (replays answer 401 `mfa_code_invalid`).
- The §6 structural gates apply unchanged: session auth on every method and
  the blanket step-up requirement on the mutations, which therefore also land
  in the operator audit trail.
- Login integration is the §5.2 `totp_code` field. Wrong TOTP guesses consume
  the same limiter budget as wrong passwords; the code-absent
  `mfa_code_required` prompt (the normal two-phase login) does not.
- Lockout recovery is operator-only: `v2board-api reset-admin-totp <email>`
  removes the factor. There are no recovery codes.
- Mandatory enrollment: the operator config flag `admin_mfa_force` (safe
  group, default off) makes the factor compulsory. While it is on, an
  admin/staff session whose account has no enabled factor may reach only its
  own `account/mfa` family — every other route under either privileged
  prefix answers 403 `mfa_enrollment_required` (a permission failure, never
  a session teardown; login itself stays password-only until a factor
  exists). The GET body's `totp_required` mirrors the flag so the SPA gates
  its shell on enrollment instead of discovering the demand through 403s.

### 6.11 Operator audit trail (native addition)

Read access to the append-only `audit_log` table that the §6 structural
guards write (one row per authenticated admin/staff mutation; request
bodies are never recorded). Like §6.10 this is a native addition with no
legacy counterpart and no §13.1 two-world mapping. It exists **only**
under the dynamic admin prefix — the staff router deliberately does not
mirror it, so staff accounts cannot read the operator trail.

| Route | Req | Resp | Notes |
| --- | --- | --- | --- |
| GET `system/audit-logs` | query + filter DSL (§7) | page | Row shape `{id, actor_id, actor_email, session_id, surface, method, path, status_code, client_ip, request_id, created_at}`; `surface` is `admin`\|`staff`, `created_at` is RFC 3339, `client_ip`/`request_id` nullable. Default sort `created_at desc`. |

### 6.12 Granular admin RBAC (native addition)

Per-family staff grants over the admin namespace. Like §6.10/§6.11 this is
a native addition with no legacy counterpart and no §13.1 two-world
mapping. Legacy staff (`is_staff`) had only the fixed §6.9 staff-prefix
allow-list; §6.12 additionally lets staff enter the **admin** namespace
(under the dynamic `secure_path` prefix) with explicit, operator-assigned
per-family permissions. The §6.9 staff namespace itself is unchanged and
needs no grants.

**Registry (fixed, code-owned).** A permission is the string
`{family}:read` or `{family}:write` over exactly these families, which
partition the admin route table by first path segment:

| Family | Admin route segments |
| --- | --- |
| `config` | `config`, `email-templates`, `telegram-webhook`, `test-mail` |
| `system` | `system` (status, queues, logs, audit-logs) |
| `servers` | `nodes`, `server-groups`, `server-routes`, `servers` |
| `plans` | `plans` |
| `orders` | `orders` |
| `payments` | `payments`, `payment-providers`, `payment-reconciliations` |
| `coupons` | `coupons` |
| `gift_cards` | `gift-cards` |
| `users` | `users` |
| `tickets` | `tickets` |
| `notices` | `notices` |
| `knowledge` | `knowledge`, `knowledge-categories` |
| `stats` | `stats` |

Operators pick from the registry but cannot extend it; grant writes are
validated against it (422 `validation_failed` on unregistered entries).

**Semantics.**

- `is_admin` bypasses the registry entirely — full admin access, no grant
  list consulted. Ordinary users (neither flag) never enter the admin
  namespace regardless of stored grants.
- Staff authorization is method-mapped: GET/HEAD requires `{family}:read`,
  every mutation requires `{family}:write`; `write` implies `read`.
- The caller's own `account/mfa` family requires no grant for any
  admin-namespace principal, so §6.10 enrollment (and `admin_mfa_force`
  compliance) stays reachable for an ungranted staff account.
- Every denial — ungranted family, unmapped path, non-privileged caller —
  is the 403 `permission_denied`, never a session teardown (§3.2). The §6
  structural gates still apply on top: mandatory-MFA enrollment
  (`admin_mfa_force`, §6.10), the blanket mutation step-up, in-handler
  sensitive-read step-ups, and the §6.11 audit trail record staff actors
  in the admin namespace exactly like admins.
- Grants are read from the account row on every request: a grant edit
  takes effect immediately and never revokes sessions (role-flag changes
  to `is_admin`/`is_staff` keep their existing session-revoking behavior).

**Storage & transport.** Grants live on the user row
(`users.admin_permissions`, `JSONB NOT NULL DEFAULT '[]'`). The session
probe exposes the §5.2 staff pair (`is_staff: true` +
`admin_permissions`, possibly `[]`) so the admin SPA gates its shell and
navigation without extra round trips; admin user rows/detail carry
`admin_permissions` (§6.6), and PATCH `users/{id}` accepts the
full-replacement validated array. Imported legacy staff start with `[]`
(the migration default) — an operator grants families consciously
post-import.

---

## 7. Admin filter & sort DSL

Replaces `filter[i][key]/[condition]/[value]` bracket params, the `模糊`
Chinese operator token, and the `'null'` string sentinel.

### 7.1 Filter

One query parameter, `filter`, containing a URL-encoded JSON array of
clauses (AND-combined, matching legacy semantics):

```
GET /{secure_path}/users?filter=[{"field":"email","op":"like","value":"@gmail.com"},{"field":"banned","op":"eq","value":true},{"field":"plan_id","op":"eq","value":null}]
```

Clause shape: `{ "field": string, "op": Op, "value": Value }`.

Operator vocabulary (closed set):

| Op | SQL | Value types | Notes |
| --- | --- | --- | --- |
| `eq` | `=` (or `IS NULL` when value is null) | scalar, null | Replaces `=`, `is`, and the `'null'` sentinel. Email equality keeps the trimmed/lowercased comparison. |
| `neq` | `<>` (or `IS NOT NULL`) | scalar, null | Replaces `!=`/`<>`/`not`. |
| `like` | `ILIKE '%v%'` (wildcards escaped) | string | Replaces `like`/`模糊`; **literal** substring, case-insensitive: `%`, `_`, and `\` in the value are escaped before binding, so they match themselves. Deliberate divergence from the legacy anchor, which binds `%{value}%` unescaped (raw `%`/`_` act as wildcards); §13.2 notes the canonicalizer consequence. |
| `gt` / `gte` | `>` / `>=` | number, RFC 3339 string | |
| `lt` / `lte` | `<` / `<=` | number, RFC 3339 string | |
| `in` | `= ANY(...)` | non-empty array of scalars | New; replaces repeated eq clauses. |

Field whitelists are per endpoint; every list route is enumerated here so
no wave has to improvise:

| Endpoint | Filter fields |
| --- | --- |
| GET `users` | the typed `UserFilterField` whitelist in `application/src/admin_user.rs`; SQL expressions are allowlisted in `db/src/admin_user.rs` |
| GET `orders` | the typed order predicate whitelist translated in `api/src/admin/commerce.rs` and executed through `application/src/admin_order.rs` + `db/src/admin_order.rs` |
| GET `system/logs` | `level` only |
| GET `system/audit-logs` | `surface`, `actor_email`, `method` |
| GET `coupons`, GET `gift-cards` | **none** — no legacy filter support; none invented (§6.3) |
| GET `payment-reconciliations` | **not DSL** — dedicated named scalar params (§6.4) |
| GET `tickets` (admin) | **not DSL** — dedicated `status`/`reply_status`/`email` params (§6.5) |

Timestamp-typed fields accept RFC 3339 values and compare on the stored
epoch. Boolean-typed fields accept JSON booleans. Unknown `field`, unknown
`op`, type-mismatched `value`, or unparsable JSON → 422 `validation_failed`
with `errors: {"filter": [reason]}`.

Body-borne `filter` (the §6.6 bulk actions `users/export`, `users/mail`,
`users/ban`, `users/bulk-delete`): the JSON body carries the **same clause
array unencoded** — a raw JSON array value, never a string-encoded copy of
the query-param form.

Validation strategy:

- **serde** (backend): `Vec<FilterClause>` with
  `#[serde(deny_unknown_fields)]`, `op` a unit enum, `value` a bounded
  `FilterValue` enum (string/number/bool/null/array-of-scalar); a second
  pass resolves `field` against the endpoint whitelist and coerces `value`
  to the column type. The SQL builder keeps binding values (never
  interpolating), as `push_user_where`/`push_order_where` do today.
- **zod** (api-client): per-endpoint
  `z.array(z.object({ field: z.enum(columns), op: filterOpSchema, value: … }))`
  request contracts serialize with `JSON.stringify` into the `filter` query
  param. The `adminFilterSchema` `{key, condition, value}` shape and its
  `AdminFilter` type are retired.

### 7.2 Sort

Two scalar query params, enum-validated on both sides:

- `sort_by` — a whitelisted column (same per-endpoint list as filters, plus
  computed `total_used` for users). Default: `created_at`.
- `sort_dir` — `asc` | `desc`. Default `desc`.

Replaces `sort` + `sort_type` (`"ASC"`-exact match). Invalid values → 422
(legacy silently defaulted; the new dialect rejects).

---

## 8. Pagination

- Request: `page` (1-based, default 1) and `per_page` (default = the
  endpoint's legacy default, 10 unless noted; max 100). Replaces
  `current`/`pageSize`/`page_size`.
- Response: `{ "items": [...], "total": <i64> }`. Replaces the
  `{data, total}` page envelope.
- Non-paginated lists are bare arrays; never wrap them in `items`.
- Out-of-range values → 422 `validation_failed`; the shared
  `crates/compat` `Pagination::resolve` policy is exercised by the typed API
  handlers.
- Frontend history-pagination display clamping stays a Tier-2 client
  concern; the raw `page` is sent unclamped, as today.

---

## 9. Named-object replacements and unions

### 9.1 `GET /user/stats` (was getStat tuple)

`[pending_orders, pending_tickets, invited_users]` →

```json
{ "pending_order_count": 2, "pending_ticket_count": 0, "invited_user_count": 7 }
```

### 9.2 Invite stat (was 5-tuple)

`stat: [registered, valid_commission, pending_commission, commission_rate,
available_commission]` (`crates/db/src/invite.rs`) →

```json
{
  "registered_count": 12,
  "valid_commission": 12300,
  "pending_commission": 4500,
  "commission_rate": 10,
  "available_commission": 8000
}
```

Commission values stay integer cents; `commission_rate` stays an integer
percent (default 10 when unset).

### 9.3 Checkout discriminated union

`POST /user/orders/{trade_no}/checkout` responses replace the
`{type: -1|0|1, data: string|bool}` envelope:

```json
{ "kind": "qr_code",  "payload": "<string the client renders as a QR>" }
{ "kind": "redirect", "url": "https://gateway.example/pay/..." }
{ "kind": "settled" }
```

| Legacy | New |
| --- | --- |
| `type: 0`, `data: <string>` | `kind: "qr_code"`, `payload` |
| `type: 1`, `data: <url>` | `kind: "redirect"`, `url` |
| `type: -1`, `data: true` | `kind: "settled"` (paid without a gateway hop, e.g. balance) |
| unknown `type` | never emitted; gateway misbehavior becomes 400 `payment_gateway_unsupported` |

The Stripe Payment Element flow keeps its dedicated
`/stripe-intent` + client confirmation + signed-webhook settlement path
(external payloads frozen).

### 9.4 Other bare-scalar → named-object conversions

| Endpoint | Was | Becomes |
| --- | --- | --- |
| POST `/user/orders` | bare trade-no string | `{"trade_no": "..."}` |
| GET `/user/orders/{trade_no}/status` | bare number | `{"status": n}` |
| POST `/user/subscription/reset-token` | bare URL string | `{"subscribe_url": "..."}` |
| POST `/auth/quick-login-url` | bare URL string | `{"url": "..."}` |
| GET `/user/sessions` | object keyed by session digest | array with `session_id` |
| POST `{secure_path}/test-mail` | `{data: true, log}` | `{"sent": bool, "log": {...}}` |
| POST `/user/gift-card-redemptions` | `{data: true, type, value}` | `{"type": n, "value": cents\|null}` |

---

## 10. Routing, HTML delivery, minted URLs

### 10.1 History routing

Both SPAs switch from `createHashRouter` to `createBrowserRouter`
(`apps/user/src/App.tsx`, `apps/admin/src/App.tsx`). Route *path shapes*
(`/login`, `/dashboard`, `/order/:trade_no`, …) are unchanged — only the
`#/` prefix dies.

### 10.2 Rust HTML fallback

`crates/api/src/fallback.rs` extends from exact-path matching to subtree
fallback for GET/HEAD:

1. Reserved, never HTML: `/api/*` (unknown API paths stay 404
   problem+json/legacy per namespace), `/assets/*` (hashed-asset gate,
   404 for unknown names), `/healthz`, `/readyz`, `/robots.txt` (fixed
   public crawler policy, routed ahead of the fallback; it never lists the
   admin path), and the operator `subscribe_path` alias.
2. `/{admin_path}` and `/{admin_path}/*` → admin `index.html`
   (`frontend::render(Admin)`), with runtime-config injection.
3. Every other GET/HEAD path → user `index.html`, gated by the existing
   `safe_mode` Host check (403 empty body on mismatch, unchanged). This
   includes deep links like `/order/T123` and unknown paths (the SPA renders
   its own 404 view).
4. Non-GET/HEAD unmatched requests → 404 problem+json — **after** the
   fallback's method-aware admin API dispatch under the live dynamic
   `/api/v1/{admin_path}/` prefix has declined the path (§6 preamble; admin
   PATCH/PUT/DELETE must survive a runtime `secure_path` change).

**Reserved-segment validation (new, enforced at config save).** Rule 2
serves the admin subtree ahead of the user-SPA fallback, and
`config.admin_path()` resolves `secure_path` → `frontend_admin_path` →
`crc32b_hex(app_key)` (that fallback precedence is normative). Today
`secure_path` is only validated syntactically (≥ 8 chars of
alphanumeric/`_`/`-`) and `frontend_admin_path` is not validated at all —
legal values like `dashboard`, `knowledge`, `order`, or `login` would
shadow user-SPA deep links under history routing (including the
backend-minted payment-return URL `{app_url}/order/{trade_no}`, a Tier-1
outcome), and the same knob builds the admin API prefix
(`admin_api_route`), where `client`/`server`/`guest` would collide with
frozen §2 namespaces. Therefore:

- `frontend_admin_path` gets the same syntactic validation as
  `secure_path`.
- The resolved admin path (from either knob) may **not** equal any reserved
  top-level segment: `api`, `assets`, `healthz`, `readyz`, `robots.txt`, the
  operator `subscribe_path` first segment, any user-SPA top-level route root
  (`login`, `register`, `forgetpassword`, `dashboard`, `plan`, `order`,
  `profile`, `node`, `traffic`, `invite`, `ticket`, `knowledge`), or any
  reserved API namespace (`auth`, `user`, `public`, `staff`, `client`,
  `server`, `guest`, `passport`).
- Violations reject the config save with 400 `config_validation_failed`
  (§3.4). The `crc32b_hex` default is hex and cannot collide.

`Cache-Control: no-store` on HTML responses stays. `previous`-release asset
fallback and the deploy-contract grammar are untouched.

### 10.3 `legacy_hash_redirect_enable`

New admin-configurable boolean, **default ON**, delivered to both apps via
the runtime-config injection (`__V2BOARD_RUNTIME_CONFIG__` gains
`"legacy_hash_redirect_enable": true|false`), and edited in the admin config
UI (site group).

Semantics (purely client-side — a hash never reaches the server, so old
`/#/x?y` URLs always arrive as `/` or `/{admin_path}`):

- ON: before router creation, if `location.hash` matches `#/…`, the SPA
  boot translates it to a history URL via
  `history.replaceState(null, '', pathAndQueryFrom(hash))` — e.g.
  `/#/order/T1?from=mail` → `/order/T1?from=mail`; the admin app resolves
  against its `/{admin_path}` base. Invalid/foreign hashes are left alone.
- OFF: the hash is ignored; the router boots on the server-delivered path.

This replaces (and generalizes) the current pre-router hash normalization in
`frontend/packages/config/src/hash-route.ts`; that module's
auth-redirect-safety logic (public-route matching, auth-storage-key gate) is
preserved in the history-routing guard, because auth redirect outcomes are
Tier-1.

### 10.4 Backend-minted URL formats (path-style)

| Mint site | Was | Becomes |
| --- | --- | --- |
| Email verify / quick-login target (`application/src/auth.rs` `login_redirect_url`) | `{app_url}/#/login?verify={token}&redirect={path}` | `{app_url}/login?verify={token}&redirect={path}` |
| Payment return URL (`payment-adapters/src/gateway/integrations.rs`) — both branches: absolute with `app_url`, and the relative fallback handed to providers when `app_url` is unset | `{app_url}/#/order/{trade_no}` / `/#/order/{trade_no}` | `{app_url}/order/{trade_no}` / `/order/{trade_no}` |
| `GET /auth/quick-login?token=` 302 Location | same as login_redirect_url | same, path-style |

`?verify=` / `?redirect=` query names and token semantics are unchanged
(the backend emails into these SPA routes — Tier-1). `redirect` values stay
same-origin path fragments; the existing redirect-safety validation on the
client is retained.

### 10.5 `custom_html` removal and CSP tightening

Removed entirely, in one wave:

- Config field `frontend_custom_html` (AppConfig, admin config
  fetch/save group, zod `frontendConfigSchema`), the admin config UI
  control, and any importer mapping.
- The `<!-- V2BOARD_CUSTOM_HTML -->` marker in `apps/user/index.html` and
  the `replacen` in `crates/api/src/frontend.rs`.

CSP then tightens on HTML-serving responses (frontend.rs / routes.rs
`security_response_headers`), replacing the lone `frame-ancestors 'self'`:

```
default-src 'self';
script-src 'self' 'sha256-<dark-mode-prepaint-hash(es)>'
    https://js.stripe.com https://www.recaptcha.net https://www.gstatic.com;
style-src 'self' 'unsafe-inline';
img-src 'self' data: https:;
connect-src 'self' https://api.stripe.com https://m.stripe.network
    https://r.stripe.com https://q.stripe.com https://www.recaptcha.net;
frame-src https://js.stripe.com https://hooks.stripe.com
    https://m.stripe.network https://www.recaptcha.net;
frame-ancestors 'self';
base-uri 'self';
form-action 'self';
object-src 'none'
```

- The retained inline scripts are the dark-mode pre-paint script and the
  runtime-config `type="application/json"` data element (data elements don't
  execute and need no allowance). The pre-paint script is allowed by the
  SHA-256 **hash(es) of each built app's inline pre-paint script** — both
  apps carry one (`apps/user/index.html`, `apps/admin/index.html`) and Vite
  may emit them differently per app, so this can be two hashes, not one.
  They are computed at build time (`frontend/scripts/build-deploy.mjs`, which
  also drops its `<!-- V2BOARD_CUSTOM_HTML -->` marker assertion in the same
  change) and pinned by the deploy contract
  (`frontend/scripts/deploy-contract.mjs`), so a drifted inline script fails
  the build, not the browser.
- Stripe entries exist because the user app loads `@stripe/stripe-js`
  (`js.stripe.com/v3`) and mounts the Payment Element, which additionally
  requires `https://m.stripe.network` (frame + connect) and the
  `r.stripe.com`/`q.stripe.com` telemetry endpoints per Stripe's CSP
  guidance.
- reCAPTCHA entries exist because the auth surface dynamically injects
  `https://www.recaptcha.net/recaptcha/api.js`
  (`apps/user/src/pages/auth/auth-recaptcha.tsx`) — the app deliberately
  pins the China-reachable `recaptcha.net` host, not `google.com` — which
  pulls `www.gstatic.com` script resources and renders its challenge in a
  `www.recaptcha.net` iframe. The reCAPTCHA verification payload itself is
  a frozen §2 integration; a CSP that blocked the loader would kill
  `recaptcha_data` carriage on register/forget/send-email-verify.
- `img-src https:` retains operator logo/background URLs
  (`logo`, `frontend_background_url`) and knowledge/notice images.
- API-route responses keep `frame-ancestors 'self'` plus the existing
  no-store cache headers; the full policy above applies to HTML documents.
  Implementation shape: `frontend.rs` builds and sets the document policy on
  the HTML response; the shared security middleware only fills the
  `frame-ancestors 'self'` baseline when a handler has not already claimed
  the header (`or_insert`).

---

## 11. Locale persistence

- One canonical client key: `localStorage["v2board_locale"]`, holding a
  supported locale code (`zh-CN`, `en-US`, `ja-JP`, `vi-VN`, `ko-KR`,
  `zh-TW`).
- One-time migration read, in order, at i18n bootstrap
  (`packages/i18n/src/bootstrap.ts`): `v2board_locale` → `umi_locale` →
  legacy i18n cookie → `window.g_lang` → navigator language → `zh-CN`.
  On first resolution the value is written to `v2board_locale`; legacy keys
  are never written again (read-only fallbacks, eventually dead).
- `window.g_lang` as a *global mutable API* dies; nothing may assign or read
  it after bootstrap (the migration read above is the single exception).
- The active locale is sent as `Accept-Language` (§4.3); language
  persistence across login/logout/reload remains Tier-1 behavior.

---

## 12. Config & importer surface changes

- `frontend_custom_html` is deleted from AppConfig, the admin config
  contract/UI, `crates/api/src/frontend.rs`, and the `index.html` marker
  (plus the `build-deploy.mjs` marker assertion, §10.5). **No lifecycle
  importer mapping for it exists and none may be added** — the importer
  emits `api.config.json` from the manifest's `api_boot_config`, not from
  legacy settings, so there is no importer code to touch. No part of the
  pre-release MySQL import contract changes.
- New config key: `legacy_hash_redirect_enable` (bool, default `true`),
  site group, admin-editable, injected into runtime config (§10.3). The
  importer does not map it (fresh default).
- New config security validation: the reserved-segment admin-path rule
  (§10.2) — `secure_path`/`frontend_admin_path` syntactic validation plus
  the reserved-top-level-segment rejection, surfacing as 400
  `config_validation_failed`.
- `safe_mode` host gate and the `dark_mode` cookie mechanism stay unchanged.

---

## 13. Parity harness — canonical-semantics adapters

The product-owned source world is the standing `make interaction-parity` gate.
The read-only reference oracle stays only in the explicit
`make legacy-oracle-parity` migration lane; cross-world **byte** equality for
internal requests/responses is retired and that optional comparison drops to
canonical semantics. `make real-stack-e2e` separately bypasses API fixtures and
drives a browser through Rust with restricted PostgreSQL/Redis runtime roles.

Harness layout today (`frontend/tests/`): `parity-spec.mjs` (spec factory),
`specs/*.spec.mjs` (per-surface groups), `lib/world.mjs` (source driver plus
optional legacy prepare → run → assertUsefulInteraction → normalize → compare),
`lib/api-fixtures.mjs` (route interception), `lib/fixture-data.mjs`
(fixture objects), `lib/normalizers.mjs` (Tier-1 reducers),
`lib/interaction-scenarios.mjs` + `lib/runners/**` + `lib/state-readers/**`, and
`real-stack/` (fixture-free black-box journeys).

New adapter layer, `frontend/tests/lib/dialect/`:

1. **`route-map.mjs`** — the machine-readable form of §5–6: an array of
   `{ id, legacy: {method, path|pattern}, modern: {method, path|pattern} }`
   entries (admin entries parameterized by `secure_path`). It is the single
   place scenario runners and fixtures resolve a *canonical route id* to a
   per-world URL matcher. `make parity-config-audit` extends to assert the
   map covers every route this document lists.
2. **`request-canonicalizer.mjs`** — decodes a captured request per world
   into one canonical object: form-urlencoded/bracket-array bodies and
   `current/pageSize`/`filter[i][…]` params (oracle world) and JSON
   bodies/`page/per_page`/`filter` JSON (source world) both reduce to
   `{routeId, params, body}` with canonical types (booleans as booleans,
   arrays as arrays, cents as integers, timestamps as epoch seconds
   canonically, tuple/object equivalences from §9 folded to the named
   form). Tier-1 payload comparison happens on this canonical object. The
   admin-config fold discards the source-only `expected_revision` solely for
   comparison with the read-only legacy oracle, which has no concurrency
   token; source wire/unit/E2E coverage still requires and asserts it.
   `like` filter values compare on the raw string; because the modern
   backend escapes SQL wildcards while the oracle passes them through
   (§7.1), fixtures must not use `%`/`_`-bearing filter values in
   cross-world scenarios — the recorded divergence only manifests there.
3. **`error-canonicalizer.mjs`** — maps each world's error surface to
   `{status_class, code}`: source world reads problem+json `code` directly;
   oracle world looks up the legacy `message` literal in the
   anchor-message→code table generated from §3.4. The 403-vs-401 session
   split is normalized via the canonical code (`session_expired`). One
   non-error equivalence is also pinned here: the oracle's 503
   `配置已提交…` config-activation message maps to the same canonical
   outcome as the source world's 202
   `{"activation": "pending", "revision": n}` (§6.1); canonical comparison
   may discard `revision` only for the read-only legacy oracle, which has no
   native operator-revision value.
4. **`page-location-canonicalizer.mjs`** — the SPA-URL adapter. After W1
   the source world is history-routed while the oracle stays hash-routed,
   so raw `window.location` reads diverge on every location-asserting
   scenario (`state-readers/auth.mjs`, `shared.mjs` capture
   `location.hash`; `world.mjs` stableJson-compares both worlds). This
   adapter (a) maps a canonical SPA route path to the per-world **entry
   URL** — `/#/x?y` for the oracle, `/x?y` for the source world; scenario
   entry paths in `scenario-meta.mjs` become canonical route paths, and
   hash-style entries are retained on the source world only for the
   `legacy_hash_redirect_enable`-ON translation cases W1 adds — and (b)
   canonicalizes location reads (oracle `#/x?y` and source
   `pathname+search`) into one canonical route object consumed by state
   readers and normalizers. The `parity-config-audit` validator that
   currently throws on non-hash scenario paths
   (`frontend/scripts/parity-config-audit.mjs`) is reworked in W1 to
   validate canonical route paths instead.
5. **World-aware fixtures** — `fixture-data.mjs` stays the canonical data
   source; `api-fixtures.mjs` gains per-world emitters:
   - oracle world: legacy shapes — `{data}` envelopes, epoch ints, 0/1
     flags, and the HTTP-200 `{code:400}` error emulation the reference
     build expects;
   - source world: modern shapes — bare objects/`{items,total}`, RFC 3339,
     booleans, and **real HTTP semantics** (real 4xx statuses with
     problem+json bodies; no in-body `code` emulation). The api-client's
     in-body `code !== 200` unwrap path (`unwrapBackendEnvelope` in
     `client.ts`) is deleted in the api-client wave once no source-world
     fixture emits it.
6. **Normalizers** — `normalizers.mjs` Tier-1 reducers consume canonical
   request/error/location objects from (2)/(3)/(4) instead of raw
   URLs/bodies; Tier-2 presentation dropping is unchanged.

Scenario authoring rules stay: union selectors (shadcn-first, Ant fallback)
so one runner drives both worlds; no per-world `run(page)` branching —
world-specific behavior lives only in the adapter layer.

Goldens: `frontend/packages/api-client/src/goldens.test.ts` and
`backend/rust/crates/api/src/golden_wire.rs` re-pin to the modern wire
shapes per family wave; legacy goldens for a family are deleted in the same
commit series that switches it (no dual goldens).

---

## 14. Backend & client implementation skeleton (normative)

- **Contract registry**: `crates/api-contract/src/operations.rs` is the
  executable source for all 158 internal operation ids, methods, paths,
  parameters, authentication, and explicit success/error representations.
  Its named DTO modules cover all 64 JSON request bodies and all 95 JSON
  success representations; OpenAPI and generated TypeScript/Zod are derived
  artifacts, never parallel hand-written contracts.
- **Rust errors**: `crates/problem-code/src/lib.rs` owns the stable code/status
  registry, `crates/api-contract/src/problem.rs` owns the RFC 9457 wire DTO,
  and `crates/compat/src/problem.rs` adapts it to Axum responses with localized
  `detail`. Internal handlers map application failures to this boundary;
  legacy `ApiError` behavior is confined to the §2 frozen external routes.
- **Rust routing and extractors**: the operation-router macro mounts every
  internal operation exactly once and derives method/path from the registry.
  Typed `Json<T>`, path, and query extractors replace flattened form maps.
  Admin resources remain method-aware and are re-dispatched behind the live
  `secure_path` check, so a runtime prefix change needs no restart (§6
  preamble, §10.2 rule 4).
- **Use-case boundary**: HTTP handlers translate contract DTOs into commands
  owned by `crates/application`; application services depend on explicit
  ports, while PostgreSQL, Redis, SMTP, provider, and protocol details live in
  `db` and the focused `*-adapters` crates. Transport DTOs and persistence rows
  do not become application models by convenience.
- **api-client**: generated operation metadata and Zod schemas drive each
  wrapper. `request()` returns the parsed bare body; pages are
  `{items,total}`; errors surface `{status, code, detail, errors}`. The
  deleted envelope and hand-written `contracts.ts` paths cannot be used as a
  fallback.
- **Types and presentation**: generated `@v2board/types` wire interfaces use
  booleans, arrays, and RFC 3339 strings. Display normalization such as
  `formatScaledBackendValue` remains client-side and outside the wire
  contract.

---

## 15. Open issues

1. **CSP `img-src` refinement** — §10.5 now enumerates every dynamic
   loader in the codebase (Stripe via `@stripe/stripe-js` including
   `m.stripe.network` and telemetry, reCAPTCHA via the pinned
   `recaptcha.net` loader plus `gstatic.com`); the W1-implementation-time
   repeat enumeration confirmed no new loader has landed: the only dynamic
   script loaders remain `loadStripe` from `@stripe/stripe-js/pure`
   (`apps/user/src/pages/order/stripe-payment-form.tsx`) and the
   `https://www.recaptcha.net/recaptcha/api.js` injector
   (`apps/user/src/pages/auth/auth-recaptcha.tsx`).
   Remaining open point: `img-src` refinement only (payment gateway QR
   values are rendered client-side from strings, not fetched cross-origin
   as scripts, but operator logo/background/knowledge image hosts keep
   `img-src https:` broad for now).
3. **`GET /auth/quick-login` residual consumers** — old
   `/passport/auth/token2Login?token=` links minted by external tooling
   break (accepted by owner decision, noted for release notes).
4. **Admin order identifier** — standardizing detail on `trade_no` (§6.4)
   requires the admin list rows to key detail fetches by `trade_no`;
   confirmed present on every row, but the reconciliation view's join to
   orders should be re-checked at implementation time.
5. **Per-endpoint `per_page` defaults** — legacy defaults differ (10 for
   invite details; 5 for user notices, already pinned in §5.8; admin lists
   effectively client-driven). Each family wave pins its default in this
   document's route tables when it lands (admin tickets in W14).

(The draft's sixth open issue — which of W11/W12 carries the filter/sort
DSL module — is resolved: the DSL module ships in **W9** with its first
consumer, `GET system/logs`; W11/W12 consume it. See Appendix A.)

---

## Appendix A — Wave-by-wave migration map

Rules: waves ship in order; each wave is a vertical slice (Rust routes +
error codes + api-client + app pages + fixtures + scenarios + goldens) that
switches its family atomically. No wave leaves a family half-dialect. Wave
0/1 are cross-cutting enablers. `make behavior-parity` and focused
`INTERACTION_PARITY_SCENARIOS` shards gate every wave.

The `Files` bullets below are an immutable migration log: they name files as
they existed when each wave landed and may therefore mention the retired
`crates/domain` or deleted hand-written `contracts.ts`. Current ownership is
the contract registry/DTOs in `api-contract`, use cases and ports in
`application`, outbound implementations in `db` and focused `*-adapters`, and
thin inbound adapters in `api`.

### W0 — Foundations (no route switches)

- Rust: `Problem` type + `Code` enum + Accept-Language localization plumbing;
  JSON extractor helpers (double-Option, deny_unknown_fields patterns);
  pagination parsing module; RFC 3339 serde helpers.
- api-client: v2 core (`Bearer` header assembly, Accept-Language, problem
  parsing, `pageSchema`), inert alongside the legacy paths.
- Harness: `tests/lib/dialect/{route-map,request-canonicalizer,error-canonicalizer}.mjs`,
  world-aware fixture emitters (initially both worlds emit legacy).
- Risk: none user-visible; everything unreferenced until W2+.

### W1 — Routing shell, custom_html, locale (cross-cutting atomics)

- Routes affected: none (API). HTML delivery: fallback.rs subtree fallback,
  history routing in both apps, `legacy_hash_redirect_enable`
  (config + runtime injection + boot translator), the reserved-segment
  admin-path validation (§10.2/§12), minted-URL formats (`sessions.rs`,
  `payment_integrations.rs` — both the absolute and the relative
  payment-return branch), custom_html removal + CSP (§10.5, §12), locale
  key migration + api-client `Accept-Language` sending (§11, §4.3; verify
  the `Accept-Language` resolver, `v2board_locale` vocabulary, and
  runtime-config `i18n` key all bind to `ENABLED_LOCALES`). The
  response-rewrite localization middleware is **not** touched: it stays
  keyed on `Content-Language` for the §2 external namespaces (§3.1);
  internal problem+json localization is built per family from W2 on.
- Files: `crates/api/{fallback,frontend,routes}.rs`,
  `apps/{user,admin}/src/App.tsx`, `packages/config/src/hash-route.ts`,
  `packages/i18n/src/bootstrap.ts`, `apps/*/index.html`,
  `frontend/scripts/build-deploy.mjs` (drop the custom-HTML marker
  assertion; add the per-app inline pre-paint script hash extraction),
  `frontend/scripts/parity-config-audit.mjs` (the validator that today
  throws on non-hash scenario paths learns canonical route paths, §13.4),
  deploy contract.
- Scenarios/fixtures: `specs/auth.spec.mjs` (redirect outcomes),
  `specs/dashboard.spec.mjs`, all `runners/**` URL assertions via the
  route-map, per-world entry URLs + location reads via the
  page-location canonicalizer (§13.4); `make deploy-smoke`,
  `make cloudflared-config-audit` untouched but re-run.
- Risk: highest-blast-radius wave (every deep link). Mitigation: hash
  translator default-ON; parity scenarios add hash-URL entry cases; the
  reserved-segment validation must land in the same series the subtree
  fallback does (an unvalidated `frontend_admin_path` shadows user routes
  only once rule 2 claims the subtree).

### W2 — Auth family

- Routes: §5.2 (`/auth/*`), plus two cross-cutting flips that are shared
  middleware and therefore switch **globally** in this wave's commit
  series: `session_expired` 401 semantics (the teardown hook moves to
  401+code, and 401s gain `WWW-Authenticate`), and the
  `Authorization: Bearer` scheme (§4.2 — the api-client prepends `Bearer `
  on every internal request, including not-yet-migrated families, while
  `select_auth_data` starts requiring and stripping the prefix on internal
  routes; §2 external routes never used `Authorization`).
- Historical files at landing: `crates/api/src/auth.rs`, the now-retired
  `crates/domain/src/auth/*`;
  `packages/api-client/src/endpoints/passport.ts`, `client.ts` error hooks;
  `apps/user/src/pages/auth/*`, `apps/admin/src/pages/login*`,
  `apps/*/src/lib/auth.ts`. Current backend owners are
  `application/src/auth.rs`, `auth-adapters/src/{cache,external,password}.rs`,
  `db/src/auth.rs`, and the `api/src/auth.rs` inbound adapter.
- Fixtures/scenarios: `runners/auth.mjs`, `specs/auth.spec.mjs`,
  `state-readers/auth.mjs`, fetch-failure auth cases.
- Risk: token2Login split (GET redirect vs POST exchange), recaptcha and TOS
  gate coverage, `authorization` storage key must remain byte-identical.

### W3 — Public config + user config/content reads

- Routes: §5.1, `/user/config`, `/user/notices`, §5.8 knowledge routes,
  `/user/telegram-bot`.
- Files: `crates/api/src/client.rs` (guest config), `crates/api/src/user/content.rs`;
  `endpoints/guest.ts`, parts of `endpoints/user.ts`; knowledge/dashboard pages.
- Fixtures/scenarios: `runners/knowledge.mjs`, `runners/dashboard.mjs`
  (notice popup), `specs/{knowledge,dashboard}.spec.mjs`.
- Risk: notice `弹窗` tag popup and knowledge `copy()/jump()` hooks are
  Tier-1; keep fixture bodies re-substituted per request.

### W4 — Commerce

- Routes: §5.5 + checkout union + `/user/payment-methods`.
- Historical files at landing: `crates/api/src/commerce.rs`, the now-retired
  `crates/domain/src/order*`; `endpoints/user.ts` commerce section, the now-
  deleted `contracts.ts` order/plan/coupon schemas;
  `apps/user/src/pages/{plan,order}*`, `stripe-payment-form.tsx`,
  and `apps/user/src/pages/profile/wallet-card.tsx` (the deposit arm of the
  §5.5 order union replaces its `plan_id: 0` + `period: "deposit"` sentinel
  payload in this wave, even though the page belongs to W5's surface).
  Current backend owners are `application/src/{order,plan,payment}.rs`,
  `db/src/{order_checkout,order_lifecycle,order_runtime,plan}.rs`, and the
  `order-adapters`/`payment-adapters` crates behind the API inbound adapters.
- Fixtures/scenarios: `runners/commerce.mjs`, `specs/commerce.spec.mjs`,
  commerce fixtures in `fixture-data.mjs` (world-split), commerce goldens.
- Risk: largest error-code surface (coupons/sold-out/periods); Stripe
  external payloads must not shift; keep `#cashier` + commerce testids.

### W5 — Profile, account, subscription

- Routes: §5.3 (minus the auth session rows, minus
  `/user/commission-transfers` — owned by W7 — and minus `/user/config` and
  `/user/telegram-bot` — owned by W3) + §5.4 subscription rows. Every route
  has exactly one owning wave.
- Files: `crates/api/src/user/{account,subscription,stats}.rs`;
  `endpoints/user.ts`; `apps/user/src/pages/profile*`, dashboard
  subscription cards, traffic page bits that read subscription.
- Fixtures/scenarios: `runners/profile.mjs`, `runners/dashboard.mjs`,
  `specs/{profile,dashboard}.spec.mjs`, `state-readers/{profile,dashboard}.mjs`.
- Risk: password-change redirect and reset-token rotation are Tier-1;
  sessions map→array reshapes the active-sessions UI.

### W6 — Service usage

- Routes: `/user/servers`, `/user/traffic-logs` (§5.4 remainder).
- Files: `crates/api/src/user/stats.rs` (servers/traffic);
  `endpoints/user.ts`; `apps/user/src/pages/{node,traffic}*` including the
  parallel-fetch + gated-render change (§4.6).
- Fixtures/scenarios: `runners/service.mjs`, `specs/service.spec.mjs`,
  `state-readers/service.mjs`.
- Risk: `(u+d)*server_rate` math with `server_rate` now numeric; legacy
  traffic-charge coercion pin updates.

### W7 — Invite & commissions

- Routes: §5.6 + `/user/commission-transfers`.
- Files: `crates/api/src/user/invite.rs`; `endpoints/user.ts`;
  `apps/user/src/pages/invite*`.
- Fixtures/scenarios: `runners/invite.mjs`, `specs/invite.spec.mjs`,
  `state-readers/invite.mjs`.
- Risk: stat 5-tuple → named object touches every invite widget; `100*amount`
  transfer conversion stays at the api-client boundary.

### W8 — Tickets (user)

- Routes: §5.7.
- Files: `crates/api/src/ticket.rs`; `endpoints/user.ts`;
  `apps/user/src/pages/ticket*`.
- Fixtures/scenarios: `runners/ticket.mjs`, `specs/ticket.spec.mjs`,
  `state-readers/ticket.mjs`.
- Risk: withdraw ticket payload is invite-flow adjacent; reply polling
  cadence stays 5 s.

### W9 — Admin config & system

- Routes: §6.1. The §7 filter/sort DSL module ships **here**, with its
  first consumer (`GET system/logs`); W11/W12 reuse it. No wave ships a
  modern route that still parses legacy `filter[i][key]` brackets.
- Historical files at landing: the now-retired
  `crates/domain/src/admin/{configuration,statistics}.rs` (system rows),
  admin dispatcher rows; `endpoints/admin.ts` config/system. Current backend
  owners are `application/src/{configuration,logs,system_monitoring}.rs`,
  `configuration-adapters`, and the corresponding `db`/`api` adapters;
  `apps/admin/src/pages/config*`, system pages.
- Fixtures/scenarios: `runners/admin/config.mjs`,
  `specs/admin-config.spec.mjs`; focused shards list exact
  `INTERACTION_PARITY_SCENARIOS` labels (matching is end-anchored — a bare
  `admin` prefix selects nothing).
- Risk: the config save conflict code (409), revision-bearing GET views, and
  the revision-bearing 202 activation-pending refetch-not-resubmit flow
  (§6.1) drive admin UX;
  array-valued config fields switch off the `'[]'` hack.

### W10 — Admin content CRUD (notice, knowledge, coupon, gift card)

- Routes: §6.3.
- Historical files at landing: the now-retired
  `crates/domain/src/admin/{content,codes}.rs`; `endpoints/admin.ts`. Current
  backend owners are `application/src/{content,promotion,giftcard}.rs`,
  `db/src/{content,coupon,giftcard}.rs`, and `api/src/admin/content.rs`;
  `apps/admin/src/pages/{notice,knowledge,coupon,giftcard}*`.
- Fixtures/scenarios: `runners/admin/coupon-giftcard-notice-knowledge.mjs`,
  `specs/admin-coupon-giftcard-notice-knowledge.spec.mjs`.
- Risk: coupon/giftcard cents rules and CSV bulk-generate byte layout
  (CSV bytes unchanged); `limit_plan_ids` array shape flips.

### W11 — Admin commerce (plans, payments, orders, reconciliation)

- Routes: §6.2 + §6.4.
- Historical files at landing: the now-retired
  `crates/domain/src/admin/commerce.rs` + `commerce/*.rs`;
  `endpoints/admin.ts`; `apps/admin/src/pages/{plan,payment,order}*`. Current
  backend owners are `application/src/{admin_order,plan,payment,reconciliation}.rs`,
  their `db` repositories, `order-adapters`/`payment-adapters`, and the API
  inbound adapters.
- Fixtures/scenarios: `runners/admin/{plan,payment,order}.mjs`,
  `specs/admin-{plan,payment,order}.spec.mjs`.
- Risk: order identifier switch to `trade_no`; payment present-but-empty
  clear convention formally becomes §4.4 double-Option.

### W12 — Admin users (+ mail, CSV, DSL flagship consumer)

- Routes: §6.6; the users list adopts the §7 DSL (module shipped in W9)
  with the largest column whitelist.
- Historical files at landing: the now-retired
  `crates/domain/src/admin/{users,support/filters}.rs`; `endpoints/admin.ts`;
  `apps/admin/src/pages/user*`. Current backend owners are
  `application/src/admin_user.rs`, `db/src/admin_user.rs`, and
  `api/src/admin_user_adapters.rs` + `api/src/admin/users.rs`.
- Fixtures/scenarios: `runners/admin/user.mjs`, `specs/admin-user.spec.mjs`.
- Risk: filter DSL correctness against the column whitelists;
  `Idempotency-Key` mail replay; CSV export path.

### W13 — Admin servers

- Routes: §6.7.
- Historical files at landing: the now-retired
  `crates/domain/src/admin/{servers,support/server}.rs`; `endpoints/admin.ts`;
  `apps/admin/src/pages/server*`. Current backend owners are
  `application/src/server_management.rs`, `db/src/admin_server.rs`,
  `server-adapters`, and the API server-management inbound adapters.
- Fixtures/scenarios: `runners/admin/server.mjs`,
  `specs/admin-server.spec.mjs`.
- Risk: eight protocol payload matrices; `param_present` → double-Option
  parity per field; vmess camelCase keys deliberately unchanged (R22).

### W14 — Admin tickets, stats, staff namespace, teardown

- Routes: §6.5 (admin tickets), §6.8 (stats, including the series/cents
  re-spec), §6.9 (staff).
- Teardown (same series): delete `legacy_data`/`legacy_page` and the legacy
  `ApiError` constructors from internal paths; delete
  `unwrapBackendEnvelope`'s in-body `code` path, `envelopeSchema`/
  `pageEnvelopeSchema`, `adminFilterSchema`, and the form serializer from
  the api-client; delete legacy fixture emitters for migrated families
  (oracle world keeps legacy emitters — it is the reference); final
  registry/doc sweep of §3.4. The response-rewrite localization middleware
  and the zh-CN catalog are **retained** for the §2 external namespaces
  (§3.1) — problem+json bodies have no `message` key, so the middleware is
  a natural no-op on every internal response; what W14 deletes is only the
  catalog entries no external route can emit. Deleting the middleware
  itself would change frozen external error bytes and is forbidden.
- Fixtures/scenarios: `runners/admin/ticket.mjs` (drives the **admin-prefix**
  ticket routes plus the staff mirrors — not `/api/v1/staff/*` alone),
  `specs/admin-ticket.spec.mjs`, admin stats scenarios (series-slug/cents
  reshape), `_coverage.spec.mjs` route-coverage audit,
  `make parity-config-audit` extended to the route map (which includes the
  §6.5 admin ticket rows).
- Risk: teardown must grep-verify no internal caller still constructs
  legacy shapes; the stats reshape touches every admin chart widget.

---

## Appendix B — AGENTS.md contract-line amendment map

This design wave's declared deliverable is this spec plus an AGENTS.md
amendment. AGENTS.md's contract lines describe the live legacy dialect and
stay authoritative for each family until its wave lands; the umbrella
amendment shipped with this design wave (the "Internal API Dialect
Direction" section plus the authoritative-until-a-wave-lands rule under
"Frontend Contract Direction") anchors the target contract to this
document. Each wave below rewords the listed line(s) in the same commit
series that flips its family, so `make behavior-parity` obligations and
this spec never pull in opposite directions:

| AGENTS.md line (today) | Replacement wording (when the wave lands) | Wave |
| --- | --- | --- |
| Tier-1 list: "hash route paths (the backend emails links into them, e.g. `?verify=`)" | history route paths plus the `legacy_hash_redirect_enable` translator; backend-minted URLs are path-style (`?verify=`/`?redirect=` query names unchanged) | W1 |
| Admin Surface Direction: "including form-encoded array shapes like `limit_plan_ids[0]`" | JSON bodies with real arrays (§4.1) and the §7 filter DSL | W10 (`limit_plan_ids`) / W12 (filter DSL) |
| User Commerce Direction: "unfinished-order and order cancellation payloads (`{trade_no}`)" | `trade_no` moves into the path (`POST /user/orders/{trade_no}/cancel`) | W4 (user) / W11 (admin orders) |
| User Profile Direction: "the reset-subscribe token rotation (`/user/resetSecurity`)" | `POST /user/subscription/reset-token` (§5.4) | W5 |
| User Invite Direction: "the `/user/invite/save` call" | `POST /user/invite-codes` (§5.6) | W7 |
| Pre-Release MySQL Import Direction | unchanged — the section never mentions `custom_html`, and the importer has no `frontend_custom_html` mapping (§12) | n/a |

The Tier-1 hash-route line and the Auth Surface route line were already
pre-reworded to transitional forms in this design wave's amendment ("hash
today, history routing per `docs/api-dialect.md`"); W1 drops the
transitional qualifier when history routing lands.

Lines that pin outcomes without quoting a concrete legacy shape ("save-order
payloads", "coupon checks", "reply / create-ticket / close-ticket payloads",
…) need no rewording: after a family's wave, the payload they pin is the §5–6
modern one, still guarded by the same behavior/interaction scenarios.

---

## Review dispositions (adversarial review round, 2026-07-17)

Three adversarial reviews produced the blockers, majors, and minors resolved
in this revision. Summary:

- **Blockers (all accepted).** Admin ticket family added as §6.5 and owned
  by W14; the dispatch-arm re-enumeration this forced also surfaced the
  `order/update` `reconciliation_id` demultiplex, now
  `POST payment-reconciliations/{id}/resolve` (§6.4). The §10.5 CSP gained
  the reCAPTCHA loader hosts (`recaptcha.net`/`gstatic.com`) and the
  completed Stripe host set (`m.stripe.network`, telemetry). The W14
  teardown no longer deletes the response-rewrite localization middleware:
  it is retained for the §2 external namespaces with the explicit
  no-`message`-key pass-through invariant (§3.1).
- **Majors (all accepted).** Reserved-segment admin-path validation
  (§10.2/§12, W1); one-status-per-code pinning with the
  `user_not_registered`/`plan_unavailable`/`gift_card_not_found` splits and
  `register_ip_rate_limited` at 429 (§3.3/§3.4); success statuses pinned in
  §1 — creates return **201 with `{id}`/`{trade_no}`** rather than the
  legacy-parity 204+refetch (recorded decision: the created id feeds the
  follow-up `PATCH …/{id}` without a racy list refetch; the one deliberate
  204 create is `/user/invite-codes`); dynamic-prefix method routing via
  fallback re-dispatch across all methods (§6/§10.2/§14); structural
  step-up middleware for all admin/staff mutations plus recorded decisions
  on the two POST→GET reads that leave the gate (§6); the Bearer scheme
  flip pinned as a W2 cross-cutting atomic (§4.2); user notices corrected
  to the paginated anchor with `per_page` default 5 and the `?id=` branch
  dropped (§5.8); admin notices corrected to the unpaginated bare array
  (§6.3); admin stats re-specified to integer cents + snake_case series
  slugs with the §4.1 claim corrected (§6.8); the page-location
  canonicalizer added as the fourth adapter with the `parity-config-audit`
  validator rework in W1 (§13.4); the filter DSL module moved to W9 with a
  complete per-endpoint whitelist table (§7.1, Appendix A); the deposit
  order flow made an explicit discriminated union owned by W4 (§5.5); and
  the AGENTS.md amendment map added (Appendix B).
- **Minors (all accepted; none rejected).** `WWW-Authenticate` on every 401
  (§3.2); `invalid_token` pinned to 400 in both §3.4 and §5.2; `like`
  wildcard escaping as a recorded divergence from the oracle plus the
  fixture rule (§7.1, §13.2); body-borne `filter` defined as the raw clause
  array (§7.1); `PATCH orders/{trade_no}` exactly-one-field rule (§6.4);
  config activation-pending reclassified from 503 to **202 Accepted**
  (§6.1) — chosen over keeping 503 because the write is durable and a 503
  invites resubmits that then 409; W5 route ownership de-duplicated
  (Appendix A); §12 importer wording corrected to match reality (no
  mapping exists); the relative payment-return mint branch covered
  (§10.4); `ENABLED_LOCALES` named as the single locale anchor (§4.3,
  §11); and `build-deploy.mjs` marker-assertion/per-app pre-paint hash
  handling added to W1 (§10.5).
- No finding contradicted the fixed owner decisions, so none was rejected
  on that ground; where a finding offered alternatives (201 vs 204 creates,
  202 vs 503 activation, escape-vs-passthrough `like`, DSL wave placement,
  reconciliation params), the choice made and its rationale are recorded at
  the cited section.
