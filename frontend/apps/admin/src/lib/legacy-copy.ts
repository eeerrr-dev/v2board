import { message } from 'antd';

export function legacyCopyText(text: string | number | null | undefined): boolean {
  const value = String(text);
  let mark: HTMLSpanElement | null = null;
  let range: Range | null = null;
  let copied = false;
  const restoreSelection = deselectCurrentSelection();

  try {
    range = document.createRange();
    const selection = document.getSelection();
    mark = document.createElement('span');
    mark.textContent = value;
    mark.ariaHidden = 'true';
    mark.style.all = 'unset';
    mark.style.position = 'fixed';
    mark.style.top = '0';
    mark.style.clip = 'rect(0, 0, 0, 0)';
    mark.style.whiteSpace = 'pre';
    mark.style.webkitUserSelect = 'text';
    (mark.style as CSSStyleDeclaration & { MozUserSelect?: string }).MozUserSelect = 'text';
    (mark.style as CSSStyleDeclaration & { msUserSelect?: string }).msUserSelect = 'text';
    mark.style.userSelect = 'text';
    mark.addEventListener('copy', (event) => {
      event.stopPropagation();
    });

    document.body.appendChild(mark);
    range.selectNodeContents(mark);
    selection?.addRange(range);
    if (!document.execCommand('copy')) throw new Error('copy command was unsuccessful');
    copied = true;
  } catch {
    const clipboardData = (window as unknown as {
      clipboardData?: { setData: (format: string, value: string) => void };
    }).clipboardData;
    try {
      clipboardData?.setData('text', value);
      copied = Boolean(clipboardData);
      if (!clipboardData) throw new Error('clipboardData unavailable');
    } catch {
      const key = /mac os x/i.test(navigator.userAgent) ? '\u2318+C' : 'Ctrl+C';
      window.prompt(`Copy to clipboard: ${key}, Enter`, value);
    }
  } finally {
    const selection = document.getSelection();
    if (selection && range) {
      if (typeof selection.removeRange === 'function') selection.removeRange(range);
      else selection.removeAllRanges();
    }
    if (mark) document.body.removeChild(mark);
    restoreSelection();
  }

  message.success('复制成功');
  return copied;
}

function deselectCurrentSelection(): () => void {
  const selection = document.getSelection();
  if (!selection?.rangeCount) return () => {};

  const activeElement = document.activeElement;
  const ranges: Range[] = [];
  for (let index = 0; index < selection.rangeCount; index += 1) {
    ranges.push(selection.getRangeAt(index));
  }

  const focusElement =
    activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement
      ? activeElement
      : null;
  focusElement?.blur();
  selection.removeAllRanges();

  return () => {
    if ((selection as Selection & { type?: string }).type === 'Caret') selection.removeAllRanges();
    if (!selection.rangeCount) ranges.forEach((savedRange) => selection.addRange(savedRange));
    focusElement?.focus();
  };
}
