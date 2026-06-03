export function legacyCopyText(text: string | number | null | undefined) {
  if (text == null || text === '') return;

  const value = String(text);

  // Old admin bundle used copy-to-clipboard, which keeps an execCommand fallback.
  if (typeof document !== 'undefined' && document.body) {
    const textarea = document.createElement('textarea');
    textarea.value = value;
    textarea.setAttribute('readonly', '');
    textarea.style.position = 'fixed';
    textarea.style.left = '-9999px';
    textarea.style.top = '0';
    textarea.style.opacity = '0';

    document.body.appendChild(textarea);
    textarea.select();
    textarea.setSelectionRange(0, textarea.value.length);

    try {
      if (document.execCommand('copy')) return;
    } catch {
      // Fall through to the modern API when execCommand is unavailable.
    } finally {
      document.body.removeChild(textarea);
    }
  }

  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    void navigator.clipboard.writeText(value);
  }
}
