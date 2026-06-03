import { useEffect, useState } from 'react';
import { App, Button, DatePicker, Drawer, Input, Select, Switch, Tooltip } from 'antd';
import { QuestionCircleOutlined } from '@ant-design/icons';
import dayjs, { type Dayjs } from 'dayjs';
import type { AdminUserRow, AdminUserUpdatePayload } from '@v2board/types';
import { BYTE_GB } from '@v2board/config/format';
import { useAdminPlans, useAdminUserInfo, useUpdateUserMutation } from '@/lib/queries';
import { i18nGet } from '@/lib/errors';

type UserManageFormValues = Omit<Partial<AdminUserRow>, 'expired_at' | 'is_admin' | 'is_staff'> & {
  invite_user_email?: string | null;
  commission_type?: number | string | null;
  speed_limit?: number | string | null;
  remarks?: string | null;
  expired_at?: number | string | null;
  is_admin?: 0 | 1;
  is_staff?: 0 | 1;
};

function scaledRounded(value: unknown, multiplier: number) {
  return Math.round(Number(value) * multiplier);
}

function scaled(value: unknown, multiplier: number) {
  return Number(value) * multiplier;
}

function legacyDefaultValue(value: unknown) {
  return value as string | number | readonly string[] | undefined;
}

function legacyExpiredAtDefaultValue(value: UserManageFormValues['expired_at']) {
  return (value !== null && dayjs(1000 * Number(value))) as unknown as Dayjs;
}

function toFormValues(user: Partial<AdminUserRow> & Record<string, unknown>): UserManageFormValues {
  const inviteUser = user.invite_user as { email?: string } | undefined;
  return {
    ...user,
    transfer_enable: user.transfer_enable as unknown as number,
    u: user.u as unknown as number,
    d: user.d as unknown as number,
    commission_balance: user.commission_balance as unknown as number,
    balance: user.balance as unknown as number,
    invite_user_email: (user.invite_user_email as string | undefined) ?? inviteUser?.email,
    expired_at: user.expired_at,
    is_admin: user.is_admin,
    is_staff: user.is_staff,
  };
}

function toPayload(values: UserManageFormValues, id: number): AdminUserUpdatePayload {
  const payload = {
    ...values,
    id,
    transfer_enable: scaled(values.transfer_enable, BYTE_GB),
    u: scaledRounded(values.u, BYTE_GB),
    d: scaledRounded(values.d, BYTE_GB),
    balance: scaledRounded(values.balance, 100),
    commission_balance: scaledRounded(values.commission_balance, 100),
    expired_at: values.expired_at as unknown as number | null,
    is_admin: values.is_admin ? 1 : 0,
    is_staff: values.is_staff ? 1 : 0,
  };
  if ((payload as Record<string, unknown>).invite_user) {
    delete (payload as Record<string, unknown>).invite_user;
  }
  return {
    ...payload,
  } as unknown as AdminUserUpdatePayload;
}

function LegacyDrawerLoadingIcon() {
  return (
    <i
      aria-label="图标: loading"
      className="anticon anticon-loading"
      style={{ fontSize: 24, color: '#415A94' }}
    >
      <svg
        className="anticon-spin"
        viewBox="0 0 1024 1024"
        focusable="false"
        data-icon="loading"
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 0 0-94.3-139.9 437.71 437.71 0 0 0-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z" />
      </svg>
    </i>
  );
}

