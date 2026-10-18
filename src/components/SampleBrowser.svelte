<script lang="ts">
  import { onDestroy } from 'svelte';
  import { invoke, isTauri } from '../lib/api.ts';
  import { middleTruncate } from '../lib/format.ts';
  import { flatten, keyNav, type TreeNode } from '../lib/tree.ts';
  let { libraryDir = $bindable(), selected, onclose, onload, ondragfile, onerror }: {
    libraryDir: string | null; selected: { bank: number; x: number; y: number } | null;
    onclose: () => void; onload: (path: string) => void; ondragfile: (path: string, e: PointerEvent) => void;
    onerror: (text: string) => void;
  } = $props();
  let dir = $state<string | null>(null), nodes = $state<TreeNode[]>([]), index = $state(0), playing = $state<string | null>(null);
  let timer = 0;
  const rows = $derived(flatten(nodes));
  let tree: HTMLDivElement;
  async function children(path: string, depth: number): Promise<TreeNode[]> {
    const entries = await invoke<{ name: string; path: string; isDir: boolean }[]>('list_dir', { path });
    return (entries ?? []).map(e => ({ ...e, depth }));
  }
  $effect(() => { dir = libraryDir ?? (isTauri ? null : 'PREVIEW'); });
  $effect(() => {
    const path = dir; let live = true;
    nodes = []; index = 0;
    if (path) void children(path, 0).then(n => { if (live) nodes = n; }).catch(e => { if (live) onerror(String(e)); });
    return () => { live = false; };
  });
  async function chooseFolder() {
    try {
      const path = await invoke<string | null>('pick_folder', { title: 'Choose sample library folder', defaultDir: dir });
      if (path) { await invoke('set_library_dir', { dir: path }); libraryDir = path; }
    } catch (e) { onerror(String(e)); }
  }
  async function toggle(node: TreeNode, open = !node.open) {
    try { if (open && !node.children) node.children = await children(node.path, node.depth + 1); node.open = open; }
    catch (e) { onerror(String(e)); }
  }
  async function audition(node: TreeNode) {
    try {
      await invoke('audition_file', { path: node.path }); playing = node.path;
      clearTimeout(timer); timer = window.setTimeout(() => { playing = null; }, 1500);
    } catch { onerror(`Can't play ${node.name}`); }
  }
  function keyboard(e: KeyboardEvent) {
    if (!['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', ' ', 'Enter'].includes(e.key)) return;
    e.preventDefault(); e.stopPropagation();
    const result = keyNav(rows, index, e.key); index = result.index;
    const node = rows[index]; if (!node) return;
    if (result.action === 'open' || result.action === 'close' || result.action === 'toggle') void toggle(node, result.action === 'open' ? true : result.action === 'close' ? false : !node.open);
    if (result.action === 'audition') void audition(node);
    if (result.action === 'load') onload(node.path);
    tree.querySelector(`#sample-row-${index}`)?.scrollIntoView({ block: 'nearest' });
  }
  onDestroy(() => { clearTimeout(timer); });
</script>

<div class="browser screen">
  <div class="br-head"><button class="key small" aria-label="Close browser" onclick={onclose}>×</button>
    <span class="lcd" title={dir ?? ''}>{dir ? middleTruncate(dir, 34) : 'NO LIBRARY FOLDER'}</span>
    <button class="key small" onclick={() => void chooseFolder()}>FOLDER…</button></div>
  {#if !dir}<p class="lcd-dim">Choose a library folder to browse samples.</p>{/if}
  <div bind:this={tree} class="tree" role="tree" aria-label="Sample library" aria-activedescendant={rows[index] ? `sample-row-${index}` : undefined} tabindex="0" onkeydown={keyboard}>
    {#each rows as node, i (node.path)}
      <div id="sample-row-{i}" class="ti" class:folder={node.isDir} class:file={!node.isDir} class:active={index === i} class:playing={playing === node.path}
        role="treeitem" tabindex="-1" aria-level={node.depth + 1} aria-expanded={node.isDir ? !!node.open : undefined} aria-selected={index === i}
        style:padding-left="{10 + node.depth * 14 + (node.isDir ? 0 : 14)}px"
        onpointerdown={(e) => { index = i; tree.focus(); if (!node.isDir && e.button === 0) ondragfile(node.path, e); }}
        onclick={() => { index = i; tree.focus(); if (node.isDir) void toggle(node); else void audition(node); }} onkeydown={keyboard}>
        {#if node.isDir}<span>{node.open ? '▾' : '▸'}</span>{node.name}
        {:else}<span>{node.name.replace(/\.[^.]+$/, '')}</span><span class="ext">{node.name.match(/\.[^.]+$/)?.[0]}</span>{/if}
      </div>
    {/each}
  </div>
</div>
<style>
  .browser { flex: 1; min-width: 0; display: flex; flex-direction: column; min-height: 0; overflow: hidden; }
  .br-head { display: flex; align-items: center; gap: 6px; padding: 8px; }
  .br-head .lcd { flex: 1; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tree { flex: 1; overflow: auto; padding: 4px 0 8px; font: 400 12px/1 var(--f-lcd); }
  .ti { display: flex; align-items: center; gap: 4px; height: 22px; padding: 0 10px; cursor: pointer; white-space: nowrap; }
  .file { color: #b9c6ca; touch-action: none; } .folder { color: var(--lcd); } .ext { color: var(--lcd-dim); }
  .ti:hover { background: rgba(255,255,255,.04); } .ti.active { background: rgba(94,200,255,.12); color: #fff; }
  .ti.playing::after { content: '♪'; margin-left: auto; color: var(--ok); }
  p { padding: 8px; font: 400 12px/1.5 var(--f-lcd); }
</style>
