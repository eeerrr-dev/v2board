import { useCallback, useEffect, useRef, useState } from 'react';
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';
import { i18nGet } from '@/lib/errors';
import { toast } from '@/lib/toast';

interface RecaptchaApi {
  render: (
    container: HTMLElement,
    options: {
      sitekey?: string | null;
      callback: (token: string) => void;
      theme?: 'light';
      type?: 'image';
      tabindex?: 0;
      size?: 'normal';
      badge?: 'bottomright';
      'expired-callback'?: () => void;
      'error-callback'?: () => void;
    },
  ) => number;
  reset: (widgetId?: number) => void;
}

declare global {
  interface Window {
    grecaptcha?: RecaptchaApi;
    onloadcallback?: () => void;
  }
}

type ProtectedAction = (recaptchaData?: string) => void | Promise<void>;

let recaptchaPromise: Promise<RecaptchaApi> | null = null;
const RECAPTCHA_SCRIPT_URL =
  'https://www.recaptcha.net/recaptcha/api.js?onload=onloadcallback&render=explicit';

function loadRecaptcha() {
  if (window.grecaptcha?.render) return Promise.resolve(window.grecaptcha);
  if (recaptchaPromise) return recaptchaPromise;

  const promise = new Promise<RecaptchaApi>((resolve, reject) => {
    const fail = (script: HTMLScriptElement | null, message: string) => {
      // Drop the failed <script> so a later attempt installs a fresh one instead of
      // re-attaching to a tag that already errored (and will never fire `load`).
      script?.remove();
      delete window.onloadcallback;
      reject(new Error(message));
    };
    const existing = document.querySelector<HTMLScriptElement>(
      `script[src="${RECAPTCHA_SCRIPT_URL}"]`,
    );
    if (existing) {
      existing.addEventListener('load', () => {
        if (window.grecaptcha?.render) resolve(window.grecaptcha);
        else fail(existing, 'reCAPTCHA is unavailable');
      });
      existing.addEventListener('error', () => fail(existing, 'reCAPTCHA failed to load'));
      return;
    }

    const script = document.createElement('script');
    const resolveRecaptcha = () => {
      if (window.grecaptcha?.render) {
        delete window.onloadcallback;
        resolve(window.grecaptcha);
      } else {
        fail(script, 'reCAPTCHA is unavailable');
      }
    };
    window.onloadcallback = resolveRecaptcha;
    script.src = RECAPTCHA_SCRIPT_URL;
    script.async = true;
    script.onerror = () => fail(script, 'reCAPTCHA failed to load');
    document.body.appendChild(script);
  }).catch((error: unknown) => {
    // A failed load must not poison the module singleton for the tab's lifetime —
    // release it so the next gated action retries the load instead of reusing the
    // cached rejection (which left the dialog blank with no recovery path).
    recaptchaPromise = null;
    throw error;
  });

  recaptchaPromise = promise;
  return promise;
}

export function useAuthRecaptcha(enabled: boolean, siteKey?: string | null) {
  const [open, setOpen] = useState(false);
  const [widgetKey, setWidgetKey] = useState(0);
  const actionRef = useRef<ProtectedAction | null>(null);

  // Plain handler: only ever invoked from the Dialog onOpenChange closure, so its
  // identity is never read and the manual useCallback was compiler-redundant residue.
  const cancel = () => {
    setOpen(false);
    actionRef.current = null;
  };

  // Plain handler: callers invoke run() from event closures and never read its
  // identity, so the manual useCallback ceremony added nothing.
  const run = (action: ProtectedAction) => {
    if (!enabled) {
      void action();
      return;
    }
    actionRef.current = action;
    setWidgetKey((value) => value + 1);
    setOpen(true);
  };

  const handleToken = useCallback((token: string | null) => {
    const action = actionRef.current;
    setOpen(false);
    actionRef.current = null;
    if (action) void action(token ?? undefined);
  }, []);

  const handleWidgetError = useCallback((error: unknown) => {
    setOpen(false);
    actionRef.current = null;
    toast.error(i18nGet('请求失败'), {
      description: error instanceof Error ? error.message : i18nGet('请求失败'),
    });
  }, []);

  const recaptchaModal = (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) cancel();
      }}
    >
      <DialogContent
        key={widgetKey}
        aria-describedby={undefined}
        className="w-fit max-w-[calc(100vw-2rem)] p-6"
        showCloseButton={false}
      >
        <DialogTitle className="sr-only">reCAPTCHA</DialogTitle>
        {enabled ? (
          <AuthRecaptchaWidget
            siteKey={siteKey}
            onToken={handleToken}
            onError={handleWidgetError}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  );

  return { run, recaptchaModal };
}

function AuthRecaptchaWidget({
  siteKey,
  onToken,
  onError,
}: {
  siteKey?: string | null;
  onToken: (token: string | null) => void;
  onError: (error: unknown) => void;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    let grecaptchaApi: RecaptchaApi | null = null;
    let widgetId: number | undefined;

    loadRecaptcha()
      .then((grecaptcha) => {
        if (cancelled || !containerRef.current) return;
        grecaptchaApi = grecaptcha;
        const renderTarget = document.createElement('div');
        containerRef.current.appendChild(renderTarget);
        widgetId = grecaptcha.render(renderTarget, {
          sitekey: siteKey,
          callback: onToken,
          theme: 'light',
          type: 'image',
          tabindex: 0,
          size: 'normal',
          badge: 'bottomright',
          'expired-callback': () => onToken(null),
          'error-callback': () => onError(new Error('reCAPTCHA verification failed')),
        });
      })
      .catch((error: unknown) => {
        if (!cancelled) onError(error);
      });

    return () => {
      cancelled = true;
      if (grecaptchaApi && widgetId !== undefined) {
        grecaptchaApi.reset(widgetId);
      }
    };
  }, [onError, onToken, siteKey]);

  return <div ref={containerRef} />;
}
