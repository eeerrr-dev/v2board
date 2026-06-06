import {
  cloneElement,
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from 'react';
import { App, Divider, Dropdown, Menu, Row, Col, Tooltip } from 'antd';
import type { DropdownProps } from 'antd';
import type { Plan, PlanPeriod } from '@v2board/types';
import {
  useAdminPlans,
  useConfig,
  useDropPlanMutation,
  useSavePlanMutation,
  useServerGroups,
  useSortPlansMutation,
  useUpdatePlanMutation,
} from '@/lib/queries';
import { i18nGet } from '@/lib/errors';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';
import { LegacyDragSort, LegacyMenuIcon } from '@/components/legacy-drag-sort';
import { LegacyButton } from '@/components/legacy-button';
import { LegacyDrawer } from '@/components/legacy-drawer';
import {
  LegacyCheckboxInput,
  LegacyInput,
  LegacyInputGroup,
  LegacyTextArea,
} from '@/components/legacy-input';
import { LegacySelect, type LegacySelectOption } from '@/components/legacy-select';
import {
  LegacyCaretDownIcon,
  LegacyDeleteIcon,
  LegacyEditIcon,
  LegacyInfoCircleIcon,
  LegacyPlusIcon,
  LegacyQuestionCircleIcon,
  LegacyUserIcon,
} from '@/components/legacy-ant-icon';
import {
  LegacyStandaloneTable,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
} from '@/components/legacy-standalone-table';

type EditablePlan = {
  [K in keyof Omit<Plan, 'id'>]?: Plan[K] | string | null;
} & {
  id?: number;
  force_update?: boolean;
};

type SavePlanPayload = EditablePlan;

type LegacyDropdownProps = Omit<DropdownProps, 'popupRender' | 'trigger'> & {
  trigger?: DropdownProps['trigger'] | 'click';
  overlay: ReactNode;
};

const LEGACY_DROPDOWN_CLICK_TRIGGER = 'click' satisfies LegacyDropdownProps['trigger'];
const LEGACY_RESET_TRAFFIC_OPTIONS: LegacySelectOption[] = [
  { value: null, label: '跟随系统设置' },
  { value: 0, label: '每月1号' },
  { value: 1, label: '按月重置' },
  { value: 2, label: '不重置' },
  { value: 3, label: '每年1月1日' },
  { value: 4, label: '按年重置' },
];

function LegacyDropdown({ overlay, trigger, ...props }: LegacyDropdownProps) {
  const nextTrigger = Array.isArray(trigger) ? trigger : trigger ? [trigger] : undefined;

  return <Dropdown {...props} trigger={nextTrigger} popupRender={() => overlay} />;
}

function emptyPlan(): EditablePlan {
  return {
    show: 0,
    name: null,
    transfer_enable: null,
    group_id: undefined,
    month_price: null,
    quarter_price: null,
    half_year_price: null,
    year_price: null,
    two_year_price: null,
    three_year_price: null,
    onetime_price: null,
    reset_price: null,
  } as EditablePlan;
}

function legacyInputValue(value: unknown) {
  return value === null ? undefined : (value as string | number | readonly string[] | undefined);
}

function PlanEditor({
  record,
  children,
  saveLoading,
  currencySymbol,
  groups,
  onSave,
  onLegacyMount,
}: {
  record?: Plan;
  children: ReactElement<{ onClick?: () => void }>;
  saveLoading: boolean;
  currencySymbol?: string;
  groups: Array<{ id: number; name: string }>;
  onSave: (payload: SavePlanPayload) => Promise<unknown>;
  onLegacyMount: () => void;
}) {
  const { message } = App.useApp();
  const [visible, setVisible] = useState(false);
  const [submit, setSubmit] = useState<EditablePlan>(() => ({ ...(record ?? emptyPlan()) }));

  useEffect(() => {
    onLegacyMount();
  }, [onLegacyMount]);

  const change = (key: keyof EditablePlan, value: unknown) => {
    setSubmit((current) => ({ ...current, [key]: value }));
  };

  const priceOnChange = (key: PlanPeriod, value: string) => {
    change(key, value !== '' ? value : null);
  };

  const save = async () => {
    try {
      await onSave({ ...submit });
      setVisible(false);
    } catch (error) {
      if (error instanceof Error) message.error(i18nGet(error.message));
    }
  };

  return (
    <>
      {cloneElement(children, { onClick: () => setVisible(true) })}
      <LegacyDrawer
        id="plan"
        maskClosable
        onClose={() => setVisible(false)}
        title={`${submit.id ? '编辑订阅' : '新建订阅'}`}
        open={visible}
        width="80%"
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">套餐名称</label>
            <LegacyInput
              className="ant-input"
              placeholder="请输入套餐名称"
              value={legacyInputValue(submit.name)}
              onChange={(event) => change('name', event.target.value)}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">套餐描述</label>
            <LegacyTextArea
              className="ant-input"
              rows={4}
              value={legacyInputValue(submit.content)}
              placeholder="请输入套餐描述，支持HTML"
              onChange={(event) => change('content', event.target.value)}
            />
          </div>

          <Divider>
            售价设置{' '}
            <Tooltip title="将金额留空则不会进行出售" placement="top">
              <LegacyInfoCircleIcon />
            </Tooltip>
          </Divider>

          <Row gutter={10}>
            <Col md={4}>
              <PlanInput
                label="月付"
                value={submit.month_price !== null ? submit.month_price : undefined}
                onChange={(value) => priceOnChange('month_price', value)}
              />
            </Col>
            <Col md={4}>
              <PlanInput
                label="季付"
                value={submit.quarter_price !== null ? submit.quarter_price : undefined}
                onChange={(value) => priceOnChange('quarter_price', value)}
              />
            </Col>
            <Col md={4}>
              <PlanInput
                label="半年"
                value={submit.half_year_price !== null ? submit.half_year_price : undefined}
                onChange={(value) => priceOnChange('half_year_price', value)}
              />
            </Col>
            <Col md={4}>
              <PlanInput
                label="年付"
                value={submit.year_price !== null ? submit.year_price : undefined}
                onChange={(value) => priceOnChange('year_price', value)}
              />
            </Col>
            <Col md={4}>
              <PlanInput
                label="两年付"
                value={submit.two_year_price !== null ? submit.two_year_price : undefined}
                onChange={(value) => priceOnChange('two_year_price', value)}
              />
            </Col>
            <Col md={4}>
              <PlanInput
                label="三年付"
                value={submit.three_year_price !== null ? submit.three_year_price : undefined}
                onChange={(value) => priceOnChange('three_year_price', value)}
              />
            </Col>
          </Row>
          <Row gutter={10}>
            <Col md={12}>
              <PlanInput
                label="一次性"
                addonAfter={currencySymbol}
                value={submit.onetime_price !== null ? submit.onetime_price : undefined}
                onChange={(value) => priceOnChange('onetime_price', value)}
              />
            </Col>
            <Col md={12}>
              <PlanInput
                label="重置包"
                addonAfter={currencySymbol}
                value={submit.reset_price !== null ? submit.reset_price : undefined}
                onChange={(value) => priceOnChange('reset_price', value)}
              />
            </Col>
          </Row>

          <Divider />

          <PlanInput
            label="套餐流量"
            addonAfter="GB"
            placeholder="请输入套餐流量"
            value={submit.transfer_enable}
            onChange={(value) => change('transfer_enable', value)}
          />
          <PlanInput
            label="设备数限制"
            placeholder="留空则不限制"
            value={submit.device_limit}
            onChange={(value) => change('device_limit', value)}
          />
          <div className="form-group">
            <label htmlFor="example-text-input-alt">
              权限组 <a ref={legacyHref('javascript:(0);')}>添加权限组</a>
            </label>
            <LegacySelect
              placeholder="请选择权限组"
              style={{ width: '100%' }}
              value={submit.group_id as number | undefined}
              options={groups.map((group) => ({ value: group.id, label: group.name }))}
              onChange={(value) => change('group_id', value)}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">流量重置方式</label>
            <LegacySelect
              placeholder="请选择权限组"
              style={{ width: '100%' }}
              value={submit.reset_traffic_method as number | null | undefined}
              options={LEGACY_RESET_TRAFFIC_OPTIONS}
              onChange={(value) => change('reset_traffic_method', value)}
            />
          </div>
        </div>
        <PlanInput
          label="最大容纳用户量"
          placeholder="留空则不限制"
          value={submit.capacity_limit}
          onChange={(value) => change('capacity_limit', value)}
        />
        <PlanInput
          label="限速"
          addonAfter="Mbps"
          placeholder="留空则不限制"
          value={submit.speed_limit}
          onChange={(value) => change('speed_limit', value)}
        />

        <div className="v2board-drawer-action">
          <div style={{ float: 'left', marginTop: 5 }}>
            <Tooltip title="勾选后变更的流量、限速、权限组将应用到该套餐下的用户" placement="top">
              <label className="ant-checkbox-wrapper">
                <span
                  className={`ant-checkbox${submit.force_update ? ' ant-checkbox-checked' : ''}`}
                >
                  <LegacyCheckboxInput
                    className="ant-checkbox-input"
                    onChange={(event) => change('force_update', event.target.checked)}
                  />
                  <span className="ant-checkbox-inner" />
                </span>
                <span>强制更新到用户</span>
              </label>
            </Tooltip>
          </div>
          <LegacyButton
            className="ant-btn"
            style={{ marginRight: 8 }}
            onClick={() => setVisible(false)}
          >
            取消
          </LegacyButton>
          <LegacyButton
            className={`ant-btn ant-btn-primary${saveLoading ? ' ant-btn-loading' : ''}`}
            onClick={() => !saveLoading && void save()}
          >
            提交
          </LegacyButton>
        </div>
      </LegacyDrawer>
    </>
  );
}

function PlanInput({
  label,
  value,
  placeholder,
  addonAfter,
  onChange,
}: {
  label: string;
  value: unknown;
  placeholder?: string;
  addonAfter?: ReactNode;
  onChange: (value: string) => void;
}) {
  return (
    <div className="form-group">
      <label htmlFor="example-text-input-alt">{label}</label>
      {addonAfter ? (
        <LegacyInputGroup
          addonAfter={addonAfter}
          placeholder={placeholder}
          value={legacyInputValue(value)}
          onChange={(event) => onChange(event.target.value)}
        />
      ) : (
        <LegacyInput
          className="ant-input"
          placeholder={placeholder}
          value={legacyInputValue(value)}
          onChange={(event) => onChange(event.target.value)}
        />
      )}
    </div>
  );
}

export default function PlansPage() {
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const config = useConfig();
  const save = useSavePlanMutation();
  const drop = useDropPlanMutation();
  const update = useUpdatePlanMutation();
  const sort = useSortPlansMutation();
  const { message } = App.useApp();
  const [order, setOrder] = useState<Plan[]>(() => plans.data ?? []);
  const [legacySortLoading, setLegacySortLoading] = useState(false);
  const [contextRecord, setContextRecord] = useState<Plan | undefined>();
  const [contextMenu, setContextMenu] = useState<{ top: number; left: number } | null>(null);
  const orderRef = useRef(order);
  orderRef.current = order;

  useEffect(() => {
    if (plans.data) setOrder(plans.data);
  }, [plans.data]);

  const refetchConfig = config.refetch;
  const refetchGroups = groups.refetch;
  const refetchPlanEditorDependencies = useCallback(() => {
    void refetchConfig();
    void refetchGroups();
  }, [refetchConfig, refetchGroups]);

  const persistSort = (next: Plan[]) => {
    setOrder(next);
    setLegacySortLoading(true);
    sort.mutate(
      next.map((plan) => plan.id),
      {
        onSuccess: () => {
          void plans.refetch().finally(() => {
            setLegacySortLoading(false);
          });
        },
      },
    );
  };

  const sortPlan = (fromIndex: number, toIndex: number) => {
    const next = [...orderRef.current];
    const moved = next[fromIndex];
    if (!moved) return;
    if (fromIndex < toIndex) {
      next.splice(toIndex + 1, 0, moved);
      next.splice(fromIndex, 1);
    } else {
      next.splice(toIndex, 0, moved);
      next.splice(fromIndex + 1, 1);
    }
    persistSort(next);
  };

  const savePlan = async (payload: SavePlanPayload) => {
    await save.mutateAsync(payload);
    void plans.refetch();
  };

  const dropPlan = (id?: number) => {
    if (!id) return;
    drop.mutate(id, {
      onSuccess: () => {
        void plans.refetch();
      },
      onError: (error) => {
        if (error instanceof Error) message.error(i18nGet(error.message));
      },
    });
  };

  const updatePlan = (id: number, key: 'show' | 'renew', value: 0 | 1) => {
    update.mutate(
      { id, key, value },
      {
        onSuccess: () => {
          void plans.refetch();
        },
        onError: (error) => {
          if (error instanceof Error) message.error(i18nGet(error.message));
        },
      },
    );
  };

  const headers: LegacyStandaloneTableHeader[] = [
    { title: '排序' },
    { title: '销售状态' },
    {
      title: (
        <span>
          续费{' '}
          <Tooltip placement="top" title="在订阅停止销售时，已购用户是否可以续费">
            <LegacyQuestionCircleIcon />
          </Tooltip>
        </span>
      ),
    },
    { title: '名称' },
    { title: '统计' },
    { title: '流量' },
    { title: '设备数限制' },
    { title: '月付' },
    { title: '季付' },
    { title: '半年付' },
    { title: '年付' },
    { title: '两年付' },
    { title: '三年付' },
    { title: '一次性' },
    { title: '重置包' },
    { title: '权限组' },
    { title: '操作', alignRight: true, fixedRight: true },
  ];

  const renderPlanSwitch = (checked: 0 | 1, onClick: () => void) => {
    const enabled = Boolean(parseInt(String(checked), 10));
    return (
      <button
        type="button"
        role="switch"
        aria-checked={enabled}
        className={`ant-switch-small ant-switch${enabled ? ' ant-switch-checked' : ''}`}
        onClick={onClick}
      >
        <span className="ant-switch-inner" />
      </button>
    );
  };

  const renderGroupTags = (value: number) => {
    const tags: ReactNode[] = [];
    (groups.data ?? []).map((group) => {
      if (group.id === parseInt(String(value), 10)) {
        tags.push(
          <span key={group.id} className="ant-tag">
            {group.name}
          </span>,
        );
      }
    });
    return tags;
  };

  const renderPlanActions = (record: Plan) => (
    <LegacyDropdown
      trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
      overlay={
        <Menu>
          <Menu.Item key="edit" onContextMenu={(event) => event.stopPropagation()}>
            <PlanEditor
              key={record.id}
              record={record}
              groups={groups.data ?? []}
              currencySymbol={config.data?.site?.currency_symbol}
              saveLoading={save.isPending}
              onSave={savePlan}
              onLegacyMount={refetchPlanEditorDependencies}
            >
              <a>
                <LegacyEditIcon /> 编辑
              </a>
            </PlanEditor>
          </Menu.Item>
          <Menu.Item key="delete" style={{ color: '#ff4d4f' }} onClick={() => dropPlan(record.id)}>
            <LegacyDeleteIcon /> 删除
          </Menu.Item>
        </Menu>
      }
    >
      <a ref={legacyHref()}>
        操作 <LegacyCaretDownIcon />
      </a>
    </LegacyDropdown>
  );

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={plans.isFetching || legacySortLoading}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <PlanEditor
                groups={groups.data ?? []}
                currencySymbol={config.data?.site?.currency_symbol}
                saveLoading={save.isPending}
                onSave={savePlan}
                onLegacyMount={refetchPlanEditorDependencies}
              >
                <LegacyButton className="ant-btn">
                  <LegacyPlusIcon />
                  <span> 添加订阅</span>
                </LegacyButton>
              </PlanEditor>
            </div>
            <LegacyDragSort
              onDragEnd={(fromIndex, toIndex) => sortPlan(fromIndex, toIndex)}
              nodeSelector="tr"
              handleSelector="i"
            >
              <LegacyStandaloneTable
                headers={headers}
                isEmpty={order.length === 0}
                scrollX={1300}
                scrollPositionRight={false}
                fixedRightRowHeight={75}
                fixedRightChildren={order.map((record, index) => (
                  <tr
                    key={index}
                    className="ant-table-row ant-table-row-level-0"
                    style={{ height: 75 }}
                    {...legacyTableRowKey(index)}
                  >
                    <td className="" style={{ textAlign: 'right' }}>
                      {renderPlanActions(record)}
                    </td>
                  </tr>
                ))}
              >
                {order.map((record, index) => (
                  <tr
                    key={index}
                    className="ant-table-row ant-table-row-level-0"
                    onClick={() => setContextMenu(null)}
                    onContextMenu={(event) => {
                      event.preventDefault();
                      setContextRecord(record);
                      setContextMenu({ top: event.clientY, left: event.clientX });
                    }}
                    {...legacyTableRowKey(index)}
                  >
                    <td className="">
                      <LegacyMenuIcon style={{ cursor: 'move' }} />
                    </td>
                    <td className="">
                      {renderPlanSwitch(record.show, () =>
                        updatePlan(record.id, 'show', parseInt(String(record.show), 10) ? 0 : 1),
                      )}
                    </td>
                    <td className="">
                      {renderPlanSwitch(record.renew, () =>
                        updatePlan(record.id, 'renew', parseInt(String(record.renew), 10) ? 0 : 1),
                      )}
                    </td>
                    <td className="">{record.name}</td>
                    <td className="">
                      <LegacyUserIcon style={{ cursor: 'move' }} /> {record.count}
                    </td>
                    <td className="">{record.transfer_enable} GB</td>
                    <td className="">{record.device_limit !== null ? record.device_limit : '-'}</td>
                    <td className="">{formatPrice(record.month_price)}</td>
                    <td className="">{formatPrice(record.quarter_price)}</td>
                    <td className="">{formatPrice(record.half_year_price)}</td>
                    <td className="">{formatPrice(record.year_price)}</td>
                    <td className="">{formatPrice(record.two_year_price)}</td>
                    <td className="">{formatPrice(record.three_year_price)}</td>
                    <td className="">{formatPrice(record.onetime_price)}</td>
                    <td className="">{formatPrice(record.reset_price)}</td>
                    <td className="">{renderGroupTags(record.group_id)}</td>
                    <td className="ant-table-fixed-columns-in-body" style={{ textAlign: 'right' }}>
                      {renderPlanActions(record)}
                    </td>
                  </tr>
                ))}
              </LegacyStandaloneTable>
              <div
                id="v2board-table-dropdown"
                className="ant-dropdown ant-dropdown-placement-bottomLeft"
                style={{
                  display: contextMenu ? 'unset' : 'none',
                  position: 'fixed',
                  top: contextMenu?.top ?? 0,
                  left: contextMenu?.left ?? 0,
                }}
                onClick={() => setContextMenu(null)}
              >
                <ul className="ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical">
                  <li className="ant-dropdown-menu-item">
                    <PlanEditor
                      record={contextRecord}
                      key={contextRecord?.id}
                      groups={groups.data ?? []}
                      currencySymbol={config.data?.site?.currency_symbol}
                      saveLoading={save.isPending}
                      onSave={savePlan}
                      onLegacyMount={refetchPlanEditorDependencies}
                    >
                      <a>
                        <LegacyEditIcon /> 编辑
                      </a>
                    </PlanEditor>
                  </li>
                  <li
                    className="ant-dropdown-menu-item"
                    onClick={() => {
                      setContextMenu(null);
                      dropPlan(contextRecord?.id);
                    }}
                  >
                    <a style={{ color: '#ff4d4f' }}>
                      <LegacyDeleteIcon /> 删除
                    </a>
                  </li>
                </ul>
              </div>
            </LegacyDragSort>
          </div>
        </div>
      </LegacySpin>
    </>
  );
}

function formatPrice(value: number | null) {
  return value !== null ? value.toFixed(2) : '-';
}
