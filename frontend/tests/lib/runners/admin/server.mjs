import {
  adminServerNodeDrawerState,
  openAdminNodeAddMenu,
  selectAdminNodeGroupDefault,
  adminNodeGroupDefaultSelected,
  clickVisibleAdminNodeType,
  closeVisibleAdminServerDrawers,
  openAdminServerNodeDrawerForType,
  closeAdminServerNodeDrawer,
  reloadAdminServerManagePage,
  openAdminNodeRowEditor,
  adminServerRouteModalState,
  openAdminInlineRowEditor,
  clickAdminOrderRowAction,
  adminServerGroupModalState,
} from '../../state-readers/admin.mjs';
import {
  waitForVisibleText,
  clickFirstVisibleText,
  clickFirstVisibleTextStable,
  fillVisibleAt,
  visibleCount,
  selectLegacyFormOption,
  waitForPagePropertyAtLeast,
  waitForVisibleElementsHidden,
  clickFirstVisible,
  openLegacySelectByLabel,
} from '../../dom-helpers.mjs';
import {
  adminMenuItemSelector,
  adminDrawerOpenSelector,
  adminDrawerTitleSelector,
  adminDrawerInputSelector,
  adminFormLabelSelector,
  adminNodeSubmitSelector,
  adminDialogOpenSelector,
  adminDrawerTextareaSelector,
  adminOverlayOpenSelector,
  adminSelectOptionSelector,
  adminSelectDropdownSelector,
  adminServerGroupSubmitSelector,
  adminServerRouteSubmitSelector,
} from '../../selectors.mjs';
import { jsonIncludes, clonePageRequests } from '../../json-util.mjs';

async function selectAdminNodeFieldOption(page, testId, legacyLabel, optionText) {
  const trigger = page.locator(`[data-testid="${testId}"]`).first();
  if ((await trigger.count()) > 0) {
    await trigger.click();
    await page
      .locator('[data-slot="select-content"] [data-slot="select-item"]')
      .filter({ hasText: optionText })
      .first()
      .click();
    await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
    return;
  }
  await selectLegacyFormOption(page, '.ant-drawer-open', legacyLabel, [optionText]);
}

export async function runAdminServerCreateNodeDrawerInteraction(page) {
  const before = await adminServerNodeDrawerState(page);
  await openAdminNodeAddMenu(page);
  await waitForVisibleText(page, adminMenuItemSelector,'Shadowsocks');
  const menuOpened = await adminServerNodeDrawerState(page);
  await clickVisibleAdminNodeType(page, 'Shadowsocks');
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建节点');
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Node');
  await fillVisibleAt(page, adminDrawerInputSelector, 1, '1.5');
  await page.waitForTimeout(100);
  const drawerOpened = await adminServerNodeDrawerState(page);
  await page.mouse.move(1, 1);
  await page.waitForTimeout(150);
  await selectAdminNodeGroupDefault(page);
  await page.waitForTimeout(150);
  const groupSelected = await adminServerNodeDrawerState(page);
  const groupDefaultSelected = await adminNodeGroupDefaultSelected(page);
  await closeVisibleAdminServerDrawers(page);
  await page.mouse.click(1, 1);
  await page.waitForTimeout(150);
  const closed = {
    openDrawerCount: await visibleCount(page, adminDrawerOpenSelector),
  };
  return { before, closed, drawerOpened, groupDefaultSelected, groupSelected, menuOpened };
}

