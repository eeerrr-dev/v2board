// Read-side fixture response router (split out of api-fixtures.mjs): resolves
// one intercepted request (URL, method, scenario/interaction knobs) to the
// canonical fixture response object. Consumed by installApiFixtures and the
// read-only reference oracle server; the scenario-conditional fixture
// accessors live at the bottom of the file.
import { adminPath } from './env.mjs';
import {
  entryUrlForDialect,
  routingDialectFor,
} from './dialect/page-location-canonicalizer.mjs';
import {
  modernAdminConfigFixture,
  modernAdminOrderFixture,
  modernAdminPaymentFixture,
  modernAdminPlanFixture,
  modernAdminServerGroupFixture,
  modernAdminServerNodeFixture,
  modernAdminServerRouteFixture,
  modernAdminUserDetailFixture,
  modernAdminUserFixture,
  modernAdminUserTrafficFixture,
  modernCommissionFixture,
  modernCouponFixture,
  modernGiftcardFixture,
  modernInviteFixture,
  modernKnowledgeDetailFixture,
  modernKnowledgeRecordFixture,
  modernKnowledgeSummaryFixture,
  modernNoticeFixture,
  modernOrderFixture,
  modernPaymentMethodFixture,
  modernPlanFixture,
  modernPublicConfigFixture,
  modernQueueStatsFixture,
  modernQueueWorkloadFixture,
  modernServerFixture,
  modernSubscribeFixture,
  modernTicketDetailFixture,
  modernTicketFixture,
  modernTrafficLogFixture,
  modernUserConfigFixture,
  modernUserProfileFixture,
  rfc3339FixtureTime,
} from './modern-fixtures.mjs';
import {
  adminAuditLogFixtures,
  adminConfigFixture,
  adminCouponFixtures,
  adminEmailTemplateFixtures,
  adminGiftcardFixtures,
  adminKnowledgeFixtures,
  adminNoticeFixtures,
  adminOrderFixtures,
  adminOrderStatFixtures,
  adminPaymentFixtures,
  adminPaymentFormFixtures,
  adminPaymentMethodsFixture,
  adminQueueStatsFixture,
  adminQueueWorkloadFixtures,
  adminServerGroupFixtures,
  adminServerNodeFixtures,
  adminServerRankFixtures,
  adminServerRouteFixtures,
  adminStatFixture,
  adminTicketDetailFixture,
  adminTicketFixtures,
  adminUserFixtures,
  adminUserRankFixtures,
  adminUserTrafficFixtures,
  bannedUserInfoFixture,
  couponCheckFixture,
  dashboardResetPackageOrderFixture,
  dashboardResetPackageTradeNo,
  deviceLimitExpiredSubscribeFixture,
  deviceLimitReachedSubscribeFixture,
  expiredSubscriptionFixture,
  expiredTrafficUsedUpSubscribeFixture,
  extremeKnowledgeFixtures,
  guestConfigFixture,
  inviteDetailFixtures,
  inviteFixture,
  knowledgeFixtures,
  longAdminOrderFixtures,
  longAdminServerNodeFixtures,
  longAdminUserFixtures,
  longOrderFixtures,
  longPlanFixtures,
  longTicketDetailFixture,
  longTicketFixtures,
  longUserServerFixtures,
  newPeriodSubscribeFixture,
  noSubscriptionFixture,
  noticeFixtures,
  orderFixtures,
  paymentMethodFixtures,
  planFixtures,
  profileDepositOrderFixture,
  profileDepositTradeNo,
  serverFixtures,
  subscribeFixture,
  ticketDetailFixture,
  ticketFixtures,
  trafficFixtures,
  trafficUsedUpSubscribeFixture,
  userCommConfigFixture,
  userInfoFixture,
} from './fixture-data.mjs';

export function adminFixtureEndpoint(pathname) {
  const prefix = `/api/v1/${adminPath}`;
  return pathname.startsWith(`${prefix}/`) ? pathname.slice(prefix.length) : null;
}

export function readRequestData(request) {
  const raw = request.postData();
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return Object.fromEntries(new URLSearchParams(raw));
  }
}

