import { useState, type KeyboardEvent, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { Input, type InputProps } from '@/components/ui/input';

// Authored V2Board — login password field with a 2026 show/hide affordance.
//
// Gate-safe by construction:
//  - The toggle is a keyboard-operable <span role="button">, NOT a native <button>/.btn, so it never
//    enters the behavior gate's page-wide `button, .btn` capture (no second button appears).
//  - The field DEFAULTS to hidden (type="password"), so the password input stays matchable by the
//    gate's and the browser autofill's `input[type="password"]` selector until the user opts in.
//
// It forwards every prop FormField injects (id / invalid / aria-describedby) straight to the Input,
// so label association and the error treatment behave exactly as a bare <Input> would.
export function PasswordField({ className, ...props }: InputProps) {
  const { t } = useTranslation();
  const [revealed, setRevealed] = useState(false);
  const toggle = () => setRevealed((value) => !value);
  const onToggleKeyDown = (event: KeyboardEvent<HTMLSpanElement>) => {
    if (event.key === 'Enter' || event.key === ' ' || event.key === 'Spacebar') {
      event.preventDefault();
      toggle();
    }
  };

  return (
    <div className="tw:relative">
      <Input
        {...props}
        type={revealed ? 'text' : 'password'}
        className={cn('tw:pr-11', className)}
      />
      <span
        role="button"
        tabIndex={0}
        aria-pressed={revealed}
        aria-label={revealed ? t('auth.hide_password') : t('auth.show_password')}
        onClick={toggle}
        onKeyDown={onToggleKeyDown}
        className="tw:absolute tw:inset-y-0 tw:right-0 tw:flex tw:w-11 tw:cursor-pointer tw:items-center tw:justify-center tw:rounded-field tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2 tw:focus-visible:ring-offset-surface"
      >
        {revealed ? <EyeOffIcon /> : <EyeIcon />}
      </span>
    </div>
  );
}

function PasswordIcon({ children }: { children: ReactNode }) {
  return (
    <svg
      aria-hidden="true"
      className="tw:h-5 tw:w-5"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {children}
    </svg>
  );
}

function EyeIcon() {
  return (
    <PasswordIcon>
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
      <circle cx="12" cy="12" r="3" />
    </PasswordIcon>
  );
}

function EyeOffIcon() {
  return (
    <PasswordIcon>
      <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
      <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
      <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
      <line x1="1" y1="1" x2="23" y2="23" />
    </PasswordIcon>
  );
}
