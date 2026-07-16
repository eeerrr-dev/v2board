import { useEffect, type BaseSyntheticEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import type { AuthData } from '@v2board/types';
import { getErrorPresentation } from '@v2board/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, useFormState, type UseFormRegister } from 'react-hook-form';
import { normalizeLoginRedirectTarget, setAuthData } from '@/lib/auth';
import { i18nGet } from '@/lib/errors';
import { useLoginMutation, useTokenLoginMutation } from '@/lib/guest';
import { userQueryOptions } from '@/lib/queries';
import { loginSchema, type LoginFormInput, type LoginFormValues } from './auth-validation';

// A one-time verify token (the backend-emailed `?verify=` handoff) must be redeemed
// exactly once. React 19 StrictMode double-invokes the bootstrap effect in dev
// (mount → cleanup → mount), which would POST token2Login twice and fail the surviving
// call against the already-consumed token. De-dupe by verify value at module scope: the
// first effect run owns the in-flight redemption promise, a doubled run re-attaches to
// it (so the surviving mount still finishes login via its own `active` guard), and the
// entry is released once the request settles so a fresh visit re-redeems. This mirrors
// the file's existing race guards without touching the request or redirect contract.
const pendingVerifyRedemptions = new Map<string, Promise<AuthData | null>>();

export interface LoginController {
  registerInput: UseFormRegister<LoginFormInput>;
  submit: (event?: BaseSyntheticEvent) => Promise<void>;
  /** Dismisses the inline error (e.g. once the user edits the form). */
  clearError: () => void;
  isPending: boolean;
  error: string | null;
  emailError?: string;
  passwordError?: string;
}

// Authored V2Board — login behavior controller. Owns the auth mutations, one-time token2Login
// redemption, and the submit flow. Existing-session probing belongs to the /login route loader;
// the request/redirect contract here keeps the backend payloads and eager user-info prefetch.
export function useLoginController(): LoginController {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const queryClient = useQueryClient();
  const { mutateAsync, isPending } = useLoginMutation();
  const { mutateAsync: tokenLogin } = useTokenLoginMutation();
  const form = useForm<LoginFormInput, unknown, LoginFormValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { email: '', password: '' },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler caches
  // proxy reads, which would freeze these derived errors after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  // Server-side login failures live in react-hook-form's reserved `root` error namespace rather
  // than a parallel useState, so the inline alert is the single source of truth for this surface.
  const error = formErrors.root?.serverError?.message ?? null;

  const queryRedirect = params.get('redirect');
  const redirect = normalizeLoginRedirectTarget(queryRedirect);
  const verify = params.get('verify');

  // React Compiler keeps this stable; handleSubmit rewraps it each render anyway.
  const login = async (values: LoginFormValues) => {
    form.clearErrors('root.serverError');
    // Keep the network boundary independently guarded even though handleSubmit
    // already resolves through the same schema.
    const validated = loginSchema.safeParse(values);
    if (!validated.success) return;
    const { email, password } = validated.data;
    try {
      const result = await mutateAsync({ email, password });
      setAuthData(result.auth_data);
      // The saga dispatched user/getUserInfo with `put`, then immediately pushed — it never
      // waited for the user-info request to settle.
      void queryClient.prefetchQuery(userQueryOptions.info());
      void navigate(redirect);
    } catch (err) {
      const presentation = getErrorPresentation(err);
      // API teardown already owns a 403 and navigates to login. Do not race that
      // redirect with stale inline feedback on the page being discarded.
      if (presentation.status === 403) return;
      form.setError('root.serverError', {
        message: (err instanceof Error && err.message) || i18nGet('请求失败'),
      });
    }
  };
  const submit = form.handleSubmit(login);

  useEffect(() => {
    if (!verify) return;
    let active = true;

    const finishLogin = (authData: string) => {
      if (!active) return;
      setAuthData(authData);
      void navigate(redirect);
    };

    // Redeem this verify token at most once per value (see pendingVerifyRedemptions):
    // the first run creates the request, a StrictMode-doubled run re-attaches to the
    // same promise instead of re-POSTing the one-time token.
    let redemption = pendingVerifyRedemptions.get(verify);
    if (!redemption) {
      redemption = tokenLogin({
        verify,
        ...(queryRedirect !== null ? { redirect: queryRedirect } : {}),
      });
      pendingVerifyRedemptions.set(verify, redemption);
      void redemption.then(
        () => pendingVerifyRedemptions.delete(verify),
        () => pendingVerifyRedemptions.delete(verify),
      );
    }
    void redemption.then(
      (result) => {
        if (result?.auth_data) finishLogin(result.auth_data);
      },
      (error: unknown) => {
        // MutationCache presents the token redemption failure. This branch only
        // terminates the detached effect promise without duplicating that toast.
        void error;
      },
    );

    return () => {
      active = false;
    };
  }, [navigate, queryRedirect, redirect, tokenLogin, verify]);

  // RHF only auto-clears root errors on the next submit, so the form-level onInput keeps wiring here
  // to dismiss the alert the moment the user edits a field without resubmitting.
  const clearError = () => form.clearErrors('root.serverError');

  return {
    registerInput: form.register,
    submit,
    clearError,
    isPending,
    error,
    emailError: formErrors.email?.message,
    passwordError: formErrors.password?.message,
  };
}
