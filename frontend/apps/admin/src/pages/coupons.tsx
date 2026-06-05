import { useEffect, useState } from 'react';
import { App, Button, DatePicker, Input, Modal, Select, Switch, Table } from 'antd';
import type { TablePaginationConfig, TableProps } from 'antd';
import type { SorterResult } from 'antd/es/table/interface';
import { PlusOutlined } from '@ant-design/icons';
import dayjs, { type Dayjs } from 'dayjs';
import { useLocation } from 'react-router-dom';
import type { Coupon, Giftcard, Plan } from '@v2board/types';
import {
  useAdminCoupons,
  useAdminGiftcards,
  useAdminPlans,
  useDropCouponMutation,
  useDropGiftcardMutation,
  useGenerateCouponMutation,
  useGenerateGiftcardMutation,
  useShowCouponMutation,
} from '@/lib/queries';
import { admin } from '@v2board/api-client';
import { legacyCopyText } from '@/lib/legacy-copy';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';

type AdminPageQuery = admin.AdminPageQuery;

let legacyCouponQuery: AdminPageQuery = { current: 1, pageSize: 10 };
let legacyGiftcardQuery: AdminPageQuery = { current: 1, pageSize: 10 };

type CouponSubmit = Omit<
  Partial<Coupon>,
  'value' | 'limit_use' | 'limit_use_with_user' | 'limit_plan_ids' | 'started_at' | 'ended_at'
> & {
  value?: number | string;
  limit_use?: number | string | null;
  limit_use_with_user?: number | string | null;
  limit_plan_ids?: Array<number | string> | null;
  started_at?: number | string | null;
  ended_at?: number | string | null;
  generate_count?: number | string;
};

type GiftcardSubmit = Omit<
  Partial<Giftcard>,
  'value' | 'plan_id' | 'limit_use' | 'started_at' | 'ended_at'
> & {
  value?: number | string;
  plan_id?: number | string | null;
  limit_use?: number | string | null;
  started_at?: number | string | null;
  ended_at?: number | string | null;
  generate_count?: number | string;
};

const PERIOD_TEXT: Record<string, string> = {
  month_price: '月付',
  quarter_price: '季付',
  half_year_price: '半年付',
  year_price: '年付',
  two_year_price: '两年付',
  three_year_price: '三年付',
  onetime_price: '一次性',
  reset_price: '流量重置包',
};

function legacyDateRange(startedAt?: number | string | null, endedAt?: number | string | null) {
  return `${dayjs(1000 * Number(startedAt)).format('YYYY/MM/DD HH:mm')} ~ ${dayjs(
    1000 * Number(endedAt),
  ).format('YYYY/MM/DD HH:mm')}`;
}

function tableSort(sorter: SorterResult<unknown> | SorterResult<unknown>[]) {
  const current = Array.isArray(sorter) ? sorter[0] : sorter;
  return {
    sort_type: current?.order === 'ascend' ? 'ASC' : 'DESC',
    sort: current?.columnKey == null ? undefined : String(current.columnKey),
  } satisfies AdminPageQuery;
}

function rangeValue(startedAt?: number | string | null, endedAt?: number | string | null) {
  return [
    startedAt ? dayjs(1000 * Number(startedAt)) : null,
    endedAt ? dayjs(1000 * Number(endedAt)) : null,
  ] as [Dayjs | null, Dayjs | null];
}

function legacyGiftcardValueAddon(type: GiftcardSubmit['type']) {
  switch (type) {
    case 1:
      return '¥';
    case 2:
      return '天';
    case 3:
      return 'GB';
    case 4:
      return '';
    case 5:
      return '天';
    default:
      return '';
  }
}

function useCopy() {
  const { message } = App.useApp();
  return (text: string) => {
    legacyCopyText(text);
    message.success('复制成功');
  };
}

function downloadGeneratedCsv(prefix: 'COUPON' | 'GIFTCARD', buffer: unknown) {
  const blob = new Blob([buffer as BlobPart], { type: 'text/plain,charset=UTF-8' });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.style.display = 'none';
  anchor.download = `${prefix} ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`;
  anchor.click();
  window.URL.revokeObjectURL(url);
}

