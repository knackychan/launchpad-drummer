/** Keep the last path segment whole when it fits. */
export function middleTruncate(s: string, max: number): string {
  max = Math.max(0, Math.floor(max));
  if (s.length <= max) return s;
  if (max < 2) return max ? '…' : '';
  const last = s.split(/[\\/]/).at(-1) ?? '';
  const suffix = Math.max(Math.floor((max - 1) / 2), last.length < max ? last.length : 0);
  return s.slice(0, max - suffix - 1) + '…' + s.slice(-suffix);
}
