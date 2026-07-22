import type * as ApiClientModule from '@v2board/api-client';
import { act, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, ApiProblemError } from '@v2board/api-client';
import { renderWithProviders } from '@/test/render';
import { StepUpDialogProvider } from './step-up-dialog';
import {
  clearStepUpGrant,
  getStepUpToken,
  maybePromptStepUp,
  resolveStepUpPrompt,
} from '@/lib/step-up';

const stepUpMock = vi.hoisted(() => vi.fn());
vi.mock('@v2board/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof ApiClientModule>();
  return { ...actual, passport: { ...actual.passport, stepUp: stepUpMock } };
});

const toastSuccess = vi.hoisted(() => vi.fn());
vi.mock('@v2board/app-shell/toast', () => ({ toast: { success: toastSuccess, error: vi.fn() } }));

const STEP_UP_403 = new ApiProblemError(403, {
  type: 'about:blank',
  title: 'Forbidden',
  status: 403,
  code: 'step_up_required',
  detail: 'Recent password verification is required',
});
const PERMISSION_403 = new ApiProblemError(403, {
  type: 'about:blank',
  title: 'Forbidden',
  status: 403,
  code: 'permission_denied',
  detail: 'Permission denied',
});

describe('step-up re-auth dialog', () => {
  afterEach(() => {
    act(() => resolveStepUpPrompt());
    clearStepUpGrant();
    stepUpMock.mockReset();
    toastSuccess.mockReset();
  });

  it('opens only for the step_up_required problem and stores the grant on success', async () => {
    stepUpMock.mockResolvedValue({ step_up_token: 'grant-token', expires_in: 900 });
    const { user } = renderWithProviders(<StepUpDialogProvider />, { queryClient: true });

    // Code-keyed discrimination (§3.1): neither the permission verdict nor a
    // legacy message-shaped 403 opens the re-auth dialog.
    expect(maybePromptStepUp(PERMISSION_403)).toBe(false);
    expect(maybePromptStepUp(new ApiError(403, 'Recent password verification is required'))).toBe(
      false,
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    act(() => {
      expect(maybePromptStepUp(STEP_UP_403)).toBe(true);
    });
    const dialog = await screen.findByRole('dialog', { name: '验证管理员密码' });
    expect(dialog).toBeInTheDocument();

    await user.type(screen.getByLabelText('当前密码'), 'hunter2');
    await user.click(screen.getByRole('button', { name: '验证' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(stepUpMock).toHaveBeenCalledWith(expect.anything(), { password: 'hunter2' });
    expect(getStepUpToken()).toBe('grant-token');
    expect(toastSuccess).toHaveBeenCalledOnce();
  });

  it('reopens clean after a close that raced an in-flight rejection', async () => {
    // Radix still closes on Escape while the verify round-trip is pending;
    // the late rejection must not leak its error into the next prompt.
    let rejectSubmit!: (error: unknown) => void;
    stepUpMock.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectSubmit = reject;
        }),
    );
    const { user } = renderWithProviders(<StepUpDialogProvider />, { queryClient: true });

    act(() => {
      maybePromptStepUp(STEP_UP_403);
    });
    await screen.findByRole('dialog', { name: '验证管理员密码' });
    await user.type(screen.getByLabelText('当前密码'), 'wrong');
    await user.click(screen.getByRole('button', { name: '验证' }));

    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    await act(async () => {
      rejectSubmit(new ApiError(500, 'Incorrect email or password'));
      await Promise.resolve();
    });

    act(() => {
      maybePromptStepUp(STEP_UP_403);
    });
    await screen.findByRole('dialog', { name: '验证管理员密码' });
    expect(screen.queryByText('Incorrect email or password')).not.toBeInTheDocument();
    expect(screen.getByLabelText('当前密码')).toHaveValue('');
  });

  it('keeps the dialog open with the problem detail on a wrong password', async () => {
    // Post-W2 wire: a wrong step-up password is a 400 invalid_credentials
    // problem; its localized detail renders in place of the legacy message.
    stepUpMock.mockRejectedValue(
      new ApiProblemError(400, {
        type: 'about:blank',
        title: 'Bad Request',
        status: 400,
        code: 'invalid_credentials',
        detail: 'Incorrect email or password',
      }),
    );
    const { user } = renderWithProviders(<StepUpDialogProvider />, { queryClient: true });

    act(() => {
      maybePromptStepUp(STEP_UP_403);
    });
    await screen.findByRole('dialog', { name: '验证管理员密码' });
    await user.type(screen.getByLabelText('当前密码'), 'wrong');
    await user.click(screen.getByRole('button', { name: '验证' }));

    expect(await screen.findByText('Incorrect email or password')).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: '验证管理员密码' })).toBeInTheDocument();
    expect(getStepUpToken()).toBeNull();
  });
});