export default function CouponsPage() {
  const location = useLocation();
  if (location.pathname === '/giftcard') return <GiftcardPage />;
  return <CouponPage />;
}

function CouponPage() {
  const copy = useCopy();
  const plans = useAdminPlans();
  const generate = useGenerateCouponMutation();
  const drop = useDropCouponMutation();
  const show = useShowCouponMutation();
  const [query, setQueryState] = useState<AdminPageQuery>(() => legacyCouponQuery);
  const [visible, setVisible] = useState(false);
  const [submit, setSubmit] = useState<CouponSubmit>({ type: 1 });
  const coupons = useAdminCoupons(query);

  const setQuery = (next: AdminPageQuery) => {
    legacyCouponQuery = next;
    setQueryState(next);
  };

  useEffect(() => {
    if (!visible) setSubmit({ type: 1 });
  }, [visible]);

  const modalVisible = () => {
    setVisible((current) => !current);
  };

  const generateCoupon = async () => {
    const payload: CouponSubmit = { ...submit };
    if (payload.type === 1) payload.value = 100 * Number(payload.value);
    const response = await generate.mutateAsync(payload);
    if (payload.generate_count) downloadGeneratedCsv('COUPON', response.buffer);
    await coupons.refetch();
    modalVisible();
  };

  const onRangeChange = (dates: [Dayjs | null, Dayjs | null] | null) => {
    const range = dates as [Dayjs | null, Dayjs | null];
    setSubmit({
      ...submit,
      started_at: range[0] ? range[0].format('X') : null,
      ended_at: range[1] ? range[1].format('X') : null,
    });
  };

  const data = coupons.data?.data ?? [];

  const columns: TableProps<Coupon>['columns'] = [
    {
      title: '#',
      dataIndex: 'id',
      key: 'id',
    },
    {
      title: '启用',
      dataIndex: 'show',
      key: 'show',
      render: (value: 0 | 1, row) => (
        <Switch
          size="small"
          onChange={() =>
            show.mutate(row.id, {
              onSuccess: () => {
                void coupons.refetch();
              },
            })
          }
          checked={value as unknown as boolean}
        />
      ),
    },
    {
      title: '券名称',
      dataIndex: 'name',
      key: 'name',
    },
    {
      title: '类型',
      dataIndex: 'type',
      key: 'type',
      render: (value: Coupon['type']) => (value === 1 ? '金额' : '比例'),
    },
    {
      title: '券码',
      dataIndex: 'code',
      key: 'code',
      render: (value: string) => (
        <span style={{ cursor: 'pointer' }} onClick={() => copy(value)}>
          {value}
        </span>
      ),
    },
    {
      title: '剩余次数',
      dataIndex: 'limit_use',
      key: 'limit_use',
      render: (value: number | null) => (value !== null ? value : '无限'),
    },
    {
      title: '有效期',
      dataIndex: 'started_at',
      key: 'started_at',
      align: 'left',
      render: (_value, row) => legacyDateRange(row.started_at, row.ended_at),
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      align: 'right',
      fixed: 'right',
      render: (_value, row, index) => (
        <div>
          <a
            onClick={() => {
              setSubmit(data[index] as CouponSubmit);
              modalVisible();
            }}
            ref={legacyHref()}
          >
            编辑
          </a>
          <div className="ant-divider ant-divider-vertical" />
          <a
            onClick={() => {
              Modal.confirm({
                title: '警告',
                content: '确定要删除该条项目吗？',
                onOk: () => {
                  void drop.mutateAsync(row.id).then(() => {
                    void coupons.refetch();
                  });
                },
                okText: '确定',
                cancelText: '取消',
              });
            }}
            ref={legacyHref()}
          >
            删除
          </a>
        </div>
      ),
    },
  ];

  return (
    <>
      <LegacySpin loading={coupons.isFetching}>
        <div className="block border-bottom">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <Button onClick={modalVisible}>
                <PlusOutlined /> 添加优惠券
              </Button>
            </div>
            <Table<Coupon>
              tableLayout="auto"
              dataSource={data}
              columns={columns}
              scroll={{ x: 1050 }}
              pagination={{
                current: query.current,
                pageSize: query.pageSize,
                total: coupons.data?.total,
                size: 'small',
                showSizeChanger: true,
                pageSizeOptions: [10, 50, 100, 150],
              }}
              onChange={(pagination, _filters, sorter) => {
                setQuery({
                  ...query,
                  ...pagination,
                  ...tableSort(sorter as SorterResult<unknown>),
                });
              }}
            />
          </div>
        </div>
      </LegacySpin>
      <Modal
        title={`${submit.id ? '编辑优惠券' : '新建优惠券'}`}
        open={visible}
        onCancel={modalVisible}
        onOk={() => {
          void generateCoupon();
        }}
        okText="提交"
        cancelText="取消"
        okButtonProps={{ loading: generate.isPending }}
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">名称</label>
            <Input
              placeholder="请输入优惠券名称"
              value={submit.name}
              onChange={(event) => setSubmit({ ...submit, name: event.target.value })}
            />
          </div>
          {!submit.generate_count ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">自定义优惠券码</label>
              <Input
                placeholder="自定义优惠券码(留空随机生成)"
                value={submit.code}
                onChange={(event) =>
                  setSubmit({ ...submit, code: event.target.value, generate_count: undefined })
                }
              />
            </div>
          ) : null}
          <div className="form-group">
            <label htmlFor="example-text-input-alt">优惠信息</label>
            <Input
              type="number"
              addonBefore={
                <Select
                  style={{ width: 120 }}
                  value={submit.type}
                  onChange={(type) => setSubmit({ ...submit, type })}
                >
                  <Select.Option value={1}>按金额优惠</Select.Option>
                  <Select.Option value={2}>按比例优惠</Select.Option>
                </Select>
              }
              addonAfter={submit.type === 1 ? '¥' : '%'}
              placeholder="请输入值"
              value={submit.value}
              onChange={(event) => setSubmit({ ...submit, value: event.target.value })}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">优惠券有效期</label>
            <DatePicker.RangePicker
              style={{ width: '100%' }}
              showTime={{ format: 'HH:mm' }}
              format="YYYY-MM-DD HH:mm"
              placeholder={['Start Time', 'End Time']}
              value={rangeValue(submit.started_at, submit.ended_at)}
              onChange={onRangeChange}
              onOk={onRangeChange}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">最大使用次数</label>
            <Input
              placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
              value={submit.limit_use as string | number | undefined}
              onChange={(event) => setSubmit({ ...submit, limit_use: event.target.value })}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">每个用户可使用次数</label>
            <Input
              placeholder="限制每个用户可使用次数(为空则不限制)"
              value={submit.limit_use_with_user as string | number | undefined}
              onChange={(event) =>
                setSubmit({ ...submit, limit_use_with_user: event.target.value })
              }
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">指定订阅</label>
            <div>
              <Select
                value={submit.limit_plan_ids || []}
                onChange={(value) => setSubmit({ ...submit, limit_plan_ids: value.length ? value : null })}
                mode="multiple"
                placeholder="限制指定订阅可以使用优惠(为空则不限制)"
                style={{ width: '100%' }}
              >
                {(plans.data ?? []).map((plan) => (
                  <Select.Option key={Math.random()} value={`${plan.id}`}>
                    {plan.name}
                  </Select.Option>
                ))}
              </Select>
            </div>
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">指定周期</label>
            <div>
              <Select
                value={submit.limit_period || []}
                onChange={(value) => setSubmit({ ...submit, limit_period: value.length ? value : null })}
                mode="multiple"
                placeholder="限制指定周期可以使用优惠(为空则不限制)"
                style={{ width: '100%' }}
              >
                {Object.keys(PERIOD_TEXT).map((period) => (
                  <Select.Option key={Math.random()} value={period}>
                    {PERIOD_TEXT[period]}
                  </Select.Option>
                ))}
              </Select>
            </div>
          </div>
          {!submit.code && !submit.id ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">生成数量</label>
              <Input
                placeholder="输入数量批量生成"
                value={submit.generate_count}
                onChange={(event) =>
                  setSubmit({ ...submit, generate_count: event.target.value, code: undefined })
                }
              />
            </div>
          ) : null}
        </div>
      </Modal>
    </>
  );
}

