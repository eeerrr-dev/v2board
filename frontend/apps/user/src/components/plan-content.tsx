import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

interface PlanContentProps {
  content?: string | null;
  className?: string;
  htmlClassName?: string;
  guardNull?: boolean;
}

export function PlanContent({ content, className, htmlClassName, guardNull = false }: PlanContentProps) {
  // The original parses the plan content as JSON, then maps it as a feature list.
  // The plan-list page gates ONLY on `typeof parsed === 'object'`, so a JSON
  // `null` (typeof 'object') enters the map branch and `null.map` throws. The
  // checkout page instead gates on `g && typeof g === 'object'`, so a JSON `null`
  // falls back to raw HTML — `guardNull` selects that variant. In both cases a
  // parse failure or a plain `{}` behaves exactly as the original does.
  let parsed: unknown;
  let parseFailed = false;
  try {
    parsed = JSON.parse(content as string);
  } catch {
    parseFailed = true;
  }

  const isFeatureList = guardNull
    ? parsed != null && typeof parsed === 'object'
    : typeof parsed === 'object';
  if (parseFailed || !isFeatureList) {
    return (
      <div
        className={cn(htmlClassName ?? className)}
        dangerouslySetInnerHTML={{ __html: content as string }}
      />
    );
  }

  const features = parsed as Array<{ feature?: unknown; support?: unknown }>;
  return (
    <div className={cn(className)}>
      {features.map((item, index) => {
        const supported = Boolean(item.support);
        return (
          <div
            key={index}
            style={{
              textAlign: 'left',
              marginBottom: 8,
              opacity: supported ? 1 : 0.3,
            }}
          >
            <i
              className={`si ${supported ? 'si-check' : 'si-close'} text-primary`}
              style={{ fontSize: 21, verticalAlign: 'sub' }}
            />
            <span style={{ paddingLeft: 8 }}>{item.feature as ReactNode}</span>
          </div>
        );
      })}
    </div>
  );
}
