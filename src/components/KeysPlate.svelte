<script lang="ts">
  // Keys plate (UI spec 4.6), not fitted yet: drawn as the design shows it, inert and dimmed.
  /** short: keybed under 68 px, the third control row is dropped. */
  let { short = false }: { short?: boolean } = $props();
  const LO = 21, HI = 108, BASE = 36; // A0–C8; Keystation 49 window C2–C6
  const isBlack = (n: number) => [1, 3, 6, 8, 10].includes(n % 12);
  const keys: { n: number; black: boolean; left: number }[] = [];
  let whites = 0;
  for (let n = LO; n <= HI; n++) {
    if (isBlack(n)) keys.push({ n, black: true, left: (whites / 52) * 100 });
    else { keys.push({ n, black: false, left: 0 }); whites++; }
  }
  const whiteIdx = (n: number) => keys.filter((k) => !k.black && k.n < n).length;
  const winLeft = (whiteIdx(BASE) / 52) * 100;
  const winWidth = ((whiteIdx(BASE + 48) + 1 - whiteIdx(BASE)) / 52) * 100;
</script>

<div class="keys plate unfitted" class:short aria-label="Keyboard — not fitted yet" data-label="KEYS · KEYSTATION 49" title="Keys — Part 2, not fitted yet">
  <div class="k-wheels" aria-hidden="true">
    <div class="wheel"><div class="thumb" style:top="calc(.5 * (100% - 12px))"></div></div>
    <div class="wheel"><div class="thumb" style:top="calc(100% - 12px)"></div></div>
  </div>
  <div class="k-ctl" aria-hidden="true">
    <div class="k-row"><span class="lamp"></span><span class="lcd screen">E.PIANO</span><span class="lcd screen num">−6</span></div>
    <div class="k-row"><span class="lcd screen">C2–C6</span><span class="lcd screen out">→ MASTER</span></div>
    <div class="k-row k-last"><span class="silk">NOT FITTED</span><span class="lcd lcd-dim">PART 2</span></div>
  </div>
  <div class="kbed" aria-hidden="true">
    <div class="felt"><span class="win" style:left="{winLeft}%" style:width="{winWidth}%"></span></div>
    <div class="kwhites">
      {#each keys.filter((k) => !k.black) as k}
        <div class="kw" class:out={k.n < BASE || k.n > BASE + 48}>{#if k.n % 12 === 0}<span class="cn">C{k.n / 12 - 1}</span>{/if}</div>
      {/each}
    </div>
    {#each keys.filter((k) => k.black) as k}
      <div class="kb" class:out={k.n < BASE || k.n > BASE + 48} style:left="{k.left}%"></div>
    {/each}
  </div>
</div>

<style>
  .keys { display: flex; gap: 10px; padding: 6px 10px; flex: none; }
  .k-wheels { display: flex; gap: 5px; flex: none; }
  .wheel { width: 16px; height: var(--kh); position: relative; }
  .thumb { position: absolute; left: 1px; right: 1px; height: 12px; border-radius: 3px; background: var(--glass); box-shadow: inset 0 0 0 1px var(--vec); }
  .k-ctl { flex: none; width: 176px; display: flex; flex-direction: column; justify-content: center; gap: 4px; min-width: 0; }
  .k-row { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .k-row .lcd { flex: 1; min-width: 0; height: 20px; line-height: 20px; padding: 0 6px; font-size: 11px; white-space: nowrap; overflow: hidden; }
  .k-row .num { flex: none; min-width: 34px; text-align: center; }
  .k-row .out { flex: none; width: 76px; font-size: 10px; }
  .k-row .silk { font-size: 9.5px; }
  .short .k-last { display: none; }
  .kbed { flex: 1; min-width: 0; height: var(--kh); position: relative; padding-top: 5px; border-radius: 4px; background: var(--glass); box-shadow: inset 0 0 0 1px var(--line); }
  .felt { position: absolute; left: 0; right: 0; top: 0; height: 5px; background: #2a1208; border-radius: 4px 4px 0 0; }
  .win { position: absolute; top: 1px; height: 3px; border-radius: 2px; background: var(--orange); box-shadow: 0 0 6px rgba(255,91,31,.7); }
  .kwhites { display: flex; gap: 1px; height: 100%; }
  .kw { flex: 1; min-width: 0; position: relative; border-radius: 0 0 2px 2px; background: #e9e1cc; box-shadow: inset 0 -2px 0 #bdb49c; }
  .kb { position: absolute; top: 5px; z-index: 1; width: calc(100% / 52 * .6); margin-left: calc(100% / 52 * -.3); height: calc((var(--kh) - 5px) * .62); border-radius: 0 0 2px 2px; background: #101826; box-shadow: inset 0 0 0 1px #33496a; }
  .kw.out { filter: brightness(.6); }
  .kb.out { filter: brightness(.7); }
  .cn { position: absolute; left: 0; right: 0; bottom: 6px; text-align: center; font: 600 9px/1 var(--f-silk); color: #8a826d; }
</style>
