import { screen, waitFor } from '@testing-library/react';
import { ApiProblemError } from '@v2board/api-client';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { MfaDialog } from './mfa-dialog';

// §6.10 account MFA management: enrollment (setup → one-time secret →
// confirm-with-live-code) and disable-with-live-code, with the wrong-code
// problem surfaced inline. The wire payloads live in @v2board/api-client and
// are pinned by the golden lane; this covers the dialog behavior.

interface MutationMock {
  mutate: ReturnType<typeof vi.fn>;
  reset: ReturnType<typeof vi.fn>;
  isPending: boolean;
  data?: unknown;
}

const mocks = vi.hoisted(() => ({
  status: { data: undefined as unknown, isPending: false },
  setup: { mutate: vi.fn(), reset: vi.fn(), isPending: false, data: undefined } as MutationMock,
  confirm: { mutate: vi.fn(), reset: vi.fn(), isPending: false } as MutationMock,
  disable: { mutate: vi.fn(), reset: vi.fn(), isPending: false } as MutationMock,
  toastSuccess: vi.fn(),
  copyText: vi.fn().mockResolvedValue(true),
}));

vi.mock('@/lib/queries', () => ({
  useAccountMfa: () => mocks.status,
  useSetupTotpMutation: () => mocks.setup,
  useConfirmTotpMutation: () => mocks.confirm,
  useDisableTotpMutation: () => mocks.disable,
}));

vi.mock('@/lib/toast', () => ({
  toast: { success: mocks.toastSuccess, error: vi.fn() },
}));

vi.mock('@v2board/config/clipboard', () => ({
  copyText: mocks.copyText,
}));

const PROVISIONING = {
  secret: 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ',
  otpauth_url:
    'otpauth://totp/V2Board:admin@example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=V2Board&algorithm=SHA1&digits=6&period=30',
};

function invalidCodeProblem() {
  return new ApiProblemError(401, {
    type: 'about:blank',
    title: 'Unauthorized',
    status: 401,
    code: 'mfa_code_invalid',
    detail: '验证码错误',
  });
}

function renderDialog() {
  return renderWithProviders(<MfaDialog open onOpenChange={() => {}} />, { queryClient: true });
}

describe('MfaDialog', () => {
  beforeEach(() => {
    mocks.status.data = { totp_enabled: false, totp_enabled_at: null };
    mocks.status.isPending = false;
    mocks.setup.mutate.mockReset();
    mocks.setup.reset.mockReset();
    mocks.setup.data = undefined;
    mocks.confirm.mutate.mockReset();
    mocks.disable.mutate.mockReset();
    mocks.toastSuccess.mockReset();
    mocks.copyText.mockClear();
  });

  it('starts an enrollment from the disabled state', async () => {
    const { user } = renderDialog();

    expect(screen.getByText(/当前账号未启用两步验证/)).toBeInTheDocument();
    await user.click(screen.getByTestId('admin-mfa-setup'));
    expect(mocks.setup.mutate).toHaveBeenCalledTimes(1);
  });

  it('shows the one-time secret and confirms the enrollment with a live code', async () => {
    mocks.setup.data = PROVISIONING;
    mocks.confirm.mutate.mockImplementation((code, options) => {
      void code;
      options?.onSuccess?.();
    });

    const { user } = renderDialog();

    expect(screen.getByTestId('admin-mfa-secret')).toHaveTextContent(PROVISIONING.secret);
    // The QR code renders inside the portalled dialog surface.
    expect(screen.getByTestId('admin-mfa-dialog').querySelector('svg')).not.toBeNull();

    await user.type(screen.getByTestId('admin-mfa-confirm-code'), '123456');
    await user.click(screen.getByTestId('admin-mfa-confirm-submit'));

    expect(mocks.confirm.mutate).toHaveBeenCalledWith('123456', expect.any(Object));
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('两步验证已启用'));
  });

  it('copies the manual-entry secret', async () => {
    mocks.setup.data = PROVISIONING;
    const { user } = renderDialog();

    await user.click(screen.getByTestId('admin-mfa-secret'));
    expect(mocks.copyText).toHaveBeenCalledWith(PROVISIONING.secret);
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功'));
  });

  it('surfaces a wrong confirm code inline', async () => {
    mocks.setup.data = PROVISIONING;
    mocks.confirm.mutate.mockImplementation((code, options) => {
      void code;
      options?.onError?.(invalidCodeProblem());
    });

    const { user } = renderDialog();
    await user.type(screen.getByTestId('admin-mfa-confirm-code'), '000000');
    await user.click(screen.getByTestId('admin-mfa-confirm-submit'));

    expect(await screen.findByText('验证码错误或已被使用')).toBeInTheDocument();
  });

  it('disables an enabled factor with a live code', async () => {
    mocks.status.data = { totp_enabled: true, totp_enabled_at: '2023-11-14T22:13:20Z' };
    mocks.disable.mutate.mockImplementation((code, options) => {
      void code;
      options?.onSuccess?.();
    });

    const { user } = renderDialog();

    expect(screen.getByText(/两步验证已启用/)).toBeInTheDocument();
    await user.type(screen.getByTestId('admin-mfa-disable-code'), '654321');
    await user.click(screen.getByTestId('admin-mfa-disable-submit'));

    expect(mocks.disable.mutate).toHaveBeenCalledWith('654321', expect.any(Object));
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('两步验证已关闭'));
  });

  it('blocks the disable submit until a code is entered', () => {
    mocks.status.data = { totp_enabled: true, totp_enabled_at: null };
    renderDialog();

    expect(screen.getByTestId('admin-mfa-disable-submit')).toBeDisabled();
    expect(mocks.disable.mutate).not.toHaveBeenCalled();
  });
});
