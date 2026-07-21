import assert from 'node:assert/strict';
import { test } from 'node:test';
import { internalApiOperations } from '../../../packages/api-client/src/generated/internal-api.ts';
import { apiFixtureResponse } from '../api-fixture-response.mjs';
import { adminPath } from '../env.mjs';
import { adminServerNodeFixtures } from '../fixture-data.mjs';
import { modernAdminServerNodeFixture } from '../modern-fixtures.mjs';

const representativePathParameters = {
  code: 'AlipayF2F',
  id: '1',
  secure_path: adminPath,
  trade_no: 'VISUAL2026110001',
};

const expectedModernSourceReadOperations = [
  'adminConfigGet',
  'adminCouponsList',
  'adminEmailTemplatesList',
  'adminGiftCardsList',
  'adminKnowledgeCategoriesList',
  'adminKnowledgeGet',
  'adminKnowledgeList',
  'adminNodesList',
  'adminNoticesList',
  'adminOrdersGet',
  'adminOrdersList',
  'adminPaymentProvidersForm',
  'adminPaymentProvidersList',
  'adminPaymentsList',
  'adminPlansList',
  'adminServerGroupsList',
  'adminServerRoutesList',
  'adminStatsOrders',
  'adminStatsRecords',
  'adminStatsServerRank',
  'adminStatsSummary',
  'adminStatsUserRank',
  'adminStatsUserTraffic',
  'adminSystemAuditLogsList',
  'adminSystemQueueStats',
  'adminSystemQueueWorkload',
  'adminTicketsGet',
  'adminTicketsList',
  'adminUsersGet',
  'adminUsersList',
  'authSessionGet',
  'publicConfig',
  'staffTicketsGet',
  'staffTicketsList',
  'userCommissionsList',
  'userConfigGet',
  'userInviteGet',
  'userKnowledgeCategoriesList',
  'userKnowledgeGet',
  'userKnowledgeList',
  'userNoticesList',
  'userOrdersGet',
  'userOrdersList',
  'userOrdersStatus',
  'userPaymentMethodsList',
  'userPlansGet',
  'userPlansList',
  'userProfileGet',
  'userServersList',
  'userSessionsList',
  'userStatsGet',
  'userSubscriptionGet',
  'userTelegramBotGet',
  'userTicketsGet',
  'userTicketsList',
  'userTrafficLogsList',
];

function sourceReadCases() {
  const modernCases = [];
  for (const [operationId, operation] of Object.entries(internalApiOperations)) {
    if (operation.method !== 'GET') continue;
    const path = operation.path.replace(/\{([^}]+)\}/g, (_, name) => {
      const value = representativePathParameters[name];
      assert.notEqual(value, undefined, `${operationId} needs a representative ${name}`);
      return encodeURIComponent(value);
    });
    const isAdmin = operationId.startsWith('admin');
    const scenario = { label: `source-contract-${operationId}` };
    const fixture = apiFixtureResponse(
      new URL(path, 'https://fixture.test'),
      isAdmin,
      scenario,
      null,
      {},
      'source',
      'GET',
    );
    if (fixture.dialect === 'v2') {
      modernCases.push({ fixture, operation, operationId, scenario });
    }
  }
  return modernCases;
}

test('source-world read fixtures satisfy their generated response contracts', () => {
  const cases = sourceReadCases();
  assert.deepEqual(
    cases.map(({ operationId }) => operationId),
    expectedModernSourceReadOperations,
    'the modern source-world GET fixture inventory drifted',
  );

  const failures = [];
  for (const { fixture, operation, operationId, scenario } of cases) {
    assert.equal(fixture.httpStatus, 200, `${operationId} must emit HTTP 200`);

    const result = operation.responseSchema.safeParse(fixture.data);
    if (!result.success) {
      failures.push(`${operationId}/${scenario.label}:\n${result.error.message}`);
    }
  }
  assert.deepEqual(failures, [], `source-world fixture contract drift:\n${failures.join('\n\n')}`);
});

const sourceReadVariants = [
  [
    'adminNodesList',
    `/api/v1/${adminPath}/nodes`,
    true,
    { label: 'admin-server-manage-long-data', longData: true },
  ],
  [
    'userServersList',
    '/api/v1/user/servers',
    false,
    { label: 'user-node-long-data', longData: true },
  ],
];

test('source-world long-data read fixtures satisfy their generated response contracts', () => {
  for (const [operationId, path, isAdmin, scenario] of sourceReadVariants) {
    const fixture = apiFixtureResponse(
      new URL(path, 'https://fixture.test'),
      isAdmin,
      scenario,
      null,
      {},
      'source',
      'GET',
    );
    const result = internalApiOperations[operationId].responseSchema.safeParse(fixture.data);
    assert.equal(
      result.success,
      true,
      `${operationId}/${scenario.label} fixture drifted from its generated contract:\n${
        result.success ? '' : result.error.message
      }`,
    );
  }
});

test('admin node fixture projection covers every protocol union arm', () => {
  const seed = adminServerNodeFixtures[0];
  const nodes = [
    'shadowsocks',
    'vmess',
    'trojan',
    'tuic',
    'hysteria',
    'vless',
    'anytls',
    'v2node',
  ].map((type, index) =>
    modernAdminServerNodeFixture({
      ...seed,
      id: index + 1,
      type,
    }),
  );

  const result = internalApiOperations.adminNodesList.responseSchema.safeParse(nodes);
  assert.equal(
    result.success,
    true,
    `admin node protocol fixture drift:\n${result.success ? '' : result.error.message}`,
  );
});