export async function runAdminServerVlessRealityMatrixInteraction(page) {
  const initialNodeFetchCount = page.__visualParityAdminServerNodeFetchCount ?? 0;
  const before = await adminServerNodeDrawerState(page);
  await openAdminNodeAddMenu(page);
  await waitForVisibleText(page, adminMenuItemSelector,'VLess');
  const menuOpened = await adminServerNodeDrawerState(page);
  await clickVisibleAdminNodeType(page, 'VLess');
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建节点');
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity VLess Reality');
  await fillVisibleAt(page, adminDrawerInputSelector, 1, '3.5');
  await fillVisibleAt(page, adminDrawerInputSelector, 2, 'vless.example.test');
  await fillVisibleAt(page, adminDrawerInputSelector, 3, '443');
  await fillVisibleAt(page, adminDrawerInputSelector, 4, '10443');
  await selectAdminNodeGroupDefault(page);
  const opened = await adminServerVlessMatrixState(page);
  await selectAdminNodeFieldOption(page, 'node-vless-security', '安全性', 'Reality');
  await selectAdminNodeFieldOption(page, 'node-network', '传输协议', 'TCP');
  await waitForVisibleText(page, adminFormLabelSelector, 'XTLS流控算法');
  await selectAdminNodeFieldOption(
    page,
    'node-flow',
    'XTLS流控算法',
    'xtls-rprx-vision',
  );
  const realityTcp = await adminServerVlessMatrixState(page);
  await clickFirstVisible(page, adminNodeSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerNodeSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerNodeFetchCount',
    initialNodeFetchCount + 1,
  );
  return {
    before,
    menuOpened,
    nodeFetchDelta:
      (page.__visualParityAdminServerNodeFetchCount ?? 0) - initialNodeFetchCount,
    opened,
    realityTcp,
    saveRequests: (page.__visualParityAdminServerNodeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function adminServerVlessMatrixState(page) {
  const state = await adminServerNodeDrawerState(page);
  const selectedValues = ['Reality', 'TCP', 'xtls-rprx-vision'].filter((value) =>
    jsonIncludes(state.selectedValues, value),
  );
  if (await adminNodeGroupDefaultSelected(page)) selectedValues.unshift('Default');
  return {
    actionButtons: state.actionButtons,
    drawerCount: state.drawerCount,
    inputValues: state.inputValues.filter(Boolean),
    labels: state.labels,
    selectedValues,
  };
}

export async function runAdminServerNodeSaveFailureInteraction(page) {
  const initialNodeFetchCount = page.__visualParityAdminServerNodeFetchCount ?? 0;
  const before = await adminServerNodeDrawerState(page);
  await openAdminNodeAddMenu(page);
  await waitForVisibleText(page, adminMenuItemSelector,'VLess');
  const menuOpened = await adminServerNodeDrawerState(page);
  await clickVisibleAdminNodeType(page, 'VLess');
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建节点');
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Failed VLess');
  await fillVisibleAt(page, adminDrawerInputSelector, 1, '2.5');
  await fillVisibleAt(page, adminDrawerInputSelector, 2, 'failed-vless.example.test');
  await fillVisibleAt(page, adminDrawerInputSelector, 3, '443');
  await fillVisibleAt(page, adminDrawerInputSelector, 4, '10443');
  await selectAdminNodeGroupDefault(page);
  await selectAdminNodeFieldOption(page, 'node-network', '传输协议', 'TCP');
  const filled = await adminServerNodeDrawerState(page);
  await clickFirstVisible(page, adminNodeSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerNodeSaveCount', 1);
  await page.waitForTimeout(350);
  const after = await adminServerNodeDrawerState(page);
  return {
    after,
    before,
    filled,
    menuOpened,
    nodeFetchDelta:
      (page.__visualParityAdminServerNodeFetchCount ?? 0) - initialNodeFetchCount,
    saveRequests: clonePageRequests(page.__visualParityAdminServerNodeSaveRequests),
  };
}

export async function runAdminServerProtocolFieldMatrixInteraction(page) {
  const snapshots = {};
  const mark = (step) => page.__visualParityDiagnostics?.push(`protocol matrix: ${step}`);

  {
    mark('open Shadowsocks');
    const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'Shadowsocks');
    mark('Shadowsocks select HTTP obfs');
    await selectLegacyFormOption(page, '.ant-drawer-open', '混淆', ['HTTP']);
    snapshots.shadowsocks = {
      httpObfs: await adminServerNodeDrawerState(page),
      menuOpened,
      opened,
    };
    mark('close Shadowsocks');
    await closeAdminServerNodeDrawer(page);
    await reloadAdminServerManagePage(page);
  }

  {
    mark('open VMess');
    const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'VMess');
    mark('VMess select TLS');
    await selectLegacyFormOption(page, '.ant-drawer-open', 'TLS', ['支持']);
    mark('VMess select transport gRPC');
    await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['gRPC']);
    snapshots.vmess = {
      grpcTls: await adminServerNodeDrawerState(page),
      menuOpened,
      opened,
    };
    mark('close VMess');
    await closeAdminServerNodeDrawer(page);
    await reloadAdminServerManagePage(page);
  }

  {
    mark('open Trojan');
    const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'Trojan');
    await fillVisibleAt(page, adminDrawerInputSelector, 5, 'trojan-sni.example.test');
    mark('Trojan select allow insecure');
    await selectLegacyFormOption(page, '.ant-drawer-open', '允许不安全', ['是']);
    mark('Trojan select WebSocket');
    await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['WebSocket']);
    snapshots.trojan = {
      webSocket: await adminServerNodeDrawerState(page),
      menuOpened,
      opened,
    };
    mark('close Trojan');
    await closeAdminServerNodeDrawer(page);
    await reloadAdminServerManagePage(page);
  }

  {
    mark('open Hysteria');
    const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'Hysteria');
    mark('Hysteria select v2');
    await selectLegacyFormOption(page, '.ant-drawer-open', 'HYSTERIA版本', ['v2']);
    mark('Hysteria select salamander');
    await selectLegacyFormOption(page, '.ant-drawer-open', '混淆方式obfs', ['salamander']);
    snapshots.hysteria = {
      hysteria2: await adminServerNodeDrawerState(page),
      menuOpened,
      opened,
    };
    mark('close Hysteria');
    await closeAdminServerNodeDrawer(page);
    await reloadAdminServerManagePage(page);
  }

  {
    mark('open Tuic');
    const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'Tuic');
    mark('Tuic select disable SNI');
    await selectLegacyFormOption(page, '.ant-drawer-open', '禁用SNI', ['是']);
    mark('Tuic select relay mode');
    await selectLegacyFormOption(page, '.ant-drawer-open', '数据包中继模式', ['quic']);
    mark('Tuic select congestion');
    await selectLegacyFormOption(page, '.ant-drawer-open', '拥塞控制算法', ['bbr']);
    snapshots.tuic = {
      quic: await adminServerNodeDrawerState(page),
      menuOpened,
      opened,
    };
    mark('close Tuic');
    await closeAdminServerNodeDrawer(page);
    await reloadAdminServerManagePage(page);
  }

  {
    mark('open AnyTLS');
    const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'AnyTLS');
    await fillVisibleAt(page, adminDrawerInputSelector, 5, 'anytls-sni.example.test');
    snapshots.anytls = {
      filled: await adminServerNodeDrawerState(page),
      menuOpened,
      opened,
    };
    mark('close AnyTLS');
    await closeAdminServerNodeDrawer(page);
  }

  return snapshots;
}

