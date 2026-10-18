<script lang="ts">
  // Sequencer plate (UI spec 4.7), not fitted yet: drawn as the design shows it, inert and dimmed.
  const LANES = 4, STEPS = 16;
</script>

<div class="seq plate unfitted" aria-label="Sequencer — not fitted yet" data-label="SEQUENCER · STEP GRID" title="Sequencer — Part 2, not fitted yet">
  <div class="sq-head" aria-hidden="true">
    <div class="sq-left">
      <span class="key round">▶</span>
      <span class="key round">●</span>
      <span class="silk">BPM</span><span class="lcd screen num">100</span>
      <span class="silk">SWING</span><span class="lcd screen num">50%</span>
    </div>
    <span class="silk">STEPS</span>
    <div class="switch">
      {#each [4, 8, 16, 32] as n}<span class="key" class:on={n === STEPS}>{n}</span>{/each}
    </div>
    <span class="key">+ LANE</span>
    <span class="key">CLEAR</span>
    <span class="spacer"></span>
    <span class="silk">NOT FITTED · PART 2</span>
  </div>
  <div class="sq-body" aria-hidden="true">
    {#each Array(LANES) as _}
      <div class="sq-lane">
        <div class="sq-lh"><span class="sw"></span><span class="lcd screen">—</span><span class="key small">M</span><span class="key small">×</span></div>
        <div class="sq-steps">
          {#each Array(STEPS) as _, i}<div class="st" class:beat={i % 4 === 0}></div>{/each}
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .seq { display: flex; flex-direction: column; gap: 6px; padding: 6px 10px; height: var(--sq, 120px); min-height: 0; flex: none; }
  .sq-head { flex: none; display: flex; align-items: center; gap: 8px; height: 22px; }
  .sq-head .key { height: 22px; display: inline-flex; align-items: center; }
  .sq-head .key.round { width: 22px; height: 22px; font-size: 10px; justify-content: center; }
  .sq-left { width: 223px; flex: none; display: flex; align-items: center; gap: 6px; }
  .num { height: 22px; line-height: 22px; padding: 0 5px; font-size: 11px; min-width: 38px; text-align: center; }
  .switch .key { height: 20px; padding: 0 8px; }
  .spacer { flex: 1; }
  .sq-body { flex: 1; min-height: 0; overflow: hidden; display: flex; flex-direction: column; gap: 3px; }
  .sq-lane { flex: none; display: flex; gap: 10px; height: var(--lh, 18px); }
  .sq-lh { width: 223px; flex: none; display: flex; align-items: center; gap: 5px; min-width: 0; }
  .sq-lh .sw { width: 4px; height: calc(var(--lh, 18px) - 4px); border-radius: 2px; flex: none; background: var(--line-hi); }
  .sq-lh .lcd { flex: 1; min-width: 0; height: var(--lh, 18px); line-height: var(--lh, 18px); padding: 0 6px; font-size: 10.5px; color: var(--lcd-dim); }
  .sq-lh .key.small { width: 20px; min-width: 0; height: min(18px, var(--lh, 18px)); padding: 0; font-size: 9px; display: grid; place-items: center; }
  .sq-steps { flex: 1; min-width: 0; display: grid; grid-template-columns: repeat(16, 1fr); gap: 2px; }
  .st { border-radius: 2px; background: var(--st); box-shadow: inset 0 0 0 1px var(--stl), 0 3px 0 #0b0908, 0 5px 6px rgba(0,0,0,.45); }
  .st.beat { background: var(--stb); }
</style>
