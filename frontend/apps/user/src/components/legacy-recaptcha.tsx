import { useCallback, useEffect, useRef, useState } from 'react';
import { Dialog, DialogContent } from '@/components/ui/dialog';

interface RecaptchaApi {
  render: (
    container: HTMLElement,
    options: {
      sitekey: string;
      callback: (token: string) => void;
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

function loadRecaptcha() {
  if (window.grecaptcha?.render) return Promise.resolve(window.grecaptcha);
  if (recaptchaPromise) return recaptchaPromise;

  recaptchaPromise = new Promise((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>('script[data-v2board-recaptcha]');
    if (existing) {
      existing.addEventListener('load', () => {
        if (window.grecaptcha?.render) resolve(window.grecaptcha);
        else reject(new Error('reCAPTCHA is unavailable'));
      });
      existing.addEventListener('error', () => reject(new Error('reCAPTCHA failed to load')));
      return;
    }

    const script = document.createElement('script');
    const resolveRecaptcha = () => {
      if (window.grecaptcha?.render) resolve(window.grecaptcha);
      else reject(new Error('reCAPTCHA is unavailable'));
    };
    window.onloadcallback = resolveRecaptcha;
    script.src = 'https://www.recaptcha.net/recaptcha/api.js?onload=onloadcallback&render=explicit';
    script.async = true;
    script.defer = true;
    script.dataset.v2boardRecaptcha = 'true';
    script.onload = resolveRecaptcha;
    script.onerror = () => reject(new Error('reCAPTCHA failed to load'));
    document.head.appendChild(script);
  });

  return recaptchaPromise;
}

export function useLegacyRecaptcha(enabled: boolean, siteKey?: string | null) {
  const [open, setOpen] = useState(false);
  const [failed, setFailed] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const widgetIdRef = useRef<number | null>(null);
  const actionRef = useRef<ProtectedAction | null>(null);

  const close = useCallback(() => {
    setOpen(false);
    setFailed(false);
    actionRef.current = null;
  }, []);

  const run = useCallback(
    (action: ProtectedAction) => {
      if (!enabled) {
        void action();
        return;
      }
      actionRef.current = action;
      setFailed(false);
      setOpen(true);
    },
    [enabled],
  );

  const handleToken = useCallback(
    (token: string) => {
      window.setTimeout(() => {
        const action = actionRef.current;
        setOpen(false);
        setFailed(false);
        actionRef.current = null;
        if (action) void action(token);
      }, 500);
    },
    [],
  );

  useEffect(() => {
    if (!open || !enabled || !siteKey) return;

    let cancelled = false;
    setFailed(false);
    loadRecaptcha()
      .then((grecaptcha) => {
        if (cancelled || !containerRef.current) return;
        containerRef.current.innerHTML = '';
        widgetIdRef.current = grecaptcha.render(containerRef.current, {
          sitekey: siteKey,
          callback: handleToken,
          'expired-callback': () => {
            if (widgetIdRef.current !== null) grecaptcha.reset(widgetIdRef.current);
          },
          'error-callback': () => setFailed(true),
        });
      })
      .catch(() => setFailed(true));

    return () => {
      cancelled = true;
    };
  }, [enabled, handleToken, open, siteKey]);

  const recaptchaModal = (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) close();
      }}
    >
      <DialogContent
        showClose={false}
        centered
        className="v2board-ant-modal v2board-ant-recaptcha-modal"
      >
        <div className="ant-modal-body">
          <div className="v2board-recaptcha-box">
            {siteKey && !failed ? (
              <div ref={containerRef} />
            ) : (
              <div className="spinner-grow text-primary" role="status">
                <span className="sr-only">Loading...</span>
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );

  return { run, recaptchaModal };
}
