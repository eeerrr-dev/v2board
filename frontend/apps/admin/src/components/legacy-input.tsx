import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  type InputHTMLAttributes,
} from 'react';

export const LegacyInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function LegacyInput(
    { className, defaultValue = '', onChange, placeholder, style, type = 'text', ...rest },
    ref,
  ) {
    const inputRef = useRef<HTMLInputElement | null>(null);
    useImperativeHandle(ref, () => inputRef.current as HTMLInputElement, []);

    useLayoutEffect(() => {
      const node = inputRef.current;
      if (!node) return;
      const styleAttr = node.getAttribute('style');
      if (styleAttr === null) return;

      const valueAttr = node.getAttribute('value') ?? '';
      node.removeAttribute('value');
      node.removeAttribute('style');
      node.setAttribute('value', valueAttr);
      node.setAttribute('style', styleAttr);
    }, []);

    return (
      <input
        ref={inputRef}
        placeholder={placeholder}
        className={className || undefined}
        type={type}
        defaultValue={defaultValue}
        style={style}
        onChange={onChange}
        {...rest}
      />
    );
  },
);

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
