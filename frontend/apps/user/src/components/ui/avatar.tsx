import { type ComponentProps, useState } from 'react';
import { cn } from '@/lib/cn';

function Avatar({ className, ...props }: ComponentProps<'span'>) {
  return (
    <span
      data-slot="avatar"
      className={cn('relative flex size-8 shrink-0 overflow-hidden rounded-full', className)}
      {...props}
    />
  );
}

function AvatarImage({ className, onError, ...props }: ComponentProps<'img'>) {
  const [failed, setFailed] = useState(false);
  if (failed || !props.src) return null;

  return (
    <img
      data-slot="avatar-image"
      className={cn('aspect-square size-full object-cover', className)}
      onError={(event) => {
        setFailed(true);
        onError?.(event);
      }}
      {...props}
    />
  );
}

function AvatarFallback({ className, ...props }: ComponentProps<'span'>) {
  return (
    <span
      data-slot="avatar-fallback"
      className={cn('flex size-full items-center justify-center rounded-full bg-muted', className)}
      {...props}
    />
  );
}

export { Avatar, AvatarFallback, AvatarImage };