export function UserManageDrawer({
  userId,
  open,
  onClose,
  onSaved,
}: {
  userId?: number | null;
  open: boolean;
  onClose: () => void;
  onSaved?: () => void;
}) {
  const { message } = App.useApp();
  const [values, setValues] = useState<UserManageFormValues | null>(null);
  const user = useAdminUserInfo(open ? userId : undefined);
  const plans = useAdminPlans();
  const update = useUpdateUserMutation();
  const current = user.data as (Partial<AdminUserRow> & Record<string, unknown>) | undefined;

  useEffect(() => {
    if (current?.email) {
      setValues(toFormValues(current));
    } else if (open) {
      setValues(null);
    }
  }, [current, open]);

  const hide = () => {
    setValues(null);
    onClose();
  };

  const formChange = <K extends keyof UserManageFormValues>(
    key: K,
    value: UserManageFormValues[K],
  ) => {
    setValues((state) => ({ ...(state ?? {}), [key]: value }));
  };

  const submit = () => {
    if (!userId || !values) return;
    update
      .mutateAsync(toPayload(values, userId))
      .then(() => {
        onSaved?.();
        hide();
      })
      .catch((error: unknown) => {
        if (error instanceof Error) message.error(i18nGet(error.message));
      });
  };

  return (
    <Drawer id="user" width="80%" title="用户管理" open={open} onClose={hide}>
      {values?.email ? (
        <div>
          <div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">邮箱</label>
              <Input
                placeholder="请输入邮箱"
                defaultValue={values.email}
                onChange={(event) => formChange('email', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">邀请人邮箱</label>
              <Input
                placeholder="请输入邀请人邮箱"
                defaultValue={legacyDefaultValue(values.invite_user_email)}
                onChange={(event) => formChange('invite_user_email', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">密码</label>
              <Input
                defaultValue={values.password}
                placeholder="如需修改密码请输入"
                onChange={(event) => formChange('password', event.target.value)}
              />
            </div>
            <div className="row">
              <div className="form-group col-md-6 col-xs-12">
                <label>余额</label>
                <Input
                  type="number"
                  addonAfter="¥"
                  placeholder="余额"
                  defaultValue={values.balance}
                  onChange={(event) =>
                    formChange('balance', event.target.value as unknown as number)
                  }
                />
              </div>
              <div className="form-group col-md-6 col-xs-12">
                <label>推广佣金</label>
                <Input
                  type="number"
                  addonAfter="¥"
                  placeholder="推广佣金"
                  defaultValue={values.commission_balance}
                  onChange={(event) =>
                    formChange('commission_balance', event.target.value as unknown as number)
                  }
                />
              </div>
            </div>
            <div className="row">
              <div className="form-group col-md-6 col-xs-12">
                <label>已用上行</label>
                <Input
                  type="number"
                  addonAfter="GB"
                  placeholder="已用上行"
                  defaultValue={values.u}
                  onChange={(event) => formChange('u', event.target.value as unknown as number)}
                />
              </div>
              <div className="form-group col-md-6 col-xs-12">
                <label>已用下行</label>
                <Input
                  type="number"
                  addonAfter="GB"
                  placeholder="已用下行"
                  defaultValue={values.d}
                  onChange={(event) => formChange('d', event.target.value as unknown as number)}
                />
              </div>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">流量</label>
              <Input
                type="number"
                addonAfter="GB"
                defaultValue={values.transfer_enable}
                placeholder="请输入流量"
                onChange={(event) =>
                  formChange('transfer_enable', event.target.value as unknown as number)
                }
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">设备数限制</label>
              <Input
                placeholder="留空则不限制"
                defaultValue={legacyDefaultValue(values.device_limit)}
                onChange={(event) =>
                  formChange('device_limit', event.target.value as unknown as number)
                }
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">到期时间</label>
              <div>
                <DatePicker
                  placeholder="长期有效"
                  defaultValue={legacyExpiredAtDefaultValue(values.expired_at)}
                  style={{ width: '100%' }}
                  onChange={(value) => formChange('expired_at', value ? value.format('X') : null)}
                />
              </div>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">订阅计划</label>
              <Select
                placeholder="请选择用户订阅计划"
                style={{ width: '100%' }}
                defaultValue={values.plan_id || null}
                onChange={(value) => formChange('plan_id', value)}
              >
                <Select.Option value={null}>无</Select.Option>
                {plans.data?.map((plan) => (
                  <Select.Option key={Math.random()} value={plan.id}>
                    {plan.name}
                  </Select.Option>
                ))}
              </Select>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">账户状态</label>
              <Select
                style={{ width: '100%' }}
                defaultValue={values.banned ? 1 : 0}
                onChange={(value) => formChange('banned', value as 0 | 1)}
              >
                <Select.Option key={1} value={1}>
                  封禁
                </Select.Option>
                <Select.Option key={0} value={0}>
                  正常
                </Select.Option>
              </Select>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">推荐返利类型</label>
              <Select
                style={{ width: '100%' }}
                defaultValue={parseInt(values.commission_type as string)}
                onChange={(value) => formChange('commission_type', value)}
              >
                <Select.Option key={0} value={0}>
                  跟随系统设置
                </Select.Option>
                <Select.Option key={1} value={1}>
                  循环返利
                </Select.Option>
                <Select.Option key={2} value={2}>
                  首次返利
                </Select.Option>
              </Select>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">推荐返利比例</label>
              <Input
                addonAfter="%"
                defaultValue={legacyDefaultValue(values.commission_rate)}
                placeholder="请输入推荐返利比例(为空则跟随站点设置返利比例)"
                onChange={(event) =>
                  formChange('commission_rate', event.target.value as unknown as number)
                }
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">
                专享折扣比例{' '}
                <Tooltip title="设置后该用户购买任何订阅将始终享受该折扣" placement="top">
                  <QuestionCircleOutlined />
                </Tooltip>
              </label>
              <Input
                addonAfter="%"
                defaultValue={legacyDefaultValue(values.discount)}
                placeholder="请输入专享折扣比例"
                onChange={(event) =>
                  formChange('discount', event.target.value as unknown as number)
                }
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">限速</label>
              <Input
                addonAfter="Mbps"
                defaultValue={legacyDefaultValue(values.speed_limit)}
                placeholder="留空则不限制"
                onChange={(event) => formChange('speed_limit', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">是否管理员</label>
              <div>
                <Switch
                  checked={values.is_admin as unknown as boolean}
                  onChange={(value) => formChange('is_admin', value ? 1 : 0)}
                />
              </div>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">是否员工</label>
              <div>
                <Switch
                  checked={values.is_staff as unknown as boolean}
                  onChange={(value) => formChange('is_staff', value ? 1 : 0)}
                />
              </div>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">备注</label>
              <div>
                <Input.TextArea
                  rows={4}
                  placeholder="请在这里记录.."
                  defaultValue={legacyDefaultValue(values.remarks)}
                  onChange={(event) => formChange('remarks', event.target.value)}
                />
              </div>
            </div>
          </div>
          <div className="v2board-drawer-action">
            <Button style={{ marginRight: 8 }} onClick={hide}>
              取消
            </Button>
            <Button
              disabled={update.isPending}
              loading={update.isPending}
              onClick={submit}
              type="primary"
            >
              提交
            </Button>
          </div>
        </div>
      ) : (
        <LegacyDrawerLoadingIcon />
      )}
    </Drawer>
  );
}
