import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  type ChangeEvent,
  type InputHTMLAttributes,
  type ReactNode,
  type TextareaHTMLAttributes,
} from 'react';

type LegacyInputProps = InputHTMLAttributes<HTMLInputElement> & {
  legacyAttributeOrder?: 'placeholder-first' | 'type-first';
};

export const LegacyInput = forwardRef<HTMLInputElement, LegacyInputProps>(function LegacyInput(
  {
    className,
    defaultValue = '',
    legacyAttributeOrder,
    onChange,
    placeholder,
    style,
    type = 'text',
    ...rest
  }: LegacyInputProps,
  ref,
) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  useImperativeHandle(ref, () => inputRef.current as HTMLInputElement, []);

  useLayoutEffect(() => {
    const node = inputRef.current;
    if (!node) return;
    const placeholderAttr = node.getAttribute('placeholder');
    const typeAttr = node.getAttribute('type') ?? type;
    const classAttr = node.getAttribute('class');
    const valueAttr = node.getAttribute('value') ?? '';
    const styleAttr = node.getAttribute('style');

    node.removeAttribute('placeholder');
    node.removeAttribute('type');
    node.removeAttribute('class');
    node.removeAttribute('value');
    node.removeAttribute('style');
    if (legacyAttributeOrder === 'type-first') {
      node.setAttribute('type', typeAttr);
      if (placeholderAttr !== null) node.setAttribute('placeholder', placeholderAttr);
    } else {
      if (placeholderAttr !== null) node.setAttribute('placeholder', placeholderAttr);
      node.setAttribute('type', typeAttr);
    }
    if (classAttr) node.setAttribute('class', classAttr);
    node.setAttribute('value', valueAttr);
    if (styleAttr !== null) node.setAttribute('style', styleAttr);
  }, [legacyAttributeOrder, type]);

  return (
    <input
      ref={inputRef}
      placeholder={placeholder}
      type={type}
      className={className || undefined}
      defaultValue={defaultValue}
      style={style}
      onChange={onChange}
      {...rest}
    />
  );
});

export const LegacyCheckboxInput = forwardRef<
  HTMLInputElement,
  Omit<InputHTMLAttributes<HTMLInputElement>, 'type'>
>(function LegacyCheckboxInput({ className, value = '', ...rest }, ref) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  useImperativeHandle(ref, () => inputRef.current as HTMLInputElement, []);

  useLayoutEffect(() => {
    const node = inputRef.current;
    if (!node) return;

    const valueAttr = node.getAttribute('value') ?? '';
    node.removeAttribute('type');
    node.removeAttribute('class');
    node.removeAttribute('value');
    node.setAttribute('type', 'checkbox');
    if (className) node.setAttribute('class', className);
    node.setAttribute('value', valueAttr);
  }, [className]);

  return (
    <input
      ref={inputRef}
      type="checkbox"
      className={className || undefined}
      value={value}
      {...rest}
    />
  );
});

export const LegacyTextArea = forwardRef<
  HTMLTextAreaElement,
  TextareaHTMLAttributes<HTMLTextAreaElement>
>(function LegacyTextArea(
  { className, defaultValue = '', onChange, placeholder, rows, ...rest },
  ref,
) {
  const textAreaRef = useRef<HTMLTextAreaElement | null>(null);
  useImperativeHandle(ref, () => textAreaRef.current as HTMLTextAreaElement, []);

  return (
    <textarea
      ref={textAreaRef}
      rows={rows}
      placeholder={placeholder}
      className={className || undefined}
      defaultValue={defaultValue}
      onChange={onChange}
      {...rest}
    />
  );
});

export function LegacyInputGroup({
  addonAfter,
  className,
  defaultValue,
  onChange,
  placeholder,
  size,
  type = 'text',
  value,
}: {
  addonAfter: ReactNode;
  className?: string;
  defaultValue?: string | number | readonly string[] | undefined;
  onChange: (event: ChangeEvent<HTMLInputElement>) => void;
  placeholder?: string;
  size?: 'large';
  type?: HTMLInputElement['type'];
  value?: string | number | readonly string[] | undefined;
}) {
  const wrapperClassName =
    size === 'large'
      ? 'ant-input-group-wrapper ant-input-group-wrapper-lg'
      : 'ant-input-group-wrapper';
  const inputClassName = className ?? (size === 'large' ? 'ant-input ant-input-lg' : 'ant-input');

  return (
    <span className={wrapperClassName}>
      <span className="ant-input-wrapper ant-input-group">
        <LegacyInput
          placeholder={placeholder}
          type={type}
          className={inputClassName}
          defaultValue={defaultValue}
          value={value}
          legacyAttributeOrder={type === 'number' ? 'type-first' : undefined}
          onChange={onChange}
        />
        <span className="ant-input-group-addon">{addonAfter}</span>
      </span>
    </span>
  );
}
