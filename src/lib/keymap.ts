export function padForKey(key: string): { x: number; y: number } | null {
  if (key.length !== 1) return null;
  const rows = ['12345678', 'qwertyui', 'asdfghjk', 'zxcvbnm,'];
  for (let r = 0; r < rows.length; r++) {
    const x = rows[r].indexOf(key.toLowerCase());
    if (x !== -1) return { x, y: 3 - r };
  }
  return null;
}
