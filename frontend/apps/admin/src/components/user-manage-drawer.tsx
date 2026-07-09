import { useEffect, useState, type ReactNode } from 'react';
import dayjs from 'dayjs';
import { CircleHelp } from 'lucide-react';
import type { AdminUserRow, AdminUserUpdatePayload } from '@v2board/types';
import { BYTE_GB } from '@v2board/config/format';
import { useAdminPlans, useAdminUserInfo, useUpdateUserMutation } from '@/lib/queries';
import { Button } from '@/components/ui/button';
import { Input, type InputProps } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Sheet,
  SheetContent,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';

type UserManageFormValues = Omit<Partial<AdminUserRow>, 'expired_at' | 'is_admin' | 'is_staff'> & {
  invite_user_email?: string | null;
  commission_type?: number | string | null;
  speed_limit?: number | string | null;
  remarks?: string | null;
  expired_at?: number | string | null;
  is_admin?: 0 | 1;
  is_staff?: 0 | 1;
};

// The GB / cents scaling and the expired_at passthrough below are the Tier-1
// save contract with `/user/update` — keep the exact coercions byte-identical.
function scaledRounded(value: unknown, multiplier: number) {
  return Math.round(Number(value) * multiplier);
}

function scaled(value: unknown, multiplier: number) {
  return Number(value) * multiplier;
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

// The expired_at field persists as unix SECONDS (string on edit, or the original
// number when untouched). Convert to a `YYYY-MM-DD` value for the native date
// input and back to a seconds string on change, mirroring the legacy
// dayjs(1000 * sec) / dayjs(value).format('X') round-trip.
function toDateInput(value: UserManageFormValues['expired_at']) {
  return value == null ? '' : dayjs(1000 * Number(value)).format('YYYY-MM-DD');
}

function fromDateInput(value: string) {
  return value ? dayjs(value).format('X') : null;
}

const PLAN_NONE = 'null';

const ACCOUNT_STATUS_OPTIONS = [
  { value: '1', label: '封禁' },
  { value: '0', label: '正常' },
];

const COMMISSION_TYPE_OPTIONS = [
  { value: '0', label: '跟随系统设置' },
  { value: '1', label: '循环返利' },
  { value: '2', label: '首次返利' },
];

function Field({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-2">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

function SuffixInput({ suffix, ...props }: InputProps & { suffix: string }) {
  return (
    <div className="relative">
      <Input className="pr-10" {...props} />
      <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
        {suffix}
      </span>
    </div>
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
  onSaved?: () => void | Promise<unknown>;
}) {
  const [values, setValues] = useState<UserManageFormValues | null>(null);
  const user = useAdminUserInfo(open ? userId : undefined);
  const plans = useAdminPlans();
  const update = useUpdateUserMutation();
  const current = user.data as (Partial<AdminUserRow> & Record<string, unknown>) | undefined;
  const planOptions = [
    { value: PLAN_NONE, label: '无' },
    ...(plans.data?.map((plan) => ({ value: String(plan.id), label: plan.name })) ?? []),
  ];

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
      .then(async () => {
        await onSaved?.();
        hide();
      })
      // Errors are surfaced by the global onError handler; keep the drawer open.
      .catch(() => undefined);
  };

  const commissionType = parseInt(String(values?.commission_type));
  const commissionTypeValue = Number.isNaN(commissionType) ? undefined : String(commissionType);

  return (
    <Sheet open={open} onOpenChange={(next) => (!next ? hide() : undefined)}>
      <SheetContent
        side="right"
        className="flex w-full flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl"
        data-testid="user-manage-drawer"
      >
        <SheetHeader className="border-b border-border px-6 py-4">
          <SheetTitle>用户管理</SheetTitle>
        </SheetHeader>

        {values?.email ? (
          <TooltipProvider delayDuration={100}>
            <div className="flex-1 space-y-4 overflow-y-auto px-6 py-4">
              <Field label="邮箱">
                <Input
                  placeholder="请输入邮箱"
                  value={values.email ?? ''}
                  onChange={(event) => formChange('email', event.target.value)}
                  data-testid="user-drawer-email"
                />
              </Field>
              <Field label="邀请人邮箱">
                <Input
                  placeholder="请输入邀请人邮箱"
                  value={values.invite_user_email ?? ''}
                  onChange={(event) => formChange('invite_user_email', event.target.value)}
                />
              </Field>
              <Field label="密码">
                <Input
                  placeholder="如需修改密码请输入"
                  value={values.password ?? ''}
                  onChange={(event) => formChange('password', event.target.value)}
                />
              </Field>

              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Field label="余额">
                  <SuffixInput
                    type="number"
                    suffix="¥"
                    placeholder="余额"
                    value={values.balance ?? ''}
                    onChange={(event) =>
                      formChange('balance', event.target.value as unknown as number)
                    }
                  />
                </Field>
                <Field label="推广佣金">
                  <SuffixInput
                    type="number"
                    suffix="¥"
                    placeholder="推广佣金"
                    value={values.commission_balance ?? ''}
                    onChange={(event) =>
                      formChange('commission_balance', event.target.value as unknown as number)
                    }
                  />
                </Field>
              </div>

              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Field label="已用上行">
                  <SuffixInput
                    type="number"
                    suffix="GB"
                    placeholder="已用上行"
                    value={values.u ?? ''}
                    onChange={(event) => formChange('u', event.target.value as unknown as number)}
                  />
                </Field>
                <Field label="已用下行">
                  <SuffixInput
                    type="number"
                    suffix="GB"
                    placeholder="已用下行"
                    value={values.d ?? ''}
                    onChange={(event) => formChange('d', event.target.value as unknown as number)}
                  />
                </Field>
              </div>

              <Field label="流量">
                <SuffixInput
                  type="number"
                  suffix="GB"
                  placeholder="请输入流量"
                  value={values.transfer_enable ?? ''}
                  onChange={(event) =>
                    formChange('transfer_enable', event.target.value as unknown as number)
                  }
                />
              </Field>
              <Field label="设备数限制">
                <Input
                  placeholder="留空则不限制"
                  value={values.device_limit ?? ''}
                  onChange={(event) =>
                    formChange('device_limit', event.target.value as unknown as number)
                  }
                />
              </Field>
              <Field label="到期时间">
                <Input
                  type="date"
                  placeholder="长期有效"
                  value={toDateInput(values.expired_at)}
                  onChange={(event) => formChange('expired_at', fromDateInput(event.target.value))}
                  data-testid="user-drawer-expired"
                />
              </Field>
              <Field label="订阅计划">
                <Select
                  value={values.plan_id ? String(values.plan_id) : PLAN_NONE}
                  onValueChange={(value) =>
                    formChange('plan_id', value === PLAN_NONE ? null : Number(value))
                  }
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="请选择用户订阅计划" />
                  </SelectTrigger>
                  <SelectContent>
                    {planOptions.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Field label="账户状态">
                <Select
                  value={String(values.banned ? 1 : 0)}
                  onValueChange={(value) => formChange('banned', Number(value) as 0 | 1)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {ACCOUNT_STATUS_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Field label="推荐返利类型">
                <Select
                  value={commissionTypeValue}
                  onValueChange={(value) => formChange('commission_type', Number(value))}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="请选择推荐返利类型" />
                  </SelectTrigger>
                  <SelectContent>
                    {COMMISSION_TYPE_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Field label="推荐返利比例">
                <SuffixInput
                  suffix="%"
                  placeholder="请输入推荐返利比例(为空则跟随站点设置返利比例)"
                  value={values.commission_rate ?? ''}
                  onChange={(event) =>
                    formChange('commission_rate', event.target.value as unknown as number)
                  }
                />
              </Field>
              <Field
                label={
                  <span className="inline-flex items-center gap-1">
                    专享折扣比例
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <span
                          tabIndex={0}
                          className="inline-flex cursor-help items-center outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                        >
                          <CircleHelp className="size-3.5 text-muted-foreground" />
                        </span>
                      </TooltipTrigger>
                      <TooltipContent>设置后该用户购买任何订阅将始终享受该折扣</TooltipContent>
                    </Tooltip>
                  </span>
                }
              >
                <SuffixInput
                  suffix="%"
                  placeholder="请输入专享折扣比例"
                  value={values.discount ?? ''}
                  onChange={(event) =>
                    formChange('discount', event.target.value as unknown as number)
                  }
                />
              </Field>
              <Field label="限速">
                <SuffixInput
                  suffix="Mbps"
                  placeholder="留空则不限制"
                  value={values.speed_limit ?? ''}
                  onChange={(event) => formChange('speed_limit', event.target.value)}
                />
              </Field>
              <div className="flex items-center justify-between">
                <Label>是否管理员</Label>
                <Switch
                  checked={Boolean(values.is_admin)}
                  onCheckedChange={(checked) => formChange('is_admin', checked ? 1 : 0)}
                  aria-label="是否管理员"
                />
              </div>
              <div className="flex items-center justify-between">
                <Label>是否员工</Label>
                <Switch
                  checked={Boolean(values.is_staff)}
                  onCheckedChange={(checked) => formChange('is_staff', checked ? 1 : 0)}
                  aria-label="是否员工"
                />
              </div>
              <Field label="备注">
                <Textarea
                  rows={4}
                  placeholder="请在这里记录.."
                  value={values.remarks ?? ''}
                  onChange={(event) => formChange('remarks', event.target.value)}
                />
              </Field>
            </div>

            <SheetFooter className="flex-row justify-end gap-2 border-t border-border px-6 py-4">
              <Button variant="outline" onClick={hide}>
                取消
              </Button>
              <Button
                onClick={() => !update.isPending && submit()}
                disabled={update.isPending}
                loading={update.isPending}
                data-testid="user-manage-submit"
              >
                提交
              </Button>
            </SheetFooter>
          </TooltipProvider>
        ) : (
          <div className="flex flex-1 items-center justify-center py-10" role="status">
            <Spinner className="size-6 text-muted-foreground" />
            <span className="sr-only">加载中</span>
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}
