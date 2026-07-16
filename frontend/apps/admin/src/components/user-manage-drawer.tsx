import { useEffect, type ComponentProps } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import dayjs from 'dayjs';
import { CircleHelp } from 'lucide-react';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import type { AdminUserUpdateInput } from '@v2board/api-client';
import type { AdminUserRow } from '@v2board/types';
import { useAdminPlans, useAdminUserInfo, useUpdateUserMutation } from '@/lib/queries';
import { Button } from '@/components/ui/button';
import { ErrorState } from '@/components/ui/error-state';
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { EmptyState } from '@/components/ui/page';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from '@/components/ui/input-group';
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
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { userManageSchema, type UserManageFormValues } from './user-manage-schema';

function optionalNumber(value: string | number | null | undefined) {
  if (value === undefined) return undefined;
  if (value === null || value === '') return null;
  return Number(value);
}

type AdminUserEditRecord = Partial<AdminUserRow> & {
  invite_user_email?: string | null;
  commission_type?: number | string | null;
  speed_limit?: number | string | null;
  remarks?: string | null;
};

function toFormValues(user: AdminUserEditRecord): UserManageFormValues {
  return {
    email: user.email ?? '',
    invite_user_email: user.invite_user_email ?? '',
    password: user.password ?? '',
    transfer_enable: user.transfer_enable ?? '',
    u: user.u ?? '',
    d: user.d ?? '',
    commission_balance: user.commission_balance ?? '',
    balance: user.balance ?? '',
    device_limit: user.device_limit ?? null,
    expired_at: user.expired_at ?? null,
    plan_id: user.plan_id ?? null,
    banned: user.banned ?? 0,
    commission_type: user.commission_type ?? 0,
    commission_rate: user.commission_rate ?? null,
    discount: user.discount ?? null,
    speed_limit: user.speed_limit ?? null,
    is_admin: user.is_admin ?? 0,
    is_staff: user.is_staff ?? 0,
    remarks: user.remarks ?? '',
  };
}

function toPayload(values: UserManageFormValues, id: number): AdminUserUpdateInput {
  const payload: AdminUserUpdateInput = {
    ...values,
    id,
    password: values.password?.trim() === '' ? '' : values.password,
    // Keep display units here. @v2board/api-client performs the exact
    // decimal -> integer bytes/cents conversion at the wire boundary.
    transfer_enable: values.transfer_enable,
    u: values.u,
    d: values.d,
    balance: values.balance,
    commission_balance: values.commission_balance,
    expired_at: values.expired_at,
    device_limit: optionalNumber(values.device_limit),
    commission_rate: optionalNumber(values.commission_rate),
    discount: optionalNumber(values.discount),
    speed_limit: optionalNumber(values.speed_limit),
    is_admin: values.is_admin ? 1 : 0,
    is_staff: values.is_staff ? 1 : 0,
  };
  return payload;
}

// The expired_at field persists as unix seconds. Convert to a `YYYY-MM-DD`
// value for the native date input and back to a number with Day.js's core API.
function toDateInput(value: UserManageFormValues['expired_at']) {
  return value == null ? '' : dayjs(1000 * Number(value)).format('YYYY-MM-DD');
}