export async function runAdminServerV2nodeProtocolMatrixInteraction(page) {
  const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'V2node');
  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['Shadowsocks']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['HTTP伪装']);
  const shadowsocks = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['VLess']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '安全性', ['Reality']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['WebSocket']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '加密方式', ['MLKEM768X25519PLUS']);
  const vless = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['Trojan']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '安全性', ['TLS']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['gRPC']);
  const trojan = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['Hysteria2']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '混淆方式obfs', ['salamander']);
  const hysteria2 = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['Tuic']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '数据包中继模式', ['quic']);
  const tuic = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['AnyTLS']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['TCP']);
  const anytls = await adminServerNodeDrawerState(page);
  await closeAdminServerNodeDrawer(page);

  return { anytls, hysteria2, menuOpened, opened, shadowsocks, trojan, tuic, vless };
}

export async function runAdminServerV2nodeSecurityTransportMatrixInteraction(page) {
  const { menuOpened, opened } = await openAdminServerNodeDrawerForType(page, 'V2node');

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['VMess']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '安全性', ['无']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['XHTTP']);
  const vmessNoneXhttp = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '安全性', ['TLS']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['gRPC']);
  const vmessTlsGrpc = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['VLess']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '安全性', ['TLS']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['HTTPUpgrade']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '加密方式', ['MLKEM768X25519PLUS']);
  const vlessTlsHttpUpgrade = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '安全性', ['Reality']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['WebSocket']);
  const vlessRealityWebSocket = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '节点协议', ['Trojan']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '安全性', ['TLS']);
  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['TCP']);
  const trojanTlsTcp = await adminServerNodeDrawerState(page);

  await selectLegacyFormOption(page, '.ant-drawer-open', '传输协议', ['gRPC']);
  const trojanTlsGrpc = await adminServerNodeDrawerState(page);

  await closeAdminServerNodeDrawer(page);
  return {
    menuOpened,
    opened,
    trojanTlsGrpc,
    trojanTlsTcp,
    vlessRealityWebSocket,
    vlessTlsHttpUpgrade,
    vmessNoneXhttp,
    vmessTlsGrpc,
  };
}

