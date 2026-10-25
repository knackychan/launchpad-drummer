import { invoke } from './api.ts';
import { fit, present } from './phosphor.ts';
import { onSynthFeed } from './synthFeed.ts';

type Options = { draw: (g: CanvasRenderingContext2D, w: number, h: number) => void; key?: unknown };
const painters = new Map<HTMLCanvasElement, Options>();
const dirty = new Set<HTMLCanvasElement>();
const tables = new Map<string, Promise<number[][] | null>>();
let raf = 0, timer: ReturnType<typeof setTimeout> | undefined, expiry: ReturnType<typeof setTimeout> | undefined, last = -Infinity;
const motion = typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;
function schedule() {
  if (typeof document === 'undefined' || document.hidden || !dirty.size || raf || timer !== undefined) return;
  const wait = (motion?.matches ? 100 : 1000/60) - (performance.now()-last);
  if (wait > 0) { timer = setTimeout(() => { timer = undefined; schedule(); }, wait); return; }
  raf = requestAnimationFrame(() => {
    raf = 0;
    if(document.hidden) return;
    last = performance.now();
    for(const node of dirty){const opts=painters.get(node),fitted=fit(node);if(opts&&fitted){opts.draw(...fitted);present(node);}}
    dirty.clear();
  });
}
function redraw() { for(const node of painters.keys())dirty.add(node);schedule(); }
onSynthFeed(() => {
  if(!painters.size)return;
  redraw();clearTimeout(expiry);expiry=setTimeout(redraw,501);
});
if(typeof document !== 'undefined')document.addEventListener('visibilitychange',redraw);
motion?.addEventListener('change',redraw);

/** One shared, feed/resize-driven paint queue; no per-canvas animation loop. */
export function display(node: HTMLCanvasElement, opts: Options) {
  painters.set(node,opts);
  const observer=new ResizeObserver(()=>{dirty.add(node);schedule();});observer.observe(node);
  dirty.add(node);schedule();
  return {
    update(next:Options){painters.set(node,next);dirty.add(node);schedule();},
    destroy(){observer.disconnect();painters.delete(node);dirty.delete(node);if(!painters.size){cancelAnimationFrame(raf);raf=0;clearTimeout(timer);timer=undefined;clearTimeout(expiry);}},
  };
}
/** Fetch each table once; the browser preview has no table data. */
export function tableFrames(table:string):Promise<number[][]|null>{
  if(!tables.has(table))tables.set(table,invoke<{frames:number[][]}|undefined>('synth_table',{table}).then(result=>result?.frames??null).catch(()=>null));
  return tables.get(table)!;
}
