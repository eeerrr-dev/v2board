import type { ReactNode } from 'react';
import { Check, X } from 'lucide-react';
import { cn } from '@/lib/cn';
import { sanitizeLegacyHtml } from '@/lib/sanitize-html';

interface PlanContentProps {
  content?: string | null;
  className?: string;
  htmlClassName?: string;
}

export function PlanContent({ content, className, htmlClassName }: PlanContentProps) {
  let parsed: unknown;
  let parseFailed = false;
  try {
    parsed = JSON.parse(content as string);
  } catch {
    parseFailed = true;
  }

  const isFeatureList = Array.isArray(parsed);
  if (parseFailed || !isFeatureList) {
    return (
      <div
        className={cn(htmlClassName ?? className)}
        dangerouslySetInnerHTML={{ __html: sanitizeLegacyHtml(content as string) }}
      />
    );
  }

  const features = parsed as Array<{ feature?: unknown; support?: unknown }>;
  return (
    <div className={cn('grid gap-2.5 text-sm', className)}>
      {features.map((item, index) => {
        const supported = Boolean(item.support);
        const Icon = supported ? Check : X;
        return (
          <div
            key={index}
            className={cn(
              'flex items-start gap-2 text-left leading-5',
              supported ? 'text-foreground' : 'text-muted-foreground opacity-70',
            )}
          >
            <Icon
              className={cn(
                'mt-0.5 size-4 shrink-0',
                supported ? 'text-primary' : 'text-muted-foreground',
              )}
            />
            <span>{item.feature as ReactNode}</span>
          </div>
        );
      })}
    </div>
  );
}
