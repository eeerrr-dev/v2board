import { cn } from '@/lib/cn';

interface PlanContentProps {
  content: string;
  className?: string;
  htmlClassName?: string;
}

export function PlanContent({ content, className, htmlClassName }: PlanContentProps) {
  const features = parseFeatures(content);
  if (!features) {
    return (
      <div
        className={cn(htmlClassName ?? className)}
        dangerouslySetInnerHTML={{ __html: content }}
      />
    );
  }

  return (
    <div className={cn(className)}>
      {features.map((item, index) => {
        const supported = Boolean(item.support);
        return (
          <div
            key={`${item.feature}-${index}`}
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
            <span style={{ paddingLeft: 8 }}>{item.feature}</span>
          </div>
        );
      })}
    </div>
  );
}

function parseFeatures(content: string): Array<{ feature: string; support?: boolean }> | null {
  try {
    const parsed = JSON.parse(content) as unknown;
    if (
      Array.isArray(parsed) &&
      parsed.every((item) => {
        return (
          item &&
          typeof item === 'object' &&
          'feature' in item &&
          typeof (item as { feature?: unknown }).feature === 'string'
        );
      })
    ) {
      return parsed as Array<{ feature: string; support?: boolean }>;
    }
  } catch {}
  return null;
}