function fromDateInput(value: string) {
  return value ? dayjs(value).unix() : null;
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

function SuffixInput({
  suffix,
  ...props
}: ComponentProps<typeof InputGroupInput> & { suffix: string }) {
  return (
    <InputGroup>
      <InputGroupInput {...props} />
      <InputGroupAddon align="inline-end">
        <InputGroupText>{suffix}</InputGroupText>
      </InputGroupAddon>
    </InputGroup>
  );
}

export function UserManageDrawer({
  userId,
  open,
  onClose,
}: {
  userId?: number | null;
  open: boolean;
  onClose: () => void;
}) {
  const user = useAdminUserInfo(open ? userId : undefined);
  const plans = useAdminPlans();
  const update = useUpdateUserMutation();
  const current = user.data as AdminUserEditRecord | undefined;
  const form = useForm<UserManageFormValues>({
    resolver: zodResolver(userManageSchema),
    defaultValues: {
      email: '',
      invite_user_email: '',
      password: '',
      balance: '',
      commission_balance: '',
      transfer_enable: '',
      u: '',
      d: '',
      device_limit: null,
      expired_at: null,
      plan_id: null,
      banned: 0,
      commission_type: 0,
      commission_rate: null,
      discount: null,
      speed_limit: null,
      is_admin: 0,
      is_staff: 0,
      remarks: '',
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const planOptions = [
    { value: PLAN_NONE, label: '无' },
    ...(plans.data?.map((plan) => ({ value: String(plan.id), label: plan.name })) ?? []),
  ];

  useEffect(() => {
    if (current) {
      form.reset(toFormValues(current));
    } else if (open) {
      form.reset();
    }
  }, [current, form, open]);

  const hide = () => {
    form.reset();
    onClose();
  };

  const submit = form.handleSubmit((values) => {
    if (!userId || plans.isError || plans.isPending) return;
    update.mutate(toPayload(values, userId), { onSuccess: hide });
  });

  const commissionType = parseInt(
    String(useWatch({ control: form.control, name: 'commission_type' })),
  );
  const commissionTypeValue = Number.isNaN(commissionType) ? undefined : String(commissionType);
  const userError = Boolean(open && userId != null && user.isError);
  const userLoading = Boolean(open && userId != null && user.isPending);
  const plansError = Boolean(open && plans.isError);
  const plansLoading = Boolean(open && plans.isPending);

  return (
    <Sheet open={open} onOpenChange={(next) => (!next ? hide() : undefined)}>
      <SheetContent
        side="right"
        className="flex w-full flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl"
        data-testid="user-manage-drawer"
      >
        <SheetHeader className="border-b border-border px-6 py-4">
          <SheetTitle>用户管理</SheetTitle>
          <SheetDescription>查看并修改用户账户、订阅和余额信息。</SheetDescription>
        </SheetHeader>

        {userError ? (
          <div className="px-6 py-8">
            <ErrorState
              data-testid="user-manage-error"
              message="用户信息加载失败"
              onRetry={() => void user.refetch()}
            />
          </div>
        ) : plansError ? (
          <div className="px-6 py-8">
            <ErrorState
              data-testid="user-manage-plans-error"
              message="订阅列表加载失败"
              onRetry={() => void plans.refetch()}
            />
          </div>
        ) : userLoading || plansLoading ? (
          <div
            className="flex flex-1 items-center justify-center py-10"
            role="status"
            data-testid="user-manage-loading"
          >
            <Spinner className="size-6 text-muted-foreground" />
            <span className="sr-only">加载中</span>
          </div>
        ) : !current ? (
          <EmptyState
            className="m-6 min-h-32"
            data-testid="user-manage-empty"
            title={userId == null ? '未选择用户' : '未找到用户'}
          />
        ) : (
          <TooltipProvider delayDuration={100}>
            <form
              id="user-manage-form"
              className="flex min-h-0 flex-1 flex-col"
              onSubmit={submit}
              noValidate
            >
              <FieldGroup className="flex-1 overflow-y-auto px-6 py-4">
                <Field data-invalid={Boolean(formErrors.email)}>
                  <FieldLabel htmlFor="user-manage-email">邮箱</FieldLabel>
                  <Input
                    id="user-manage-email"
                    placeholder="请输入邮箱"
                    aria-invalid={Boolean(formErrors.email)}
                    data-testid="user-drawer-email"
                    {...form.register('email')}
                  />
                  <FieldError errors={[formErrors.email]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.invite_user_email)}>
                  <FieldLabel htmlFor="user-manage-invite-email">邀请人邮箱</FieldLabel>
                  <Input
                    id="user-manage-invite-email"
                    placeholder="请输入邀请人邮箱"
                    aria-invalid={Boolean(formErrors.invite_user_email)}
                    {...form.register('invite_user_email')}
                  />
                  <FieldError errors={[formErrors.invite_user_email]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.password)}>
                  <FieldLabel htmlFor="user-manage-password">密码</FieldLabel>
                  <Input
                    id="user-manage-password"
                    type="password"
                    placeholder="如需修改密码请输入"
                    aria-invalid={Boolean(formErrors.password)}
                    {...form.register('password')}
                  />
                  <FieldError errors={[formErrors.password]} />
                </Field>

                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <Field data-invalid={Boolean(formErrors.balance)}>
                    <FieldLabel htmlFor="user-manage-balance">余额</FieldLabel>
                    <SuffixInput
                      id="user-manage-balance"
                      type="number"
                      step="0.01"
                      suffix="¥"
                      placeholder="余额"
                      aria-invalid={Boolean(formErrors.balance)}
                      {...form.register('balance')}
                    />
                    <FieldError errors={[formErrors.balance]} />
                  </Field>
                  <Field data-invalid={Boolean(formErrors.commission_balance)}>
                    <FieldLabel htmlFor="user-manage-commission-balance">推广佣金</FieldLabel>
                    <SuffixInput
                      id="user-manage-commission-balance"
                      type="number"
                      step="0.01"
                      suffix="¥"
                      placeholder="推广佣金"
                      aria-invalid={Boolean(formErrors.commission_balance)}
                      {...form.register('commission_balance')}
                    />
                    <FieldError errors={[formErrors.commission_balance]} />
                  </Field>
                </div>

                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <Field data-invalid={Boolean(formErrors.u)}>
                    <FieldLabel htmlFor="user-manage-upload">已用上行</FieldLabel>
                    <SuffixInput
                      id="user-manage-upload"
                      type="number"
                      step="any"
                      suffix="GB"
                      placeholder="已用上行"
                      aria-invalid={Boolean(formErrors.u)}
                      {...form.register('u')}
                    />
                    <FieldError errors={[formErrors.u]} />
                  </Field>
                  <Field data-invalid={Boolean(formErrors.d)}>
                    <FieldLabel htmlFor="user-manage-download">已用下行</FieldLabel>
                    <SuffixInput
                      id="user-manage-download"
                      type="number"
                      step="any"
                      suffix="GB"
                      placeholder="已用下行"
                      aria-invalid={Boolean(formErrors.d)}
                      {...form.register('d')}
                    />
                    <FieldError errors={[formErrors.d]} />
                  </Field>
                </div>

                <Field data-invalid={Boolean(formErrors.transfer_enable)}>
                  <FieldLabel htmlFor="user-manage-transfer">流量</FieldLabel>
                  <SuffixInput
                    id="user-manage-transfer"
                    type="number"
                    step="any"
                    suffix="GB"
                    placeholder="请输入流量"
                    aria-invalid={Boolean(formErrors.transfer_enable)}
                    {...form.register('transfer_enable')}
                  />
                  <FieldError errors={[formErrors.transfer_enable]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.device_limit)}>
                  <FieldLabel htmlFor="user-manage-device-limit">设备数限制</FieldLabel>
                  <Input
                    id="user-manage-device-limit"
                    type="number"
                    placeholder="留空则不限制"
                    aria-invalid={Boolean(formErrors.device_limit)}
                    {...form.register('device_limit')}
                  />
                  <FieldError errors={[formErrors.device_limit]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.expired_at)}>
                  <FieldLabel htmlFor="user-manage-expired">到期时间</FieldLabel>
                  <Controller
                    control={form.control}
                    name="expired_at"
                    render={({ field }) => (
                      <Input
                        id="user-manage-expired"
                        name={field.name}
                        type="date"
                        placeholder="长期有效"
                        value={toDateInput(field.value)}
                        onChange={(event) =>
                          form.setValue('expired_at', fromDateInput(event.target.value), {
                            shouldDirty: true,
                            shouldValidate: true,
                          })
                        }
                        onBlur={field.onBlur}
                        ref={field.ref}
                        aria-invalid={Boolean(formErrors.expired_at)}
                        data-testid="user-drawer-expired"
                      />
                    )}
                  />
                  <FieldError errors={[formErrors.expired_at]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.commission_type)}>
                  <FieldLabel htmlFor="user-manage-plan">订阅计划</FieldLabel>
                  <Controller
                    control={form.control}
                    name="plan_id"
                    render={({ field }) => (
                      <Select
                        value={field.value ? String(field.value) : PLAN_NONE}
                        onValueChange={(value) =>
                          field.onChange(value === PLAN_NONE ? null : Number(value))
                        }
                      >
                        <SelectTrigger id="user-manage-plan" className="w-full">
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
                    )}
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="user-manage-status">账户状态</FieldLabel>
                  <Controller
                    control={form.control}
                    name="banned"
                    render={({ field }) => (
                      <Select
                        value={String(field.value ? 1 : 0)}
                        onValueChange={(value) => field.onChange(Number(value) as 0 | 1)}
                      >
                        <SelectTrigger id="user-manage-status" className="w-full">
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
                    )}
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="user-manage-commission-type">推荐返利类型</FieldLabel>
                  <Controller
                    control={form.control}
                    name="commission_type"
                    render={({ field }) => (
                      <Select
                        value={commissionTypeValue ?? ''}
                        onValueChange={(value) => field.onChange(Number(value))}
                      >
                        <SelectTrigger
                          id="user-manage-commission-type"
                          className="w-full"
                          aria-invalid={Boolean(formErrors.commission_type)}
                        >
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
                    )}
                  />
                  <FieldError errors={[formErrors.commission_type]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.commission_rate)}>
                  <FieldLabel htmlFor="user-manage-commission-rate">推荐返利比例</FieldLabel>
                  <SuffixInput
                    id="user-manage-commission-rate"
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    suffix="%"
                    placeholder="请输入推荐返利比例(为空则跟随站点设置返利比例)"
                    aria-invalid={Boolean(formErrors.commission_rate)}
                    {...form.register('commission_rate')}
                  />
                  <FieldError errors={[formErrors.commission_rate]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.discount)}>
                  <FieldLabel htmlFor="user-manage-discount">
                    <span className="inline-flex items-center gap-1">
                      专享折扣比例
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <span
                            tabIndex={0}
                            aria-label="专享折扣说明"
                            className="inline-flex cursor-help items-center outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                          >
                            <CircleHelp className="size-3.5 text-muted-foreground" />
                          </span>
                        </TooltipTrigger>
                        <TooltipContent>设置后该用户购买任何订阅将始终享受该折扣</TooltipContent>
                      </Tooltip>
                    </span>
                  </FieldLabel>
                  <SuffixInput
                    id="user-manage-discount"
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    suffix="%"
                    placeholder="请输入专享折扣比例"
                    aria-invalid={Boolean(formErrors.discount)}
                    {...form.register('discount')}
                  />
                  <FieldError errors={[formErrors.discount]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.speed_limit)}>
                  <FieldLabel htmlFor="user-manage-speed-limit">限速</FieldLabel>
                  <SuffixInput
                    id="user-manage-speed-limit"
                    type="number"
                    step="1"
                    suffix="Mbps"
                    placeholder="留空则不限制"
                    aria-invalid={Boolean(formErrors.speed_limit)}
                    {...form.register('speed_limit')}
                  />
                  <FieldError errors={[formErrors.speed_limit]} />
                </Field>
                <Controller
                  control={form.control}
                  name="is_admin"
                  render={({ field }) => (
                    <Field orientation="horizontal" className="justify-between">
                      <FieldLabel htmlFor="user-manage-is-admin">是否管理员</FieldLabel>
                      <Switch
                        id="user-manage-is-admin"
                        checked={Boolean(field.value)}
                        onCheckedChange={(checked) => field.onChange(checked ? 1 : 0)}
                      />
                    </Field>
                  )}
                />
                <Controller
                  control={form.control}
                  name="is_staff"
                  render={({ field }) => (
                    <Field orientation="horizontal" className="justify-between">
                      <FieldLabel htmlFor="user-manage-is-staff">是否员工</FieldLabel>
                      <Switch
                        id="user-manage-is-staff"
                        checked={Boolean(field.value)}
                        onCheckedChange={(checked) => field.onChange(checked ? 1 : 0)}
                      />
                    </Field>
                  )}
                />
                <Field>
                  <FieldLabel htmlFor="user-manage-remarks">备注</FieldLabel>
                  <Textarea
                    id="user-manage-remarks"
                    rows={4}
                    placeholder="请在这里记录.."
                    {...form.register('remarks')}
                  />
                  <FieldDescription>仅供管理员内部记录。</FieldDescription>
                </Field>
              </FieldGroup>

              <SheetFooter className="flex-row justify-end gap-2 border-t border-border px-6 py-4">
                <Button type="button" variant="outline" onClick={hide}>
                  取消
                </Button>
                <Button
                  type="submit"
                  disabled={update.isPending}
                  loading={update.isPending}
                  data-testid="user-manage-submit"
                >
                  提交
                </Button>
              </SheetFooter>
            </form>
          </TooltipProvider>
        )}
      </SheetContent>
    </Sheet>
  );
}
