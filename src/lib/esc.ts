const handlers: (() => void)[] = [];
export function pushEsc(handler: () => void): () => void {
  handlers.push(handler);
  return () => { const index = handlers.lastIndexOf(handler); if (index >= 0) handlers.splice(index, 1); };
}
export function runEsc(): boolean {
  const handler = handlers.pop();
  if (!handler) return false;
  handler();
  return true;
}