export async function runAdminServerEditNodeDrawerInteraction(page) {
  const before = await adminServerNodeDrawerState(page);
  await openAdminNodeRowEditor(page, 'Tokyo 01');
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑节点');
  await page.waitForFunction(
    (inputSelector) => {
      const values = Array.from(document.querySelectorAll(inputSelector)).map(
        (element) => ('value' in element ? element.value : ''),
      );
      return values.includes('Tokyo 01') && values.includes('jp.example.com') && values.includes('8388');
    },
    adminDrawerInputSelector,
    { timeout: 5_000 },
  );
  const opened = await adminServerNodeDrawerState(page);
  const openedGroupSelected = await adminNodeGroupDefaultSelected(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Edited Node');
  await fillVisibleAt(page, adminDrawerInputSelector, 1, '2.25');
  await fillVisibleAt(page, adminDrawerInputSelector, 2, 'edited-node.example.test');
  await fillVisibleAt(page, adminDrawerInputSelector, 3, '9443');
  await fillVisibleAt(page, adminDrawerInputSelector, 4, '18388');
  await page.waitForTimeout(100);
  const edited = await adminServerNodeDrawerState(page);
  await closeVisibleAdminServerDrawers(page);
  const closed = {
    openDrawerCount: await visibleCount(page, adminDrawerOpenSelector),
  };
  return { before, closed, edited, opened, openedGroupSelected };
}

export async function runAdminServerRouteEditModalInteraction(page) {
  const initialRouteFetchCount = page.__visualParityAdminServerRouteFetchCount ?? 0;
  const before = await adminServerRouteModalState(page);
  await openAdminInlineRowEditor(page, 'Block ads', 'server-route-edit-', () =>
    clickAdminOrderRowAction(page, 'Block ads', '编辑'),
  );
  await page.waitForSelector(adminDialogOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑路由');
  await page.waitForFunction(
    (inputSelector) => {
      const values = Array.from(document.querySelectorAll(inputSelector)).map(
        (element) => ('value' in element ? element.value : ''),
      );
      return values.includes('Block ads') && values.some((value) => value.includes('domain:example.com'));
    },
    adminDrawerInputSelector,
    { timeout: 5_000 },
  );
  const opened = await adminServerRouteModalState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Edited Route');
  await fillVisibleAt(
    page,
    adminDrawerTextareaSelector,
    0,
    'domain:edited.example.com\ngeosite:openai',
  );
  await openLegacySelectByLabel(page, adminOverlayOpenSelector, '动作');
  await waitForVisibleText(page, adminSelectOptionSelector, '指定DNS服务器进行解析');
  const actionDropdown = await adminServerRouteModalState(page);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, [
    '指定DNS服务器进行解析',
  ]);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await waitForVisibleText(page, adminFormLabelSelector, 'DNS服务器');
  await fillVisibleAt(page, adminDrawerInputSelector, 2, '1.1.1.1');
  await page.waitForTimeout(100);
  const edited = await adminServerRouteModalState(page);
  await clickFirstVisible(page, adminServerRouteSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerRouteSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerRouteFetchCount',
    initialRouteFetchCount + 1,
  );
  const closed = await adminServerRouteModalState(page);
  return {
    actionDropdown,
    before,
    closed,
    edited,
    opened,
    routeFetchDelta:
      (page.__visualParityAdminServerRouteFetchCount ?? 0) - initialRouteFetchCount,
    saveRequests: clonePageRequests(page.__visualParityAdminServerRouteSaveRequests),
  };
}

export async function runAdminServerRouteCreateModalInteraction(page) {
  const initialRouteFetchCount = page.__visualParityAdminServerRouteFetchCount ?? 0;
  const before = await adminServerRouteModalState(page);
  await clickFirstVisibleText(page, 'button', ['添加路由']);
  await page.waitForSelector(adminDialogOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '创建路由');
  const opened = await adminServerRouteModalState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Created Route');
  await fillVisibleAt(
    page,
    adminDrawerTextareaSelector,
    0,
    'domain:created.example.com\ngeosite:created',
  );
  await openLegacySelectByLabel(page, adminOverlayOpenSelector, '动作');
  await waitForVisibleText(page, adminSelectOptionSelector, '指定DNS服务器进行解析');
  const actionDropdown = await adminServerRouteModalState(page);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, [
    '指定DNS服务器进行解析',
  ]);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await waitForVisibleText(page, adminFormLabelSelector, 'DNS服务器');
  await fillVisibleAt(page, adminDrawerInputSelector, 2, '9.9.9.9');
  await page.waitForTimeout(100);
  const edited = await adminServerRouteModalState(page);
  await clickFirstVisible(page, adminServerRouteSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerRouteSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerRouteFetchCount',
    initialRouteFetchCount + 1,
  );
  const closed = await adminServerRouteModalState(page);
  return {
    actionDropdown,
    before,
    closed,
    edited,
    opened,
    routeFetchDelta:
      (page.__visualParityAdminServerRouteFetchCount ?? 0) - initialRouteFetchCount,
    saveRequests: clonePageRequests(page.__visualParityAdminServerRouteSaveRequests),
  };
}

