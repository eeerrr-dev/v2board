import { useEffect, type ComponentProps } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import dayjs from 'dayjs';
import { CircleHelp } from 'lucide-react';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
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
import { LoadingState, SkeletonFields } from '@/components/ui/loading-state';
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
    // §6.6 (W12): the detail delivers `expired_at` as an RFC 3339 string; the
    // editor form keeps it as epoch seconds for its native date input.
    expired_at: user.expired_at == null ? null : dayjs(user.expired_at).unix(),
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

// Wire values (banned/commission_type codes sent to the API) are the record
// keys; only the labels are translated, resolved at render time.
function accountStatusOptions(t: TFunction) {
  return [
    { value: '1', label: t(($) => $.admin.users.status_banned) },
    { value: '0', label: t(($) => $.admin.users.status_normal) },
  ];
}

function commissionTypeOptions(t: TFunction) {
  return [
    { value: '0', label: t(($) => $.admin.users.commission_type_system) },
    { value: '1', label: t(($) => $.admin.users.commission_type_cycle) },
    { value: '2', label: t(($) => $.admin.users.commission_type_first) },
  ];
}

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
  const { t } = useTranslation();
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
    { value: PLAN_NONE, label: t(($) => $.admin.users.none) },
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
          <SheetTitle>{t(($) => $.admin.users.title)}</SheetTitle>
          <SheetDescription>{t(($) => $.admin.users.drawer_description)}</SheetDescription>
        </SheetHeader>

        {userError ? (
          <div className="px-6 py-8">
            <ErrorState
              data-testid="user-manage-error"
              message={t(($) => $.admin.users.user_info_load_failed)}
              onRetry={() => void user.refetch()}
            />
          </div>
        ) : plansError ? (
          <div className="px-6 py-8">
            <ErrorState
              data-testid="user-manage-plans-error"
              message={t(($) => $.admin.users.plans_load_failed)}
              onRetry={() => void plans.refetch()}
            />
          </div>
        ) : userLoading || plansLoading ? (
          <LoadingState className="flex-1 p-6" data-testid="user-manage-loading">
            <SkeletonFields fields={5} />
          </LoadingState>
        ) : !current ? (
          <EmptyState
            className="m-6 min-h-32"
            data-testid="user-manage-empty"
            title={
              userId == null
                ? t(($) => $.admin.users.no_user_selected)
                : t(($) => $.admin.users.user_not_found)
            }
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
                  <FieldLabel htmlFor="user-manage-email">
                    {t(($) => $.admin.users.email)}
                  </FieldLabel>
                  <Input
                    id="user-manage-email"
                    placeholder={t(($) => $.admin.users.email_placeholder)}
                    aria-invalid={Boolean(formErrors.email)}
                    data-testid="user-drawer-email"
                    {...form.register('email')}
                  />
                  <FieldError errors={[formErrors.email]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.invite_user_email)}>
                  <FieldLabel htmlFor="user-manage-invite-email">
                    {t(($) => $.admin.users.invite_user_email)}
                  </FieldLabel>
                  <Input
                    id="user-manage-invite-email"
                    placeholder={t(($) => $.admin.users.invite_user_email_placeholder)}
                    aria-invalid={Boolean(formErrors.invite_user_email)}
                    {...form.register('invite_user_email')}
                  />
                  <FieldError errors={[formErrors.invite_user_email]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.password)}>
                  <FieldLabel htmlFor="user-manage-password">
                    {t(($) => $.admin.users.password)}
                  </FieldLabel>
                  <Input
                    id="user-manage-password"
                    type="password"
                    placeholder={t(($) => $.admin.users.password_edit_placeholder)}
                    aria-invalid={Boolean(formErrors.password)}
                    {...form.register('password')}
                  />
                  <FieldError errors={[formErrors.password]} />
                </Field>

                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <Field data-invalid={Boolean(formErrors.balance)}>
                    <FieldLabel htmlFor="user-manage-balance">
                      {t(($) => $.admin.users.balance)}
                    </FieldLabel>
                    <SuffixInput
                      id="user-manage-balance"
                      type="number"
                      step="0.01"
                      suffix="¥"
                      placeholder={t(($) => $.admin.users.balance)}
                      aria-invalid={Boolean(formErrors.balance)}
                      {...form.register('balance')}
                    />
                    <FieldError errors={[formErrors.balance]} />
                  </Field>
                  <Field data-invalid={Boolean(formErrors.commission_balance)}>
                    <FieldLabel htmlFor="user-manage-commission-balance">
                      {t(($) => $.admin.users.commission_balance)}
                    </FieldLabel>
                    <SuffixInput
                      id="user-manage-commission-balance"
                      type="number"
                      step="0.01"
                      suffix="¥"
                      placeholder={t(($) => $.admin.users.commission_balance)}
                      aria-invalid={Boolean(formErrors.commission_balance)}
                      {...form.register('commission_balance')}
                    />
                    <FieldError errors={[formErrors.commission_balance]} />
                  </Field>
                </div>

                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <Field data-invalid={Boolean(formErrors.u)}>
                    <FieldLabel htmlFor="user-manage-upload">
                      {t(($) => $.admin.users.upload_used)}
                    </FieldLabel>
                    <SuffixInput
                      id="user-manage-upload"
                      type="number"
                      step="any"
                      suffix="GB"
                      placeholder={t(($) => $.admin.users.upload_used)}
                      aria-invalid={Boolean(formErrors.u)}
                      {...form.register('u')}
                    />
                    <FieldError errors={[formErrors.u]} />
                  </Field>
                  <Field data-invalid={Boolean(formErrors.d)}>
                    <FieldLabel htmlFor="user-manage-download">
                      {t(($) => $.admin.users.download_used)}
                    </FieldLabel>
                    <SuffixInput
                      id="user-manage-download"
                      type="number"
                      step="any"
                      suffix="GB"
                      placeholder={t(($) => $.admin.users.download_used)}
                      aria-invalid={Boolean(formErrors.d)}
                      {...form.register('d')}
                    />
                    <FieldError errors={[formErrors.d]} />
                  </Field>
                </div>

                <Field data-invalid={Boolean(formErrors.transfer_enable)}>
                  <FieldLabel htmlFor="user-manage-transfer">
                    {t(($) => $.admin.users.transfer)}
                  </FieldLabel>
                  <SuffixInput
                    id="user-manage-transfer"
                    type="number"
                    step="any"
                    suffix="GB"
                    placeholder={t(($) => $.admin.users.transfer_placeholder)}
                    aria-invalid={Boolean(formErrors.transfer_enable)}
                    {...form.register('transfer_enable')}
                  />
                  <FieldError errors={[formErrors.transfer_enable]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.device_limit)}>
                  <FieldLabel htmlFor="user-manage-device-limit">
                    {t(($) => $.admin.users.device_limit)}
                  </FieldLabel>
                  <Input
                    id="user-manage-device-limit"
                    type="number"
                    placeholder={t(($) => $.admin.users.no_limit_placeholder)}
                    aria-invalid={Boolean(formErrors.device_limit)}
                    {...form.register('device_limit')}
                  />
                  <FieldError errors={[formErrors.device_limit]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.expired_at)}>
                  <FieldLabel htmlFor="user-manage-expired">
                    {t(($) => $.admin.users.expired_at)}
                  </FieldLabel>
                  <Controller
                    control={form.control}
                    name="expired_at"
                    render={({ field }) => (
                      <Input
                        id="user-manage-expired"
                        name={field.name}
                        type="date"
                        placeholder={t(($) => $.common.long_term)}
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
                  <FieldLabel htmlFor="user-manage-plan">
                    {t(($) => $.admin.users.subscription_plan)}
                  </FieldLabel>
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
                          <SelectValue
                            placeholder={t(($) => $.admin.users.plan_select_placeholder)}
                          />
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
                  <FieldLabel htmlFor="user-manage-status">
                    {t(($) => $.admin.users.account_status_edit)}
                  </FieldLabel>
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
                          {accountStatusOptions(t).map((option) => (
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
                  <FieldLabel htmlFor="user-manage-commission-type">
                    {t(($) => $.admin.users.commission_type)}
                  </FieldLabel>
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
                          <SelectValue
                            placeholder={t(($) => $.admin.users.commission_type_placeholder)}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          {commissionTypeOptions(t).map((option) => (
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
                  <FieldLabel htmlFor="user-manage-commission-rate">
                    {t(($) => $.admin.users.commission_rate)}
                  </FieldLabel>
                  <SuffixInput
                    id="user-manage-commission-rate"
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    suffix="%"
                    placeholder={t(($) => $.admin.users.commission_rate_placeholder)}
                    aria-invalid={Boolean(formErrors.commission_rate)}
                    {...form.register('commission_rate')}
                  />
                  <FieldError errors={[formErrors.commission_rate]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.discount)}>
                  <FieldLabel htmlFor="user-manage-discount">
                    <span className="inline-flex items-center gap-1">
                      {t(($) => $.admin.users.discount)}
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <span
                            tabIndex={0}
                            aria-label={t(($) => $.admin.users.discount_help_label)}
                            className="inline-flex cursor-help items-center outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                          >
                            <CircleHelp className="size-3.5 text-muted-foreground" />
                          </span>
                        </TooltipTrigger>
                        <TooltipContent>{t(($) => $.admin.users.discount_help)}</TooltipContent>
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
                    placeholder={t(($) => $.admin.users.discount)}
                    aria-invalid={Boolean(formErrors.discount)}
                    {...form.register('discount')}
                  />
                  <FieldError errors={[formErrors.discount]} />
                </Field>
                <Field data-invalid={Boolean(formErrors.speed_limit)}>
                  <FieldLabel htmlFor="user-manage-speed-limit">
                    {t(($) => $.admin.users.speed_limit)}
                  </FieldLabel>
                  <SuffixInput
                    id="user-manage-speed-limit"
                    type="number"
                    step="1"
                    suffix="Mbps"
                    placeholder={t(($) => $.admin.users.no_limit_placeholder)}
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
                      <FieldLabel htmlFor="user-manage-is-admin">
                        {t(($) => $.admin.users.is_admin_label)}
                      </FieldLabel>
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
                      <FieldLabel htmlFor="user-manage-is-staff">
                        {t(($) => $.admin.users.is_staff_label)}
                      </FieldLabel>
                      <Switch
                        id="user-manage-is-staff"
                        checked={Boolean(field.value)}
                        onCheckedChange={(checked) => field.onChange(checked ? 1 : 0)}
                      />
                    </Field>
                  )}
                />
                <Field>
                  <FieldLabel htmlFor="user-manage-remarks">
                    {t(($) => $.admin.users.remarks)}
                  </FieldLabel>
                  <Textarea
                    id="user-manage-remarks"
                    rows={4}
                    placeholder={t(($) => $.admin.users.remarks_placeholder)}
                    {...form.register('remarks')}
                  />
                  <FieldDescription>{t(($) => $.admin.users.remarks_hint)}</FieldDescription>
                </Field>
              </FieldGroup>

              <SheetFooter className="flex-row justify-end gap-2 border-t border-border px-6 py-4">
                <Button type="button" variant="outline" onClick={hide}>
                  {t(($) => $.common.cancel)}
                </Button>
                <Button
                  type="submit"
                  disabled={update.isPending}
                  loading={update.isPending}
                  data-testid="user-manage-submit"
                >
                  {t(($) => $.common.submit)}
                </Button>
              </SheetFooter>
            </form>
          </TooltipProvider>
        )}
      </SheetContent>
    </Sheet>
  );
}
