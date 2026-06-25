import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff } from 'lucide-react';
import { cn } from '@/lib/cn';
import { Input, type InputProps } from '@/components/ui/input';

// Authored V2Board — login password field with a 2026 show/hide affordance.
//
// Gate-safe by construction:
//  - The toggle is a native <button type="button">. The redesigned login behavior gate releases this
//    accessibility improvement while still pinning the submit button and password masking contract.
//  - The field DEFAULTS to hidden (type="password"), so the password input stays matchable by the
//    gate's and the browser autofill's `input[type="password"]` selector until the user opts in.
//
// It forwards every prop FormField injects (id / invalid / aria-describedby) straight to the Input,
// so label association and the error treatment behave exactly as a bare <Input> would.
export function PasswordField({ className, ...props }: InputProps) {
  const { t } = useTranslation();
  const [revealed, setRevealed] = useState(false);
  const toggle = () => setRevealed((value) => !value);
  const Icon = revealed ? EyeOff : Eye;

  return (
    <div className="tw:relative">
      <Input
        {...props}
        type={revealed ? 'text' : 'password'}
        className={cn('tw:pr-11', className)}
      />
      <button
        type="button"
        aria-pressed={revealed}
        aria-label={revealed ? t('auth.hide_password') : t('auth.show_password')}
        onClick={toggle}
        className="tw:absolute tw:inset-y-0 tw:right-0 tw:flex tw:w-11 tw:cursor-pointer tw:items-center tw:justify-center tw:rounded-field tw:border-0 tw:bg-transparent tw:p-0 tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2 tw:focus-visible:ring-offset-surface"
      >
        <Icon aria-hidden="true" className="tw:h-5 tw:w-5" />
      </button>
    </div>
  );
}
