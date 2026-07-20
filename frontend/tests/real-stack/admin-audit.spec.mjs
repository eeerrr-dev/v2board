import { test, expect } from '@playwright/test';

const ADMIN_PATH = process.env.REAL_STACK_E2E_ADMIN_PATH ?? 'admin-e2e';

test('real stack covers auth, config, and normalized plan price writes', async ({ page }) => {
  const serverFailures = [];
  page.on('response', (response) => {
    if (response.status() >= 500) {
      serverFailures.push(`${response.status()} ${response.request().method()} ${response.url()}`);
    }
  });

  // This config has no global parity setup and installs no page.route handler:
  // every relative request below reaches rust-real-stack-api.
  await page.goto(`/${ADMIN_PATH}/login`);
  await expect(page.getByTestId('admin-login-surface')).toBeVisible();

  await page.locator('#admin-login-email').fill('admin@example.com');
  await page.locator('#admin-login-password').fill('12345678');
  const loginResponsePromise = page.waitForResponse((response) => {
    const request = response.request();
    return (
      request.method() === 'POST' &&
      new URL(response.url()).pathname === '/api/v1/auth/login'
    );
  });
  await page.getByTestId('admin-login-submit').click();
  const loginResponse = await loginResponsePromise;
  expect(loginResponse.status()).toBe(200);
  expect(loginResponse.headers()['content-type']).toContain('application/json');
  const login = await loginResponse.json();
  expect(login).toMatchObject({ is_admin: true });
  expect(login.auth_data).toEqual(expect.any(String));
  expect(login.auth_data.length).toBeGreaterThan(32);
  expect(login.auth_data).not.toBe('VISUAL_PARITY_TOKEN');

  await expect(page).toHaveURL(new RegExp(`/${ADMIN_PATH}/dashboard(?:[?#]|$)`));
  expect(await page.evaluate(() => localStorage.getItem('authorization'))).toBe(login.auth_data);

  const auditResponsePromise = page.waitForResponse((response) => {
    const request = response.request();
    return (
      request.method() === 'GET' &&
      new URL(response.url()).pathname === `/api/v1/${ADMIN_PATH}/system/audit-logs`
    );
  });
  await page.goto(`/${ADMIN_PATH}/audit`);
  const auditResponse = await auditResponsePromise;
  expect(auditResponse.status()).toBe(200);
  expect(auditResponse.headers()['content-type']).toContain('application/json');
  expect(await auditResponse.json()).toEqual({ items: [], total: 0 });

  await expect(page.getByTestId('audit-page')).toBeVisible();
  await expect(page.getByTestId('audit-empty')).toBeVisible();
  await expect(page.getByTestId('audit-error')).toHaveCount(0);

  const apiRoot = `/api/v1/${ADMIN_PATH}`;
  const headers = {
    Accept: 'application/json',
    'Accept-Language': 'zh-CN',
    Authorization: `Bearer ${login.auth_data}`,
  };

  // Drive the real config form so this lane covers the explicit draft
  // transaction (dirty-field PATCH + authoritative refetch), not just the
  // backend endpoint from a browser request context.
  const configBefore = await page.request.get(`${apiRoot}/config?group=site`, { headers });
  expect(configBefore.status()).toBe(200);
  const configBeforeJson = await configBefore.json();
  expect(configBeforeJson.site.app_name).toEqual(expect.any(String));
  expect(configBeforeJson.revision).toEqual(expect.any(Number));

  await page.goto(`/${ADMIN_PATH}/config/system`);
  await expect(page.getByTestId('config-page')).toBeVisible();
  const appName = page.getByTestId('config-app_name');
  await expect(appName).toBeVisible();
  await appName.fill('Real Stack E2E');
  const configWritePromise = page.waitForResponse((response) => {
    const request = response.request();
    return (
      request.method() === 'PATCH' &&
      new URL(response.url()).pathname === `${apiRoot}/config`
    );
  });
  await page.getByTestId('config-save').click();
  const configWrite = await configWritePromise;
  expect(configWrite.status()).toBe(204);
  expect(configWrite.request().postDataJSON()).toEqual({
    app_name: 'Real Stack E2E',
    expected_revision: configBeforeJson.revision,
  });
  await expect(page.getByTestId('config-save')).toBeDisabled();
  const configAfter = await page.request.get(`${apiRoot}/config?group=site`, { headers });
  expect(configAfter.status()).toBe(200);
  const configAfterJson = await configAfter.json();
  expect(configAfterJson.site.app_name).toBe('Real Stack E2E');
  expect(configAfterJson.revision).toBeGreaterThan(configBeforeJson.revision);

  const staleConfigWrite = await page.request.patch(`${apiRoot}/config`, {
    data: {
      app_name: 'Stale writer must lose',
      expected_revision: configBeforeJson.revision,
    },
    headers,
  });
  expect(staleConfigWrite.status()).toBe(409);
  expect(staleConfigWrite.headers()['content-type']).toContain('application/problem+json');
  expect(await staleConfigWrite.json()).toMatchObject({ code: 'config_revision_conflict' });
  const configAfterConflict = await page.request.get(`${apiRoot}/config?group=site`, { headers });
  expect(configAfterConflict.status()).toBe(200);
  expect(await configAfterConflict.json()).toMatchObject({
    revision: configAfterJson.revision,
    site: { app_name: 'Real Stack E2E' },
  });

  // Create our own FK target through the server-group surface instead of
  // relying on a seed id or using request-context setup. This makes the lane
  // cover a second real form→api-client→Rust→PostgreSQL write journey before
  // the plan editor consumes the new group.
  await page.goto(`/${ADMIN_PATH}/server/group`);
  await expect(page.getByTestId('server-group-page')).toBeVisible();
  await page.getByTestId('server-group-create').click();
  const groupEditor = page.getByTestId('server-group-editor');
  await expect(groupEditor).toBeVisible();
  await groupEditor.getByTestId('server-group-name').fill('Real Stack Group');
  const groupCreatePromise = page.waitForResponse((response) => {
    const request = response.request();
    return (
      request.method() === 'POST' &&
      new URL(response.url()).pathname === `${apiRoot}/server-groups`
    );
  });
  await groupEditor.getByTestId('server-group-submit').click();
  const groupCreate = await groupCreatePromise;
  expect(groupCreate.status()).toBe(201);
  expect(groupCreate.request().postDataJSON()).toEqual({ name: 'Real Stack Group' });
  const groupId = (await groupCreate.json()).id;
  expect(groupId).toEqual(expect.any(Number));
  await expect(groupEditor).toBeHidden();
  await expect(page.getByTestId('server-groups-table')).toContainText('Real Stack Group');

  await page.goto(`/${ADMIN_PATH}/plan`);
  await expect(page.getByTestId('plans-page')).toBeVisible();
  await expect(page.getByTestId('plan-create')).toBeEnabled();
  await page.getByTestId('plan-create').click();
  const createEditor = page.getByTestId('plan-editor');
  await expect(createEditor).toBeVisible();
  await expect(createEditor.getByTestId('plan-force-update')).toHaveCount(0);
  await createEditor.getByTestId('plan-name').fill('Real Stack Plan');
  await createEditor.getByTestId('plan-transfer-enable').fill('100');
  await createEditor.getByTestId('plan-price-month_price').fill('12.34');
  await createEditor.getByTestId('plan-price-year_price').fill('-56.78');
  await createEditor.getByTestId('plan-group').click();
  await page.getByRole('option', { name: 'Real Stack Group' }).click();
  const planCreatePromise = page.waitForResponse((response) => {
    const request = response.request();
    return (
      request.method() === 'POST' &&
      new URL(response.url()).pathname === `${apiRoot}/plans`
    );
  });
  await createEditor.getByTestId('plan-submit').click();
  const planCreate = await planCreatePromise;
  expect(planCreate.status()).toBe(201);
  expect(planCreate.request().postDataJSON()).toMatchObject({
    group_id: groupId,
    month_price: 1234,
    name: 'Real Stack Plan',
    transfer_enable: 100,
    year_price: -5678,
  });
  expect(planCreate.request().postDataJSON()).not.toHaveProperty('force_update');
  const planId = (await planCreate.json()).id;
  expect(planId).toEqual(expect.any(Number));
  await expect(createEditor).toBeHidden();
  await expect(page.getByTestId('plans-table')).toContainText('Real Stack Plan');

  await page.getByTestId(`plan-edit-${planId}`).click();
  const editEditor = page.getByTestId('plan-editor');
  await expect(editEditor).toBeVisible();
  await expect(editEditor.getByTestId('plan-force-update')).toBeVisible();
  await expect(editEditor.getByTestId('plan-price-month_price')).toHaveValue('12.34');
  await expect(editEditor.getByTestId('plan-price-year_price')).toHaveValue('-56.78');
  await editEditor.getByTestId('plan-price-month_price').fill('');
  await editEditor.getByTestId('plan-price-quarter_price').fill('25.00');
  const planPatchPromise = page.waitForResponse((response) => {
    const request = response.request();
    return (
      request.method() === 'PATCH' &&
      new URL(response.url()).pathname === `${apiRoot}/plans/${planId}`
    );
  });
  await editEditor.getByTestId('plan-submit').click();
  const planPatch = await planPatchPromise;
  expect(planPatch.status()).toBe(204);
  expect(planPatch.request().postDataJSON()).toMatchObject({
    month_price: null,
    quarter_price: 2500,
    year_price: -5678,
  });
  await expect(editEditor).toBeHidden();

  // Toggle a non-price field through the rendered row. This exercises the
  // generated partial PATCH adapter itself, proving that omitted prices retain
  // their stored values instead of relying on a hand-written request shortcut.
  const planRow = page.getByRole('row').filter({ hasText: 'Real Stack Plan' });
  const showToggle = planRow.getByRole('switch').first();
  await expect(showToggle).not.toBeChecked();
  const retainPatchPromise = page.waitForResponse((response) => {
    const request = response.request();
    return (
      request.method() === 'PATCH' &&
      new URL(response.url()).pathname === `${apiRoot}/plans/${planId}`
    );
  });
  await showToggle.click();
  const retainPatch = await retainPatchPromise;
  expect(retainPatch.status()).toBe(204);
  expect(retainPatch.request().postDataJSON()).toEqual({ show: true });
  await expect(showToggle).toBeChecked();

  const plansResponse = await page.request.get(`${apiRoot}/plans`, { headers });
  expect(plansResponse.status()).toBe(200);
  const savedPlan = (await plansResponse.json()).find((plan) => plan.id === planId);
  expect(savedPlan).toMatchObject({
    group_id: groupId,
    month_price: null,
    name: 'Real Stack Plan',
    quarter_price: 2500,
    reset_price: null,
    transfer_enable: 100,
    year_price: -5678,
    show: true,
  });
  expect(serverFailures).toEqual([]);
});
