const tauri = typeof window !== 'undefined' ? window.__TAURI__ : undefined;
export const isTauri = !!tauri;
const PREVIEW: Record<string, (args?: Record<string, unknown>) => unknown> = {
  get_startup: () => ({ kind: 'loaded', path: 'PREVIEW', name: 'PREVIEW KIT', fromBackup: false, backupSavedAt: null }),
  history_state: () => ({ canUndo: false, canRedo: false }),
  missing_samples: () => [],
  get_config: () => ({ lastSet: null, libraryDir: null, soundfont: null,
    audio: { backend: 'wasapi', device: null, bufferFrames: null }, midiRoles: {} }),
};
export function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return tauri ? tauri.core.invoke<T>(cmd, args) : Promise.resolve(PREVIEW[cmd]?.(args) as T);
}
export function listen<T>(event: string, fn: (payload: T) => void): Promise<() => void> {
  return tauri ? tauri.event.listen<T>(event, e => fn(e.payload)) : Promise.resolve(() => {});
}
