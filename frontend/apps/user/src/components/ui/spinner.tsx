import { cn } from '@/lib/cn';

// Authored V2Board — clean-modern reskin primitive.
export function Spinner({ className }: { className?: string }) {
  return (
    <svg
      className={cn('tw:size-4 tw:animate-spin', className)}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <circle className="tw:opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
      <path
        className="tw:opacity-90"
        d="M12 2a10 10 0 0 1 10 10"
        stroke="currentColor"
        strokeWidth="3"
        strokeLinecap="round"
      />
    </svg>
  );
}