export async function runAdminServerGroupCreateModalInteraction(page) {
  const initialGroupFetchCount = page.__visualParityAdminServerGroupFetchCount ?? 0;
  const before = await adminServerGroupModalState(page);
  await clickFirstVisibleText(page, 'button', ['添加权限组']);
  await page.waitForSelector(adminDialogOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '创建组');
  const opened = await adminServerGroupModalState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Created Group');
  await page.waitForTimeout(100);
  const edited = await adminServerGroupModalState(page);
  await clickFirstVisible(page, adminServerGroupSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerGroupSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerGroupFetchCount',
    initialGroupFetchCount + 1,
  );
  const closed = await adminServerGroupModalState(page);
  return {
    before,
    closed,
    edited,
    groupFetchDelta:
      (page.__visualParityAdminServerGroupFetchCount ?? 0) - initialGroupFetchCount,
    opened,
    saveRequests: (page.__visualParityAdminServerGroupSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminServerGroupSaveFailureInteraction(page) {
  const initialGroupFetchCount = page.__visualParityAdminServerGroupFetchCount ?? 0;
  const before = await adminServerGroupModalState(page);
  await clickFirstVisibleText(page, 'button', ['添加权限组']);
  await page.waitForSelector(adminDialogOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '创建组');
  const opened = await adminServerGroupModalState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Failed Group');
  await page.waitForTimeout(100);
  const filled = await adminServerGroupModalState(page);
  await clickFirstVisible(page, adminServerGroupSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerGroupSaveCount', 1);
  await page.waitForTimeout(350);
  const after = await adminServerGroupModalState(page);
  return {
    after,
    before,
    filled,
    groupFetchDelta:
      (page.__visualParityAdminServerGroupFetchCount ?? 0) - initialGroupFetchCount,
    opened,
    saveRequests: clonePageRequests(page.__visualParityAdminServerGroupSaveRequests),
  };
}

export async function runAdminServerGroupEditModalInteraction(page) {
  const initialGroupFetchCount = page.__visualParityAdminServerGroupFetchCount ?? 0;
  const before = await adminServerGroupModalState(page);
  await openAdminInlineRowEditor(page, 'Default', 'server-group-edit-', () =>
    clickAdminOrderRowAction(page, 'Default', '编辑'),
  );
  await page.waitForSelector(adminDialogOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑组');
  await page.waitForFunction(
    (inputSelector) =>
      Array.from(document.querySelectorAll(inputSelector)).some(
        (element) => 'value' in element && element.value === 'Default',
      ),
    adminDrawerInputSelector,
    { timeout: 5_000 },
  );
  const opened = await adminServerGroupModalState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Edited Group');
  await page.waitForTimeout(100);
  const edited = await adminServerGroupModalState(page);
  await clickFirstVisible(page, adminServerGroupSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerGroupSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerGroupFetchCount',
    initialGroupFetchCount + 1,
  );
  const closed = await adminServerGroupModalState(page);
  return {
    before,
    closed,
    edited,
    groupFetchDelta:
      (page.__visualParityAdminServerGroupFetchCount ?? 0) - initialGroupFetchCount,
    opened,
    saveRequests: (page.__visualParityAdminServerGroupSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}
