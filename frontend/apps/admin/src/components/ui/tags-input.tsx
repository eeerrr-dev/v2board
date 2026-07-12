import { useState, type ComponentProps } from 'react';
import { X } from 'lucide-react';
import { Badge } from './badge';
import { cn } from '@/lib/cn';

interface TagsInputProps extends Omit<ComponentProps<'input'>, 'onChange' | 'value'> {
  value: string[];
  onChange: (value: string[]) => void;
  invalid?: boolean;
}

function TagsInput({
  value,
  onChange,
  onBlur,
  invalid,
  placeholder,
  className,
  disabled,
  ...inputProps
}: TagsInputProps) {
  const [draft, setDraft] = useState('');

  const commit = () => {
    const tag = draft.trim();
    setDraft('');
    if (tag && !value.includes(tag)) onChange([...value, tag]);
  };

  return (
    <div
      data-slot="tags-input"
      data-invalid={invalid || undefined}
      data-disabled={disabled || undefined}
      className={cn(
        'flex min-h-9 flex-wrap items-center gap-1.5 rounded-md border border-input bg-transparent px-2 py-1.5 text-sm shadow-xs transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/50 data-[disabled=true]:cursor-not-allowed data-[disabled=true]:opacity-50 data-[invalid=true]:border-destructive data-[invalid=true]:ring-destructive/20',
        className,
      )}
    >
      {value.map((tag) => (
        <Badge key={tag} variant="secondary" className="gap-1 pr-1">
          {tag}
          <button
            type="button"
            className="rounded-full text-muted-foreground outline-none hover:text-foreground focus-visible:ring-[2px] focus-visible:ring-ring/50"
            disabled={disabled}
            onClick={() => onChange(value.filter((item) => item !== tag))}
            aria-label={`移除标签 ${tag}`}
          >
            <X className="size-3" />
          </button>
        </Badge>
      ))}
      <input
        {...inputProps}
        disabled={disabled}
        aria-invalid={invalid || undefined}
        className="min-w-24 flex-1 bg-transparent outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed"
        value={draft}
        placeholder={value.length ? '' : placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={(event) => {
          commit();
          onBlur?.(event);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
            event.preventDefault();
            commit();
          } else if (
            event.key === 'Backspace' &&
            !event.nativeEvent.isComposing &&
            !draft &&
            value.length > 0
          ) {
            event.preventDefault();
            onChange(value.slice(0, -1));
          }
          inputProps.onKeyDown?.(event);
        }}
      />
    </div>
  );
}

export { TagsInput, type TagsInputProps };
