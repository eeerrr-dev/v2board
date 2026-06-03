import { useCallback, useEffect, useRef, useState } from 'react';
import { Dialog, DialogContent } from '@/components/ui/dialog';

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
const RECAPTCHA_SCRIPT_URL = 'https://www.recaptcha.net/recaptcha/api.js?onload=onloadcallback&render=explicit';

function loadRecaptcha() {
  if (window.grecaptcha?.render) return Promise.resolve(window.grecaptcha);
  if (recaptchaPromise) return recaptchaPromise;

  recaptchaPromise = new Promise((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>(
      `script[src="${RECAPTCHA_SCRIPT_URL}"]`,
    );
    if (existing) {
      existing.addEventListener('load', () => {
        if (window.grecaptcha?.render) resolve(window.grecaptcha);
        else reject(new Error('reCAPTCHA is unavailable'));
      });
      existing.addEventListener('error', () => reject(new Error('reCAPTCHA failed to load')));
      return;
    }

    const resolveRecaptcha = () => {
      if (window.grecaptcha?.render) {
        delete window.onloadcallback;
        resolve(window.grecaptcha);
      } else {
        reject(new Error('reCAPTCHA is unavailable'));
      }
    };
    const script = document.createElement('script');
    window.onloadcallback = resolveRecaptcha;
    script.src = RECAPTCHA_SCRIPT_URL;
    script.async = true;
    script.onerror = () => reject(new Error('reCAPTCHA failed to load'));
    document.body.appendChild(script);
  });

  return recaptchaPromise;
}

export function useLegacyRecaptcha(enabled: boolean, siteKey?: string | null) {
  const [open, setOpen] = useState(false);
  const [widgetKey, setWidgetKey] = useState(0);
  const actionRef = useRef<ProtectedAction | null>(null);

  const cancel = useCallback(() => {
    setOpen(false);
    actionRef.current = null;
  }, []);

  const run = useCallback(
    (action: ProtectedAction) => {
      if (!enabled) {
        void action();
        return;
      }
      actionRef.current = action;
      setWidgetKey(Math.random());
      setOpen(true);
    },
    [enabled],
  );

  const handleToken = useCallback((token: string | null) => {
    // The original's handle(e) runs for BOTH a fresh token and the expired
    // callback's null: after 500ms it closes the modal and invokes the action.
    // A null token omits recaptcha_data from the request (a && (l.recaptcha_data = a)).
    window.setTimeout(() => {
      const action = actionRef.current;
      setOpen(false);
      actionRef.current = null;
      if (action) void action(token ?? undefined);
    }, 500);
  }, []);

  const recaptchaModal = (
    // The old wrapper passes onCancel: hide(), so mask clicks and Esc close the modal
    // without invoking the protected action.
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) cancel();
      }}
    >
      <DialogContent key={widgetKey} closable={false} footer={null} centered>
        {enabled ? <LegacyRecaptchaWidget siteKey={siteKey} onToken={handleToken} /> : null}
      </DialogContent>
    </Dialog>
  );

  return { run, recaptchaModal };
}

function LegacyRecaptchaWidget({
  siteKey,
  onToken,
}: {
  siteKey?: string | null;
  onToken: (token: string | null) => void;
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
          'error-callback': () => {},
        });
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      if (grecaptchaApi && widgetId !== undefined) {
        if (containerRef.current) delayCaptchaIframeRemoving(containerRef.current);
        grecaptchaApi.reset(widgetId);
      }
    };
  }, [onToken, siteKey]);

  return <div ref={containerRef} />;
}

function delayCaptchaIframeRemoving(captcha: HTMLElement): void {
  const detached = document.createElement('div');
  document.body.appendChild(detached);
  detached.style.display = 'none';
  while (captcha.firstChild) detached.appendChild(captcha.firstChild);
  window.setTimeout(() => {
    document.body.removeChild(detached);
  }, 5000);
}
