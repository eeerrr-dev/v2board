export async function copyText(text: string | number | null | undefined): Promise<boolean> {
  const value = String(text ?? '');
  if (!navigator.clipboard?.writeText) return false;
  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    return false;
  }
}
