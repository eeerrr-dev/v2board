import { Check, X } from 'lucide-react';
import { z } from 'zod';
import { cn } from '@/lib/cn';
import { sanitizeBackendHtml } from '@/lib/sanitize-html';

interface PlanContentProps {
  content?: string | null;
  className?: string;
  htmlClassName?: string;
}

const planFeatureListSchema = z.array(
  z.object({
    feature: z.union([z.string(), z.number()]),
    // The backend `support` field uses JavaScript truthiness. Keep that behavior
    // for JSON scalars while rejecting objects and arrays.
    support: z.union([z.boolean(), z.number(), z.string(), z.null()]).optional(),
  }),
);

export function PlanContent({ content, className, htmlClassName }: PlanContentProps) {
  const rawContent = content ?? '';
  let parsed: unknown;
  try {
    parsed = JSON.parse(rawContent);
  } catch {
    parsed = undefined;
  }

  const featureList = planFeatureListSchema.safeParse(parsed);
  if (!featureList.success) {
    return (
      <div
        className={cn(htmlClassName ?? className)}
        // eslint-disable-next-line @eslint-react/dom-no-dangerously-set-innerhtml -- backend HTML sanitized by sanitizeBackendHtml
        dangerouslySetInnerHTML={{ __html: sanitizeBackendHtml(rawContent) }}
      />
    );
  }

  return (
    <div className={cn('grid gap-2.5 text-sm', className)}>
      {featureList.data.map((item, index) => {
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
            <span>{item.feature}</span>
          </div>
        );
      })}
    </div>
  );
}