function GiftcardPage() {
  const copy = useCopy();
  const [query, setQueryState] = useState<AdminPageQuery>(() => legacyGiftcardQuery);
  const giftcards = useAdminGiftcards(query);
  const plans = useAdminPlans();
  const generate = useGenerateGiftcardMutation();
  const drop = useDropGiftcardMutation();
  const [visible, setVisible] = useState(false);
  const [submit, setSubmit] = useState<GiftcardSubmit>({ type: 1 });

  const setQuery = (next: AdminPageQuery) => {
    legacyGiftcardQuery = next;
    setQueryState(next);
  };

  useEffect(() => {
    if (!visible) setSubmit({ type: 1 });
  }, [visible]);

  const modalVisible = () => {
    setVisible((current) => !current);
  };

  const planName = (id: number | string | null | undefined) =>
    (plans.data ?? []).find((plan: Plan) => plan.id === id)?.name ?? '-';

  const generateGiftcard = async () => {
    const payload: GiftcardSubmit = { ...submit };
    if (payload.type === 1) payload.value = 100 * Number(payload.value);
    const response = await generate.mutateAsync(payload);
    if (payload.generate_count) downloadGeneratedCsv('GIFTCARD', response.buffer);
    await giftcards.refetch();
    modalVisible();
  };

  const onRangeChange = (dates: [Dayjs | null, Dayjs | null] | null) => {
    const range = dates as [Dayjs | null, Dayjs | null];
    setSubmit({
      ...submit,
      started_at: range[0] ? range[0].format('X') : null,
      ended_at: range[1] ? range[1].format('X') : null,
    });
  };

  const data = giftcards.data?.data ?? [];

  const columns: TableProps<Giftcard>['columns'] = [
    {
      title: '#',
      dataIndex: 'id',
      key: 'id',
    },
    {
      title: '名称',
      dataIndex: 'name',
      key: 'name',
    },
    {
      title: '类型',
      dataIndex: 'type',
      key: 'type',
      render: (value: Giftcard['type']) => {
        switch (value) {
          case 1:
            return '金额';
          case 2:
            return '时长';
          case 3:
            return '流量';
          case 4:
            return '重置';
          case 5:
            return '套餐';
          default:
            return '';
        }
      },
    },
    {
      title: '数值',
      dataIndex: 'value',
      key: 'value',
      render: (value: number, row) => {
        switch (row.type) {
          case 1:
            return `${value.toFixed(2)} ¥`;
          case 2:
            return `${value} 天`;
          case 3:
            return `${value} GB`;
          case 4:
            return '-';
          case 5:
            return `${value} 天`;
          default:
            return value;
        }
      },
    },
    {
      title: '套餐',
      dataIndex: 'plan_id',
      key: 'plan_id',
      render: (value: number | null) => planName(value),
    },
    {
      title: '卡密',
      dataIndex: 'code',
      key: 'code',
      render: (value: string) => (
        <span style={{ cursor: 'pointer' }} onClick={() => copy(value)}>
          {value}
        </span>
      ),
    },
    {
      title: '剩余次数',
      dataIndex: 'limit_use',
      key: 'limit_use',
      render: (value: number | null) => (value !== null ? value : '无限'),
    },
    {
      title: '有效期',
      dataIndex: 'started_at',
      key: 'started_at',
      align: 'left',
      render: (_value, row) => legacyDateRange(row.started_at, row.ended_at),
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      align: 'right',
      fixed: 'right',
      render: (_value, row, index) => (
        <div>
          <a
            onClick={() => {
              setSubmit(data[index] as GiftcardSubmit);
              modalVisible();
            }}
            ref={legacyHref()}
          >
            编辑
          </a>
          <div className="ant-divider ant-divider-vertical" />
          <a
            onClick={() => {
              Modal.confirm({
                title: '警告',
                content: '确定要删除该条项目吗？',
                onOk: () => {
                  void drop.mutateAsync(row.id).then(() => {
                    void giftcards.refetch();
                  });
                },
                okText: '确定',
                cancelText: '取消',
              });
            }}
            ref={legacyHref()}
          >
            删除
          </a>
        </div>
      ),
    },
  ];

  return (
    <>
      <LegacySpin loading={giftcards.isFetching}>
        <div className="block border-bottom">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <Button onClick={modalVisible}>
                <PlusOutlined />添加礼品卡
              </Button>
            </div>
            <Table<Giftcard>
              tableLayout="auto"
              dataSource={data}
              columns={columns}
              scroll={{ x: 1050 }}
              pagination={{
                current: query.current,
                pageSize: query.pageSize,
                total: giftcards.data?.total,
                size: 'small',
                showSizeChanger: true,
                pageSizeOptions: [10, 50, 100, 150],
              }}
              onChange={(pagination: TablePaginationConfig, _filters, sorter) => {
                setQuery({
                  ...query,
                  ...pagination,
                  ...tableSort(sorter as SorterResult<unknown>),
                });
              }}
            />
          </div>
        </div>
      </LegacySpin>
      <Modal
        title={`${submit.id ? '编辑礼品卡' : '新建礼品卡'}`}
        open={visible}
        onCancel={modalVisible}
        onOk={() => {
          void generateGiftcard();
        }}
        okText="提交"
        cancelText="取消"
        okButtonProps={{ loading: generate.isPending }}
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">名称</label>
            <Input
              placeholder="请输入礼品卡名称"
              value={submit.name}
              onChange={(event) => setSubmit({ ...submit, name: event.target.value })}
            />
          </div>
          {!submit.generate_count ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">自定义礼品卡卡密</label>
              <Input
                placeholder="自定义礼品卡卡密(留空随机生成)"
                value={submit.code}
                onChange={(event) =>
                  setSubmit({ ...submit, code: event.target.value, generate_count: undefined })
                }
              />
            </div>
          ) : null}
          <div className="form-group">
            <label htmlFor="example-text-input-alt">礼品卡类型</label>
            <Input
              type="number"
              addonBefore={
                <Select
                  style={{ width: 140 }}
                  value={submit.type}
                  onChange={(type) => setSubmit({ ...submit, type })}
                >
                  <Select.Option value={1}>增加账户余额</Select.Option>
                  <Select.Option value={2}>增加订阅时长</Select.Option>
                  <Select.Option value={3}>增加套餐流量</Select.Option>
                  <Select.Option value={4}>重置套餐流量</Select.Option>
                  <Select.Option value={5}>兑换订阅套餐</Select.Option>
                </Select>
              }
              addonAfter={legacyGiftcardValueAddon(submit.type)}
              disabled={submit.type === 4}
              placeholder={submit.type === 5 ? '一次性套餐输入0' : '请输入值'}
              value={submit.type === 4 ? 0 : submit.value}
              onChange={(event) => setSubmit({ ...submit, value: event.target.value })}
            />
          </div>
          {submit.type === 5 ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">指定订阅</label>
              <div>
                <Select
                  value={submit.plan_id as string | number | undefined}
                  onChange={(value) =>
                    setSubmit({
                      ...submit,
                      plan_id: ((value as string).length ? value : null) as GiftcardSubmit['plan_id'],
                    })
                  }
                  mode={'single' as 'multiple'}
                  placeholder="指定订阅"
                  style={{ width: '100%' }}
                >
                  {(plans.data ?? []).map((plan) => (
                    <Select.Option key={Math.random()} value={`${plan.id}`}>
                      {plan.name}
                    </Select.Option>
                  ))}
                </Select>
              </div>
            </div>
          ) : null}
          <div className="form-group">
            <label htmlFor="example-text-input-alt">礼品卡有效期</label>
            <DatePicker.RangePicker
              style={{ width: '100%' }}
              showTime={{ format: 'HH:mm' }}
              format="YYYY-MM-DD HH:mm"
              placeholder={['Start Time', 'End Time']}
              value={rangeValue(submit.started_at, submit.ended_at)}
              onChange={onRangeChange}
              onOk={onRangeChange}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">最大使用次数</label>
            <Input
              placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
              value={submit.limit_use as string | number | undefined}
              onChange={(event) => setSubmit({ ...submit, limit_use: event.target.value })}
            />
          </div>
          {!submit.code && !submit.id ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">生成数量</label>
              <Input
                placeholder="输入数量批量生成"
                value={submit.generate_count}
                onChange={(event) =>
                  setSubmit({ ...submit, generate_count: event.target.value, code: undefined })
                }
              />
            </div>
          ) : null}
        </div>
      </Modal>
    </>
  );
}