export function apiFixtureResponse(
  requestUrl,
  isAdminScenario,
  scenario = { label: '' },
  requestData = null,
  interaction = {},
  target = 'oracle',
  method = 'GET',
) {
  const pathname = requestUrl.pathname;
  const adminEndpoint = adminFixtureEndpoint(pathname);
  const body = (data, extra = {}) => ({ code: 200, data, ...extra });
  const error = (message, code = 400) => ({ code, data: null, message });
  const httpError = (message, status = 500) => ({
    code: status,
    data: null,
    httpStatus: status,
    message,
  });
  // Modern-dialect fixtures (docs/api-dialect.md §13.5, flipped for §5.2 auth
  // in W2): bare bodies with real HTTP statuses and problem+json errors. Only
  // the source world requests the migrated paths / receives these shapes.
  const v2Body = (data, httpStatus = 200) => ({ data, dialect: 'v2', httpStatus });
  const v2Empty = () => ({ data: null, dialect: 'v2', httpStatus: 204 });
  const v2Problem = (status, title, code, detail) => ({
    dialect: 'v2',
    httpStatus: status,
    problem: { code, detail, status, title, type: 'about:blank' },
  });
  // GLOBAL FLIP 2 (§3.2): the force-unauthorized knobs keep their legacy
  // canonical meaning — 403 was "session expired" (teardown + redirect), 401
  // was a non-session auth failure (keep token, contain the error). The
  // modern dialect swaps the wire for the same outcomes: teardown is 401
  // problem+json `session_expired`, keep-token is 403 `permission_denied`.
  const unauthorizedFixture = (legacyStatus) => {
    if (target !== 'source') return httpError('auth required', legacyStatus);
    return legacyStatus === 401
      ? v2Problem(403, 'Forbidden', 'permission_denied', 'Permission denied')
      : v2Problem(401, 'Unauthorized', 'session_expired', '未登录或登陆已过期');
  };

  if (
    (scenario.forceUserUnauthorized || interaction.forceUserUnauthorized) &&
    (pathname === '/api/v1/user/info' || pathname === '/api/v1/user/profile')
  ) {
    return unauthorizedFixture(
      interaction.forceUserUnauthorizedStatus ?? scenario.forceUserUnauthorizedStatus ?? 403,
    );
  }

  if ((scenario.forceAdminUnauthorized || interaction.forceAdminUnauthorized) && adminEndpoint) {
    return unauthorizedFixture(
      interaction.forceAdminUnauthorizedStatus ?? scenario.forceAdminUnauthorizedStatus ?? 403,
    );
  }

  // §6.9 staff mirror (W14): the /api/v1/staff prefix survives in both
  // dialects — the source world drives the modern resource rows, the oracle
  // keeps the legacy action spellings. Serve the same ticket fixtures as the
  // admin family so the cross-world mirror comparison sees one contract.
  const staffEndpoint = pathname.startsWith('/api/v1/staff/')
    ? pathname.slice('/api/v1/staff'.length)
    : null;
  if (staffEndpoint) {
    if (staffEndpoint === '/tickets' && method === 'GET') {
      return v2Body({
        items: adminTicketFixtures.map(modernTicketFixture),
        total: adminTicketFixtures.length,
      });
    }
    const staffTicketMatch = /^\/tickets\/(\d+)$/.exec(staffEndpoint);
    if (staffTicketMatch && method === 'GET') {
      return v2Body(
        modernTicketDetailFixture(
          adminTicketFixtures.find((item) => String(item.id) === staffTicketMatch[1]) ??
            adminTicketFixtures[0],
        ),
      );
    }
    if (/^\/tickets\/\d+\/(replies|close)$/.test(staffEndpoint)) {
      return v2Empty();
    }
    switch (staffEndpoint) {
      case '/ticket/fetch':
        if (requestUrl.searchParams.has('id')) {
          const requestedId = requestUrl.searchParams.get('id');
          return body(
            adminTicketFixtures.find((item) => String(item.id) === requestedId) ??
              adminTicketFixtures[0],
          );
        }
        return body(adminTicketFixtures, { total: adminTicketFixtures.length });
      case '/ticket/reply':
      case '/ticket/close':
        return body(true);
      default:
        return body(null);
    }
  }

  if (adminEndpoint) {
    if (
      scenario.adminOrdersHttpError &&
      (adminEndpoint === '/order/fetch' || (adminEndpoint === '/orders' && method === 'GET'))
    ) {
      // W11 (§6.4): the source world speaks dialect v2, so a list failure is a
      // problem+json 500; the frozen oracle keeps the legacy HTTP-500 body.
      return target === 'source'
        ? v2Problem(500, 'Internal Server Error', 'internal_error', 'Server Error')
        : httpError('Server Error', 500);
    }
    if (
      scenario.adminUsersHttpError &&
      (adminEndpoint === '/user/fetch' || (adminEndpoint === '/users' && method === 'GET'))
    ) {
      // W12 (§6.6): the source world speaks dialect v2, so a list failure is a
      // problem+json 500; the frozen oracle keeps the legacy HTTP-500 body.
      return target === 'source'
        ? v2Problem(500, 'Internal Server Error', 'internal_error', 'Server Error')
        : httpError('Server Error', 500);
    }
    if (
      /^\/server\/(shadowsocks|vmess|trojan|vless|hysteria|tuic|anytls|v2node)\/save$/.test(
        adminEndpoint,
      )
    ) {
      if (interaction?.adminServerNodeSaveError) return error('节点保存失败');
      return body(true);
    }

    // §6.3 modern admin content family (W10): notices bare unpaginated array,
    // knowledge bare array + /{id} detail, coupons/gift-cards §8 pages,
    // creates 201 {id}, updates/toggles/deletes bodiless. Only the source
    // world requests these spellings (the shared /knowledge/sort path is
    // target-gated); the oracle keeps the legacy rows in the switch below.
    // The error-knob details mirror the legacy fixture toast text — the
    // Tier-1 comparison keys on the problem `code`, presentation drops.
    const contentValidationProblem = (detail) =>
      v2Problem(422, 'Unprocessable Entity', 'validation_failed', detail);
    const isShowOnlyBody =
      requestData != null && Object.keys(requestData).length === 1 && 'show' in requestData;
    if (adminEndpoint === '/notices') {
      if (method === 'POST') {
        if (interaction?.adminNoticeSaveError) return contentValidationProblem('公告保存失败');
        return v2Body({ id: adminNoticeFixtures.length + 1 }, 201);
      }
      return v2Body(adminNoticeFixtures.map(modernNoticeFixture));
    }
    if (/^\/notices\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') {
        if (interaction?.adminNoticeDropError) return contentValidationProblem('公告删除失败');
        return v2Empty();
      }
      if (isShowOnlyBody) {
        if (interaction?.adminNoticeShowError) {
          return contentValidationProblem('公告显示状态保存失败');
        }
        return v2Empty();
      }
      if (interaction?.adminNoticeSaveError) return contentValidationProblem('公告保存失败');
      return v2Empty();
    }
    if (adminEndpoint === '/knowledge-categories') {
      return v2Body(
        Array.from(new Set(adminKnowledgeFixtures.map((knowledge) => knowledge.category))),
      );
    }
    if (adminEndpoint === '/knowledge/sort' && target === 'source') {
      return v2Empty();
    }
    if (adminEndpoint === '/knowledge') {
      if (method === 'POST') {
        if (interaction?.adminKnowledgeSaveError) return contentValidationProblem('知识保存失败');
        return v2Body({ id: adminKnowledgeFixtures.length + 1 }, 201);
      }
      return v2Body(adminKnowledgeFixtures.map(modernKnowledgeSummaryFixture));
    }
    const modernAdminKnowledgeMatch = /^\/knowledge\/(\d+)$/.exec(adminEndpoint);
    if (modernAdminKnowledgeMatch) {
      if (method === 'GET') {
        return v2Body(
          modernKnowledgeDetailFixture(
            adminKnowledgeFixtures.find(
              (knowledge) => String(knowledge.id) === modernAdminKnowledgeMatch[1],
            ) ?? adminKnowledgeFixtures[0],
          ),
        );
      }
      if (method === 'DELETE') return v2Empty();
      if (!isShowOnlyBody && interaction?.adminKnowledgeSaveError) {
        return contentValidationProblem('知识保存失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/coupons') {
      if (method === 'POST') {
        if (interaction?.adminCouponGenerateError) {
          return contentValidationProblem('优惠券生成失败');
        }
        return v2Body({ id: adminCouponFixtures.length + 1 }, 201);
      }
      return v2Body({
        items: adminCouponFixtures.map(modernCouponFixture),
        total: adminCouponFixtures.length,
      });
    }
    if (/^\/coupons\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (!isShowOnlyBody && interaction?.adminCouponGenerateError) {
        return contentValidationProblem('优惠券生成失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/gift-cards') {
      if (method === 'POST') {
        if (interaction?.adminGiftcardGenerateError) {
          return contentValidationProblem('礼品卡生成失败');
        }
        return v2Body({ id: adminGiftcardFixtures.length + 1 }, 201);
      }
      return v2Body({
        items: adminGiftcardFixtures.map(modernGiftcardFixture),
        total: adminGiftcardFixtures.length,
      });
    }
    if (/^\/gift-cards\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (interaction?.adminGiftcardGenerateError) {
        return contentValidationProblem('礼品卡生成失败');
      }
      return v2Empty();
    }

    // §6.2/§6.4 modern admin commerce family (W11): plans/payments bare arrays
    // (prices/fees stay cents/number, booleans, RFC 3339), payment-providers
    // code array + provider form, orders §8 page + trade_no bare detail, 201
    // {id}/{trade_no} creates, and bodiless updates/toggles/deletes. Only the
    // source world requests these spellings; the
    // oracle keeps the legacy rows in the switch below. Error-knob detail text
    // mirrors the legacy toast — the Tier-1 comparison keys on the problem
    // `code`, presentation drops.
    const isSingleFlagBody = (flag) =>
      requestData != null && Object.keys(requestData).length === 1 && flag in requestData;
    if (adminEndpoint === '/plans') {
      if (method === 'POST') {
        if (interaction?.adminPlanSaveError) return contentValidationProblem('订阅保存失败');
        return v2Body({ id: adminPlanFixturesFor(scenario).length + 1 }, 201);
      }
      return v2Body(adminPlanFixturesFor(scenario).map(modernAdminPlanFixture));
    }
    if (adminEndpoint === '/plans/sort') return v2Empty();
    if (/^\/plans\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') {
        if (interaction?.adminPlanDropError) return contentValidationProblem('订阅删除失败');
        return v2Empty();
      }
      if (isSingleFlagBody('show') || isSingleFlagBody('renew')) {
        if (interaction?.adminPlanUpdateError) return contentValidationProblem('订阅开关失败');
        return v2Empty();
      }
      if (interaction?.adminPlanSaveError) return contentValidationProblem('订阅保存失败');
      return v2Empty();
    }
    if (adminEndpoint === '/payments') {
      if (method === 'POST') {
        if (interaction?.adminPaymentSaveError) return contentValidationProblem('支付方式保存失败');
        return v2Body({ id: adminPaymentFixtures.length + 1 }, 201);
      }
      return v2Body(adminPaymentFixtures.map(modernAdminPaymentFixture));
    }
    if (adminEndpoint === '/payments/sort') return v2Empty();
    if (adminEndpoint === '/payment-providers') {
      return v2Body(adminPaymentMethodsFixture);
    }
    const modernProviderFormMatch = /^\/payment-providers\/([^/]+)\/form$/.exec(adminEndpoint);
    if (modernProviderFormMatch) {
      const requestedProvider = decodeURIComponent(modernProviderFormMatch[1]);
      return v2Body(
        adminPaymentFormFixtures[requestedProvider] ?? adminPaymentFormFixtures.AlipayF2F,
      );
    }
    if (/^\/payments\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (isSingleFlagBody('enable')) return v2Empty();
      if (interaction?.adminPaymentSaveError) return contentValidationProblem('支付方式保存失败');
      return v2Empty();
    }
    if (adminEndpoint === '/orders') {
      if (method === 'POST') return v2Body({ trade_no: 'VISUAL2026110099' }, 201);
      return v2Body({
        items: adminOrderFixturesFor(scenario).map(modernAdminOrderFixture),
        total: adminOrderFixturesFor(scenario).length,
      });
    }
    const modernOrderScopedMatch = /^\/orders\/([^/]+)$/.exec(adminEndpoint);
    if (modernOrderScopedMatch) {
      if (method === 'GET') {
        const requestedTradeNo = decodeURIComponent(modernOrderScopedMatch[1]);
        return v2Body(
          modernAdminOrderFixture(
            adminOrderFixturesFor(scenario).find(
              (order) => order.trade_no === requestedTradeNo,
            ) ?? adminOrderFixtures[0],
          ),
        );
      }
      // PATCH status / commission_status
      return v2Empty();
    }
    if (/^\/orders\/[^/]+\/(mark-paid|cancel)$/.test(adminEndpoint)) {
      return v2Empty();
    }

    // §6.6 modern admin users family (W12): the list is an §8 `{items, total}`
    // page over the §7 DSL (RFC 3339 timestamps, `t`/password dropped), the
    // detail is a bare user with the conditional `invite_user` object, a single
    // create returns 201 `{id}` while a bulk run streams the byte-frozen
    // credential CSV, the bulk filter actions POST `/users/{export,mail,ban,
    // bulk-delete}`, and the update/toggle/delete/reset-secret/set-inviter carry
    // identity in the path with bodiless 204s. Only the source world requests
    // these spellings; the oracle keeps the legacy rows in the switch below.
    if (adminEndpoint === '/users') {
      if (method === 'POST') {
        if (requestData?.generate_count) {
          return {
            contentType: 'text/csv',
            httpStatus: 200,
            rawBody: 'email,password\nparity.created@example.com,secret123\n',
          };
        }
        return v2Body({ id: adminUserFixturesFor(scenario).length + 1 }, 201);
      }
      return v2Body({
        items: adminUserFixturesFor(scenario).map(modernAdminUserFixture),
        total: adminUserFixturesFor(scenario).length,
      });
    }
    if (adminEndpoint === '/users/export') {
      return {
        contentType: 'text/csv',
        httpStatus: 200,
        rawBody: 'id,email\n1,visual-user@example.com\n',
      };
    }
    if (adminEndpoint === '/users/mail') {
      if (requestData?.subject === interaction?.adminUserSendMailFailureSubject) {
        return v2Problem(500, 'Internal Server Error', 'internal_error', '邮件加入队列失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/users/ban') {
      if (interaction?.adminUserBanError) {
        return v2Problem(500, 'Internal Server Error', 'internal_error', '用户封禁失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/users/bulk-delete') {
      if (interaction?.adminUserAllDeleteError) {
        return v2Problem(500, 'Internal Server Error', 'internal_error', '用户批量删除失败');
      }
      return v2Empty();
    }
    const modernUserDetailMatch = /^\/users\/(\d+)$/.exec(adminEndpoint);
    if (modernUserDetailMatch) {
      if (method === 'GET') {
        const requestedId = Number(modernUserDetailMatch[1]);
        const users = adminUserFixturesFor(scenario);
        return v2Body(
          modernAdminUserDetailFixture(
            users.find((user) => user.id === requestedId) ?? users[0],
            users,
          ),
        );
      }
      if (method === 'DELETE') {
        if (interaction?.adminUserDeleteError) {
          return v2Problem(500, 'Internal Server Error', 'internal_error', '用户删除失败');
        }
        return v2Empty();
      }
      // PATCH update
      if (interaction?.adminUserUpdateError) {
        return v2Problem(422, 'Unprocessable Entity', 'validation_failed', '邮箱格式错误');
      }
      return v2Empty();
    }
    if (/^\/users\/\d+\/(set-inviter|reset-secret)$/.test(adminEndpoint)) {
      return v2Empty();
    }

    // §6.5 modern admin tickets family (W14): the list is an §8 page with no
    // message stub, the detail is bare + the message[] thread, replies/close
    // are path-identity bodiless 204s. §6.8 stats: the three legacy summary
    // aliases collapse onto one bare integer-cent object (same field names),
    // the ranks take ?window=, orders/records are {series, date, value} slug
    // rows (the seeded arrays are empty), and user-traffic is an §8 page with
    // RFC 3339 record_at. Only the source world requests these spellings; the
    // oracle keeps the legacy rows in the switch below.
    if (adminEndpoint === '/tickets') {
      return v2Body({
        items: adminTicketFixtures.map(modernTicketFixture),
        total: adminTicketFixtures.length,
      });
    }
    const modernAdminTicketMatch = /^\/tickets\/(\d+)$/.exec(adminEndpoint);
    if (modernAdminTicketMatch) {
      const ticket =
        scenario.label === 'admin-ticket-detail'
          ? adminTicketDetailFixture
          : (adminTicketFixtures.find(
              (item) => String(item.id) === modernAdminTicketMatch[1],
            ) ?? adminTicketFixtures[0]);
      return v2Body(modernTicketDetailFixture(ticket));
    }
    if (/^\/tickets\/\d+\/(replies|close)$/.test(adminEndpoint)) {
      return v2Empty();
    }
    if (adminEndpoint === '/stats/summary') {
      return v2Body(adminStatFixture);
    }
    if (adminEndpoint === '/stats/orders' || adminEndpoint === '/stats/records') {
      return v2Body(adminOrderStatFixtures);
    }
    if (adminEndpoint === '/stats/server-rank') {
      return v2Body(adminServerRankFixtures);
    }
    if (adminEndpoint === '/stats/user-rank') {
      return v2Body(adminUserRankFixtures);
    }
    if (adminEndpoint === '/stats/user-traffic') {
      return v2Body({
        items: adminUserTrafficFixtures.map(modernAdminUserTrafficFixture),
        total: 25,
      });
    }

    // §6.7 modern admin servers family (W13): the nodes list is a bare typed
    // array (boolean show, numeric rate and id arrays, RFC 3339 timestamps,
    // no is_online), sort keeps its grouped JSON body on POST /nodes/sort,
    // groups/routes are bare arrays with 201 {id} creates and bodiless
    // PATCH/DELETE, and the eight protocol saves POST /servers/{type}
    // (201 {id}) / PATCH /servers/{type}/{id} with the single-key {show}
    // toggle sharing the PATCH row. Only the source world requests these
    // spellings; the oracle keeps the legacy rows in the switch below. The
    // error-knob detail text mirrors the legacy toast — the Tier-1 comparison
    // keys on the problem `code`, presentation drops.
    if (adminEndpoint === '/nodes') {
      return v2Body(adminServerNodeFixturesFor(scenario).map(modernAdminServerNodeFixture));
    }
    if (adminEndpoint === '/nodes/sort') {
      if (interaction?.adminServerSortError) {
        return v2Problem(500, 'Internal Server Error', 'internal_error', '节点排序失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/server-groups') {
      if (method === 'POST') {
        if (interaction?.adminServerGroupSaveError) {
          return contentValidationProblem('权限组保存失败');
        }
        return v2Body({ id: adminServerGroupFixtures.length + 1 }, 201);
      }
      return v2Body(adminServerGroupFixtures.map(modernAdminServerGroupFixture));
    }
    if (/^\/server-groups\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (interaction?.adminServerGroupSaveError) {
        return contentValidationProblem('权限组保存失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/server-routes') {
      if (method === 'POST') return v2Body({ id: adminServerRouteFixtures.length + 1 }, 201);
      return v2Body(adminServerRouteFixtures.map(modernAdminServerRouteFixture));
    }
    if (/^\/server-routes\/\d+$/.test(adminEndpoint)) {
      return v2Empty();
    }
    if (/^\/servers\/[^/]+$/.test(adminEndpoint) && method === 'POST') {
      if (interaction?.adminServerNodeSaveError) return contentValidationProblem('节点保存失败');
      return v2Body({ id: adminServerNodeFixturesFor(scenario).length + 1 }, 201);
    }
    if (/^\/servers\/[^/]+\/\d+\/copy$/.test(adminEndpoint)) {
      return v2Body({ id: adminServerNodeFixturesFor(scenario).length + 1 }, 201);
    }
    if (/^\/servers\/[^/]+\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (!isShowOnlyBody && interaction?.adminServerNodeSaveError) {
        return contentValidationProblem('节点保存失败');
      }
      return v2Empty();
    }

    switch (adminEndpoint) {
      case '/config/fetch':
        return body(adminConfigFixture);
      case '/config/save':
        if (interaction?.adminConfigSaveError) return error('配置保存失败');
        return body(true);
      case '/config/getEmailTemplate':
        return body(adminEmailTemplateFixtures);
      // §6.1 modern config & system family (W9). Only the source world
      // requests these spellings; the oracle keeps the legacy rows above.
      case '/config':
        if (method === 'PATCH') {
          if (interaction?.adminConfigSaveError) {
            return v2Problem(400, 'Bad Request', 'config_validation_failed', '配置保存失败');
          }
          // Full activation is a bodiless 204 (the 202 activation-pending
          // split is a single-process runtime concern, not a fixture path).
          return v2Empty();
        }
        return v2Body(modernAdminConfigFixture(adminConfigFixture));
      case '/email-templates':
        return v2Body(adminEmailTemplateFixtures);
      case '/telegram-webhook':
        return v2Empty();
      case '/test-mail':
        return v2Body({ log: null, sent: true });
      case '/system/queue-stats':
        return v2Body(modernQueueStatsFixture(adminQueueStatsFixture));
      case '/system/queue-workload':
        return v2Body(adminQueueWorkloadFixtures.map(modernQueueWorkloadFixture));
      case '/system/audit-logs':
        // §6.11 (native-only, source world only): the operator audit trail is
        // an §8 {items, total} page. The fixture ignores the §7 filter/§8 page
        // query — the interaction asserts on the captured request, not on a
        // filtered response.
        return v2Body({ items: adminAuditLogFixtures, total: adminAuditLogFixtures.length });
      case '/coupon/fetch':
        return body(adminCouponFixtures, { total: adminCouponFixtures.length });
      case '/coupon/generate':
        if (interaction?.adminCouponGenerateError) return error('优惠券生成失败');
        return body(true);
      case '/giftcard/fetch':
        return body(adminGiftcardFixtures, { total: adminGiftcardFixtures.length });
      case '/giftcard/generate':
        if (interaction?.adminGiftcardGenerateError) return error('礼品卡生成失败');
        return body(true);
      case '/knowledge/fetch':
        return body(
          requestUrl.searchParams.has('id')
            ? (adminKnowledgeFixtures.find(
                (knowledge) => String(knowledge.id) === requestUrl.searchParams.get('id'),
              ) ?? adminKnowledgeFixtures[0])
            : adminKnowledgeFixtures,
        );
      case '/knowledge/getCategory':
        return body(
          Array.from(new Set(adminKnowledgeFixtures.map((knowledge) => knowledge.category))),
        );
      case '/knowledge/save':
        if (interaction?.adminKnowledgeSaveError) return error('知识保存失败');
        return body(true);
      case '/notice/fetch':
        return body(adminNoticeFixtures, { total: adminNoticeFixtures.length });
      case '/notice/save':
        if (interaction?.adminNoticeSaveError) return error('公告保存失败');
        return body(true);
      case '/notice/show':
        if (interaction?.adminNoticeShowError) return error('公告显示状态保存失败');
        return body(true);
      case '/notice/drop':
        if (interaction?.adminNoticeDropError) return error('公告删除失败');
        return body(true);
      case '/stat/getOverride':
        return body(adminStatFixture);
      case '/stat/getOrder':
        return body(adminOrderStatFixtures);
      case '/stat/getServerLastRank':
      case '/stat/getServerTodayRank':
        return body(adminServerRankFixtures);
      case '/stat/getUserLastRank':
      case '/stat/getUserTodayRank':
        return body(adminUserRankFixtures);
      case '/plan/fetch':
        return body(adminPlanFixturesFor(scenario));
      case '/plan/save':
        if (interaction?.adminPlanSaveError) return error('订阅保存失败');
        return body(true);
      case '/plan/update':
        if (interaction?.adminPlanUpdateError) return error('订阅开关失败');
        return body(true);
      case '/plan/drop':
        if (interaction?.adminPlanDropError) return error('订阅删除失败');
        return body(true);
      case '/plan/sort':
        return body(true);
      case '/payment/fetch':
        return body(adminPaymentFixtures);
      case '/payment/save':
        if (interaction?.adminPaymentSaveError) return error('支付方式保存失败');
        return body(true);
      case '/payment/getPaymentMethods':
        return body(adminPaymentMethodsFixture);
      case '/payment/getPaymentForm': {
        const requestedPayment =
          requestData && typeof requestData.payment === 'string'
            ? requestData.payment
            : adminPaymentMethodsFixture[0];
        return body(
          adminPaymentFormFixtures[requestedPayment] ?? adminPaymentFormFixtures.AlipayF2F,
        );
      }
      case '/server/group/fetch':
        return body(adminServerGroupFixtures);
      case '/server/group/save':
        if (interaction?.adminServerGroupSaveError) return error('权限组保存失败');
        return body(true);
      case '/server/route/save':
        return body(true);
      case '/server/manage/getNodes':
        return body(adminServerNodeFixturesFor(scenario));
      case '/server/manage/sort':
        if (interaction?.adminServerSortError) return error('节点排序失败');
        return body(true);
      case '/server/route/fetch':
        return body(adminServerRouteFixtures);
      case '/system/getQueueStats':
        return body(adminQueueStatsFixture);
      case '/system/getQueueWorkload':
        return body(adminQueueWorkloadFixtures);
      case '/order/fetch':
        return body(adminOrderFixturesFor(scenario), {
          total: adminOrderFixturesFor(scenario).length,
        });
      case '/order/detail': {
        const requestedId = requestData?.id == null ? 1 : Number(requestData.id);
        return body(
          adminOrderFixturesFor(scenario).find((order) => order.id === requestedId) ??
            adminOrderFixtures[0],
        );
      }
      case '/order/assign':
        return body('VISUAL2026110099');
      case '/order/paid':
      case '/order/cancel':
      case '/order/update':
        return body(true);
      case '/user/fetch':
        return body(adminUserFixturesFor(scenario), {
          total: adminUserFixturesFor(scenario).length,
        });
      case '/user/update':
        if (interaction?.adminUserUpdateError) return error('邮箱格式错误');
        return body(true);
      case '/user/generate':
        return {
          contentType: 'text/csv',
          httpStatus: 200,
          rawBody: 'email,password\nparity.created@example.com,secret123\n',
        };
      case '/user/delUser':
        if (interaction?.adminUserDeleteError) return error('用户删除失败');
        return body(true);
      case '/user/ban':
        if (interaction?.adminUserBanError) return error('用户封禁失败');
        return body(true);
      case '/user/allDel':
        if (interaction?.adminUserAllDeleteError) return error('用户批量删除失败');
        return body(true);
      case '/user/dumpCSV':
        return {
          contentType: 'text/csv',
          httpStatus: 200,
          rawBody: 'id,email\n1,visual-user@example.com\n',
        };
      case '/user/sendMail':
        if (requestData?.subject === interaction?.adminUserSendMailFailureSubject) {
          return error('邮件加入队列失败');
        }
        return body(true);
      case '/user/getUserInfoById': {
        const requestedId = requestUrl.searchParams.has('id')
          ? Number(requestUrl.searchParams.get('id'))
          : 1;
        return body(
          adminUserFixturesFor(scenario).find((user) => user.id === requestedId) ??
            adminUserFixtures[0],
        );
      }
      case '/stat/getStatUser':
        return body(adminUserTrafficFixtures, { total: 25 });
      case '/ticket/fetch':
        if (requestUrl.searchParams.has('id')) {
          const requestedId = requestUrl.searchParams.get('id') ?? '7';
          const ticket =
            scenario.label === 'admin-ticket-detail'
              ? adminTicketDetailFixture
              : (adminTicketFixtures.find((item) => String(item.id) === requestedId) ??
                adminTicketFixtures[0]);
          return body(ticket);
        }
        return body(adminTicketFixtures, { total: adminTicketFixtures.length });
      case '/ticket/reply':
        return body(true);
      case '/ticket/close':
        return body(true);
      default:
        return body(null);
    }
  }

  // §5.8 modern knowledge detail (W3): /user/knowledge/{id} is source-world
  // only; the oracle keeps the legacy `?id=` branch on /user/knowledge/fetch.
  const knowledgeDetailMatch = /^\/api\/v1\/user\/knowledge\/(\d+)$/.exec(pathname);
  if (knowledgeDetailMatch) {
    return v2Body(
      modernKnowledgeDetailFixture(
        userKnowledgeFixtureById(knowledgeDetailMatch[1], interaction),
      ),
    );
  }

  // §5.5 modern commerce family (W4): path-identity routes are source-world
  // only; the oracle keeps the legacy /user/plan/* + /user/order/* cases below.
  const planDetailMatch = /^\/api\/v1\/user\/plans\/(\d+)$/.exec(pathname);
  if (planDetailMatch) {
    return v2Body(modernPlanFixture(userPlanFixtureById(planDetailMatch[1], scenario)));
  }
  const orderRouteMatch =
    /^\/api\/v1\/user\/orders\/([^/]+)(?:\/(status|cancel|checkout|stripe-intent))?$/.exec(
      pathname,
    );
  if (orderRouteMatch) {
    const [, tradeNo, orderAction] = orderRouteMatch;
    if (!orderAction) {
      return v2Body(modernOrderFixture(userOrderDetailFixtureFor(tradeNo, scenario)));
    }
    if (orderAction === 'status') {
      return v2Body({ status: 0 });
    }
    if (orderAction === 'cancel') {
      return v2Empty();
    }
    if (orderAction === 'stripe-intent') {
      return v2Body({
        public_key: 'pk_test_visual_parity',
        client_secret: 'pi_visual_secret_parity',
        amount: 1000,
        currency: 'cny',
      });
    }
    // checkout — the §9.3 discriminated union.
    if (interaction?.orderCheckoutError) {
      // A gateway pay failure surfaces from Rust as the 500 internal_error
      // problem (the legacy fixture's HTTP-200 `{code: 400}` "支付失败").
      return v2Problem(
        500,
        'Internal Server Error',
        'internal_error',
        '遇到了些问题，我们正在进行处理',
      );
    }
    const checkoutMethodId = Number(requestData?.method_id);
    if (checkoutMethodId === 2) {
      // Unreachable from the modern SPA (Stripe confirms via Payment Element,
      // never the checkout POST); mirror the legacy acknowledgment as settled.
      return v2Body({ kind: 'settled' });
    }
    if (checkoutMethodId === 3) {
      const redirectRoute =
        interaction.checkoutRedirectRoute ?? '/order/VISUAL2026110001?cashier=visual';
      return v2Body({
        kind: 'redirect',
        url: entryUrlForDialect(redirectRoute, routingDialectFor(target)),
      });
    }
    return v2Body({ kind: 'qr_code', payload: 'https://pay.example.test/qr/VISUAL2026110001' });
  }

  // §9.4 modern session revocation (W5): path-identified, 204, idempotent.
  if (/^\/api\/v1\/user\/sessions\/[^/]+$/.test(pathname) && method === 'DELETE') {
    return v2Empty();
  }

  // §5.7 modern user ticket family (W8): path-identity routes are
  // source-world only; the oracle keeps the legacy /user/ticket/* cases
  // below. The error details mirror the legacy fixture toast text — the
  // Tier-1 comparison keys on the problem `code`, presentation drops.
  const ticketRouteMatch = /^\/api\/v1\/user\/tickets(?:\/([^/]+)(?:\/(replies|close))?)?$/.exec(
    pathname,
  );
  if (ticketRouteMatch) {
    const [, ticketId, ticketAction] = ticketRouteMatch;
    if (!ticketId) {
      if (method === 'POST') {
        if (interaction?.ticketSaveError) {
          return v2Problem(422, 'Unprocessable Entity', 'validation_failed', '工单内容不能为空');
        }
        // §5.7: 201 with the created ticket id.
        return v2Body({ id: 10 }, 201);
      }
      return v2Body(userTicketFixturesFor(scenario).map(modernTicketFixture));
    }
    if (!ticketAction) {
      return v2Body(modernTicketDetailFixture(userTicketDetailFixtureFor(scenario)));
    }
    if (ticketAction === 'replies') {
      if (
        interaction?.ticketReplyError ||
        requestData?.message === interaction?.ticketReplyErrorMessage
      ) {
        return v2Problem(400, 'Bad Request', 'ticket_invalid_state', '工单回复失败');
      }
      return v2Empty();
    }
    // close
    if (interaction?.ticketCloseError) {
      return v2Problem(400, 'Bad Request', 'ticket_invalid_state', '工单关闭失败');
    }
    return v2Empty();
  }

  switch (pathname) {
    case '/api/v1/guest/comm/config':
      return body(guestConfigFixture);
    // §5.1 + §5.3 + §5.8 modern public/content family (W3). Only the source
    // world requests these paths; the oracle keeps the legacy cases below.
    case '/api/v1/public/config':
      return v2Body(modernPublicConfigFixture(guestConfigFixture));
    case '/api/v1/public/invite-views':
      return v2Empty();
    case '/api/v1/user/config':
      return v2Body(
        modernUserConfigFixture(
          interaction?.enableTelegramProfile
            ? {
                ...userCommConfigFixture,
                is_telegram: 1,
                telegram_discuss_link: 'https://t.me/visual_discuss',
              }
            : userCommConfigFixture,
        ),
      );
    case '/api/v1/user/notices':
      return v2Body({
        items: noticeFixtures.map(modernNoticeFixture),
        total: noticeFixtures.length,
      });
    case '/api/v1/user/knowledge':
      return v2Body(modernKnowledgeRecordFixture(userKnowledgeFixturesFor(interaction)));
    case '/api/v1/user/knowledge-categories':
      return v2Body(
        Object.keys(userKnowledgeFixturesFor(interaction)).map((category) => ({ category })),
      );
    case '/api/v1/user/telegram-bot':
      return v2Body({ username: 'legacy_bot' });
    // §5.5 modern commerce family (W4). Only the source world requests these
    // paths; the oracle keeps the legacy /user/plan/* + /user/order/* cases.
    case '/api/v1/user/plans':
      return v2Body(userPlanFixturesFor(scenario).map(modernPlanFixture));
    case '/api/v1/user/orders': {
      if (method === 'POST') {
        // §9.2: 201 with the created identity; the union arm picks the
        // scenario-specific trade_no exactly like the legacy sentinel did.
        if (requestData?.kind === 'deposit') {
          return v2Body({ trade_no: profileDepositTradeNo }, 201);
        }
        if (requestData?.period === 'reset_price') {
          return v2Body({ trade_no: dashboardResetPackageTradeNo }, 201);
        }
        return v2Body({ trade_no: 'VISUAL2026110099' }, 201);
      }
      if (scenario.userOrdersHttpError) {
        return v2Problem(
          500,
          'Internal Server Error',
          'internal_error',
          '遇到了些问题，我们正在进行处理',
        );
      }
      return v2Body(userOrderFixturesFor(scenario).map(modernOrderFixture));
    }
    case '/api/v1/user/payment-methods':
      return v2Body(paymentMethodFixtures.map(modernPaymentMethodFixture));
    case '/api/v1/user/coupons/check':
      if (interaction?.couponError) {
        return v2Problem(400, 'Bad Request', 'coupon_invalid', '优惠券无效');
      }
      return v2Body(modernCouponFixture(couponCheckFixture));
    // The Rust wire shape deliberately omits the permanent subscription
    // credential (`token`) from login/token2Login — clients read only
    // auth_data + is_admin and fetch the subscribe URL via /user/getSubscribe.
    // Cross-world safe: the reference bundles never read `data.token` either;
    // they persist only the `authorization` storage key.
    case '/api/v1/passport/auth/login':
      return body({
        auth_data: 'VISUAL_PARITY_TOKEN',
        is_admin: isAdminScenario,
      });
    case '/api/v1/passport/auth/token2Login':
      return body({
        auth_data: 'VISUAL_PARITY_TOKEN',
        is_admin: isAdminScenario,
      });
    case '/api/v1/user/checkLogin':
      return body({
        is_admin: isAdminScenario && !scenario.forceCheckLoginNotAdmin,
        is_login: !(scenario.forceUserUnauthorized || scenario.forceAdminUnauthorized),
      });
    // §5.2 modern auth family (W2). Only the source world requests these
    // paths; the oracle keeps the legacy passport/checkLogin cases above.
    case '/api/v1/auth/login':
    case '/api/v1/auth/token-login':
      return v2Body({ auth_data: 'VISUAL_PARITY_TOKEN', is_admin: isAdminScenario });
    case '/api/v1/auth/register':
      return v2Body({ auth_data: 'VISUAL_PARITY_TOKEN', is_admin: isAdminScenario }, 201);
    case '/api/v1/auth/password-reset':
    case '/api/v1/auth/email-codes':
      return v2Empty();
    case '/api/v1/auth/quick-login-url':
      return v2Body({
        url: 'https://visual.v2board.test/login?verify=VISUAL_VERIFY_TOKEN&redirect=dashboard',
      });
    case '/api/v1/auth/step-up':
      return v2Body({ expires_in: 900, step_up_token: 'VISUAL_STEP_UP_TOKEN' });
    case '/api/v1/auth/session': {
      if (method === 'DELETE') return v2Empty();
      const isLogin = !(scenario.forceUserUnauthorized || scenario.forceAdminUnauthorized);
      const isAdmin = isAdminScenario && !scenario.forceCheckLoginNotAdmin;
      // Mirror the Rust wire (golden auth.session*): `is_admin` appears only
      // on a logged-in admin session.
      return v2Body(isLogin && isAdmin ? { is_admin: true, is_login: true } : { is_login: isLogin });
    }
    case '/api/v1/user/info':
      return body(
        interaction?.telegramBoundProfile
          ? { ...userInfoFixture, telegram_id: 12345 }
          : scenario.bannedUser
            ? bannedUserInfoFixture
            : userInfoFixture,
      );
    // §5.3/§5.4 + §9.1/§9.4 modern profile family (W5). Only the source world
    // requests these paths; the oracle keeps the legacy /user/* cases.
    case '/api/v1/user/profile':
      if (method === 'PATCH') return v2Empty();
      return v2Body(
        modernUserProfileFixture(
          interaction?.telegramBoundProfile
            ? { ...userInfoFixture, telegram_id: 12345 }
            : scenario.bannedUser
              ? bannedUserInfoFixture
              : userInfoFixture,
        ),
      );
    case '/api/v1/user/password':
      return v2Empty();
    case '/api/v1/user/stats':
      return v2Body({ pending_order_count: 2, pending_ticket_count: 3, invited_user_count: 0 });
    case '/api/v1/user/sessions':
      return v2Body([
        {
          current: true,
          ip: '203.0.113.10',
          login_at: rfc3339FixtureTime(1_700_000_000),
          session_id: 'visual-parity-session',
          ua: 'Visual Parity Browser',
        },
      ]);
    case '/api/v1/user/gift-card-redemptions':
      if (interaction?.redeemGiftcardHttpError) {
        return v2Problem(
          500,
          'Internal Server Error',
          'internal_error',
          '遇到了些问题，我们正在进行处理',
        );
      }
      return v2Body({ type: 1, value: 1234 });
    case '/api/v1/user/telegram-binding':
      return v2Empty();
    case '/api/v1/user/subscription':
      return v2Body(modernSubscribeFixture(userSubscribeFixtureFor(scenario, interaction)));
    case '/api/v1/user/subscription/new-period':
      return v2Empty();
    case '/api/v1/user/subscription/reset-token':
      return v2Body({ subscribe_url: 'VISUAL-RESET-UUID' });
    // §5.4 modern service-usage family (W6). Only the source world requests
    // these paths; the oracle keeps the legacy /user/server/fetch +
    // /user/stat/getTrafficLog cases.
    case '/api/v1/user/servers':
      if (scenario.userServersHttpError) {
        return v2Problem(
          500,
          'Internal Server Error',
          'internal_error',
          '遇到了些问题，我们正在进行处理',
        );
      }
      return v2Body(userServerFixturesFor(scenario).map(modernServerFixture));
    case '/api/v1/user/traffic-logs':
      return v2Body(trafficFixtures.map(modernTrafficLogFixture));
    // §5.6 modern invite & commission family (W7). Only the source world
    // requests these paths; the oracle keeps the legacy /user/invite/* +
    // /user/transfer cases.
    case '/api/v1/user/invite':
      return v2Body(modernInviteFixture(inviteFixture));
    case '/api/v1/user/commissions':
      return v2Body({
        items: inviteDetailFixtures.map(modernCommissionFixture),
        total: inviteDetailFixtures.length,
      });
    case '/api/v1/user/invite-codes':
      // The one deliberate 204-no-body create (§1/§5.6).
      return v2Empty();
    // §5.7 modern withdrawal-ticket create (W8). Only the source world
    // requests this path; the oracle keeps the legacy /user/ticket/withdraw
    // case below.
    case '/api/v1/user/withdrawal-tickets':
      if (
        interaction?.withdrawError ||
        requestData?.withdraw_account === interaction?.withdrawErrorAccount
      ) {
        return v2Problem(400, 'Bad Request', 'withdraw_method_unsupported', '提现失败');
      }
      return v2Body({ id: 11 }, 201);
    case '/api/v1/user/commission-transfers':
      if (interaction?.transferError) {
        return v2Problem(
          400,
          'Bad Request',
          'insufficient_commission_balance',
          '推广佣金余额不足',
        );
      }
      return v2Empty();
    case '/api/v1/user/update':
      return body(true);
    case '/api/v1/user/redeemgiftcard':
      if (interaction?.redeemGiftcardHttpError) return httpError('Server Error', 500);
      return body(true, { type: 1, value: 1234 });
    case '/api/v1/user/changePassword':
      return body(true);
    case '/api/v1/user/transfer':
      if (interaction?.transferError) return error('余额不足');
      return body(true);
    case '/api/v1/user/resetSecurity':
      return body('VISUAL-RESET-UUID');
    case '/api/v1/user/unbindTelegram':
      return body(true);
    case '/api/v1/user/getSubscribe':
      return body(userSubscribeFixtureFor(scenario, interaction));
    case '/api/v1/user/getStat':
      return body([2, 3, 0]);
    case '/api/v1/user/plan/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? userPlanFixtureById(requestUrl.searchParams.get('id'), scenario)
          : userPlanFixturesFor(scenario),
      );
    case '/api/v1/user/order/save':
      if (requestData?.period === 'deposit') return body(profileDepositTradeNo);
      if (requestData?.period === 'reset_price') return body(dashboardResetPackageTradeNo);
      return body('VISUAL2026110099');
    case '/api/v1/user/newPeriod':
      return body(true);
    case '/api/v1/user/order/fetch':
      if (scenario.userOrdersHttpError) return httpError('Server Error', 500);
      return body(userOrderFixturesFor(scenario));
    case '/api/v1/user/order/detail':
      return body(
        userOrderDetailFixtureFor(requestUrl.searchParams.get('trade_no'), scenario),
      );
    case '/api/v1/user/order/cancel':
      return body(true);
    case '/api/v1/user/order/getPaymentMethod':
      return body(paymentMethodFixtures);
    case '/api/v1/user/order/checkout': {
      if (interaction?.orderCheckoutError) return error('支付失败');
      const methodId = Number(requestData?.method);
      if (methodId === 2) {
        return body('stripe-accepted', { type: 1 });
      }
      if (methodId === 3) {
        // Backend-minted relative payment-return URL: the modern backend
        // mints path-style URLs while the legacy oracle mints `/#/…`
        // (docs/api-dialect.md Appendix A §W1), so emit per world.
        const redirectRoute =
          interaction.checkoutRedirectRoute ?? '/order/VISUAL2026110001?cashier=visual';
        return body(entryUrlForDialect(redirectRoute, routingDialectFor(target)), {
          type: 1,
        });
      }
      return body('https://pay.example.test/qr/VISUAL2026110001', { type: 0 });
    }
    case '/api/v1/user/order/check':
      return body(0);
    case '/api/v1/user/coupon/check':
      if (interaction?.couponError) return error('优惠券无效');
      return body(couponCheckFixture);
    case '/api/v1/user/server/fetch':
      if (scenario.userServersHttpError) return httpError('Server Error', 500);
      return body(userServerFixturesFor(scenario));
    case '/api/v1/user/stat/getTrafficLog':
      return body(trafficFixtures);
    case '/api/v1/user/invite/fetch':
      return body(inviteFixture);
    case '/api/v1/user/invite/details':
      return body(inviteDetailFixtures, { total: inviteDetailFixtures.length });
    case '/api/v1/user/invite/save':
      return body(true);
    case '/api/v1/user/ticket/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? userTicketDetailFixtureFor(scenario)
          : scenario.emptyTickets
            ? []
            : userTicketFixturesFor(scenario),
      );
    case '/api/v1/user/ticket/save':
      if (interaction?.ticketSaveError) return error('工单内容不能为空');
      return body(true);
    case '/api/v1/user/ticket/reply':
      if (
        interaction?.ticketReplyError ||
        requestData?.message === interaction?.ticketReplyErrorMessage
      ) {
        return error('工单回复失败');
      }
      return body(true);
    case '/api/v1/user/ticket/close':
      if (interaction?.ticketCloseError) return error('工单关闭失败');
      return body(true);
    case '/api/v1/user/ticket/withdraw':
      if (
        interaction?.withdrawError ||
        requestData?.withdraw_account === interaction?.withdrawErrorAccount
      ) {
        return error('提现失败');
      }
      return body(true);
    case '/api/v1/user/knowledge/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? userKnowledgeFixtureById(requestUrl.searchParams.get('id'), interaction)
          : userKnowledgeFixturesFor(interaction),
      );
    case '/api/v1/user/notice/fetch':
      return body(noticeFixtures);
    case '/api/v1/user/comm/config':
      return body(
        interaction?.enableTelegramProfile
          ? {
              ...userCommConfigFixture,
              is_telegram: 1,
              telegram_discuss_link: 'https://t.me/visual_discuss',
            }
          : userCommConfigFixture,
      );
    case '/api/v1/user/comm/getStripePublicKey':
      return body('pk_test_visual_parity');
    case '/api/v1/user/order/stripe/intent':
      return body({
        public_key: 'pk_test_visual_parity',
        client_secret: 'pi_visual_secret_parity',
        amount: 1000,
        currency: 'cny',
      });
    case '/api/v1/user/telegram/getBotInfo':
      return body({ username: 'legacy_bot' });
    default:
      return body(null);
  }
}

export function userKnowledgeFixturesFor(interaction = {}) {
  return interaction.extremeKnowledgeContent ? extremeKnowledgeFixtures : knowledgeFixtures;
}

export function userKnowledgeFixtureById(id, interaction = {}) {
  const fixtures = userKnowledgeFixturesFor(interaction);
  const articles = Object.values(fixtures).flat();
  return articles.find((knowledge) => String(knowledge.id) === String(id)) ?? articles[0];
}

export function userPlanFixturesFor(scenario = {}) {
  if (scenario.emptyPlans) return [];
  if (scenario.longData) return longPlanFixtures;
  if (scenario.soldOutPlans) {
    return planFixtures.map((plan) => (plan.id === 2 ? { ...plan, capacity_limit: 0 } : plan));
  }
  return planFixtures;
}

export function userOrderFixturesFor(scenario = {}) {
  if (scenario.emptyOrders) return [];
  if (scenario.longData) return longOrderFixtures;
  return orderFixtures;
}

/** Resolve one order-detail fixture by trade_no (legacy query or W4 path). */
export function userOrderDetailFixtureFor(tradeNo, scenario = {}) {
  if (tradeNo === dashboardResetPackageTradeNo) return dashboardResetPackageOrderFixture;
  if (tradeNo === profileDepositTradeNo) return profileDepositOrderFixture;
  return (
    userOrderFixturesFor(scenario).find((order) => order.trade_no === tradeNo) ??
    orderFixtures.find((order) => order.trade_no === tradeNo) ??
    orderFixtures[0]
  );
}

export function userServerFixturesFor(scenario = {}) {
  if (scenario.emptyServers) return [];
  if (scenario.longData) return longUserServerFixtures;
  return serverFixtures;
}

export function userTicketFixturesFor(scenario = {}) {
  if (scenario.emptyTickets) return [];
  if (scenario.longData) return longTicketFixtures;
  return ticketFixtures;
}

export function userTicketDetailFixtureFor(scenario = {}) {
  if (scenario.longData) return longTicketDetailFixture;
  return ticketDetailFixture;
}

export function adminPlanFixturesFor(scenario = {}) {
  return scenario.longData ? longPlanFixtures : planFixtures;
}

export function adminServerNodeFixturesFor(scenario = {}) {
  return scenario.longData ? longAdminServerNodeFixtures : adminServerNodeFixtures;
}

export function adminOrderFixturesFor(scenario = {}) {
  return scenario.longData ? longAdminOrderFixtures : adminOrderFixtures;
}

export function adminUserFixturesFor(scenario = {}) {
  return scenario.longData ? longAdminUserFixtures : adminUserFixtures;
}

export function userSubscribeFixtureFor(scenario = {}, interaction = {}) {
  if (interaction?.newPeriodSubscribe) return newPeriodSubscribeFixture;
  if (scenario.noSubscription) return noSubscriptionFixture;
  if (scenario.expiredTrafficUsedUp) return expiredTrafficUsedUpSubscribeFixture;
  if (scenario.deviceLimitExpired) return deviceLimitExpiredSubscribeFixture;
  if (scenario.expiredSubscription) return expiredSubscriptionFixture;
  if (scenario.trafficUsedUp) return trafficUsedUpSubscribeFixture;
  if (scenario.deviceLimitReached) return deviceLimitReachedSubscribeFixture;
  return subscribeFixture;
}

export function userPlanFixtureById(id, scenario = {}) {
  const plan =
    userPlanFixturesFor(scenario).find((item) => String(item.id) === String(id)) ?? planFixtures[0];
  if (scenario.nonRenewablePlan) return { ...plan, renew: 0 };
  return plan;
}
