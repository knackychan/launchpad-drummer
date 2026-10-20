<script lang="ts">
  // The six SETUP sections as one lazily loaded unit; App owns the data and the wrappers.
  import SetupAudio from './SetupAudio.svelte';
  import SetupDisplay from './SetupDisplay.svelte';
  import SetupLog from './SetupLog.svelte';
  import SetupMidi from './SetupMidi.svelte';
  import SetupLearn from './SetupLearn.svelte';
  import SetupKeys from './SetupKeys.svelte';
  import type { Bend, CrtLevel } from '../lib/display.ts';
  import type { AudioStatus, KeysStatus, MidiDevice, SetModel } from '../lib/model.ts';
  let { audioStatus, dropoutRecent, onaudiostatus, onerror, midiDevices, midiLog, onrole, keysStatus, soundfont, onloadsoundfont, onclearsoundfont,
    set, onunbind, crt, bend, oncrt, onbend, onopenlog }: {
    audioStatus: AudioStatus | null; dropoutRecent: boolean; onaudiostatus: (status: AudioStatus) => void; onerror: (message: string) => void;
    midiDevices: MidiDevice[]; midiLog: { time: string; text: string }[]; onrole: (port: string, role: 'keys' | 'other') => void;
    keysStatus: KeysStatus; soundfont: string | null; onloadsoundfont: () => void; onclearsoundfont: () => void;
    set: SetModel; onunbind: (index: number) => void; crt: CrtLevel; bend: Bend; oncrt: (level: CrtLevel) => void; onbend: (bend: Bend) => void; onopenlog: () => void;
  } = $props();
</script>
<SetupAudio status={audioStatus} {dropoutRecent} onstatus={onaudiostatus} {onerror} />
<SetupMidi devices={midiDevices} events={midiLog} {onrole} />
<SetupKeys status={keysStatus} {soundfont} onload={onloadsoundfont} onclear={onclearsoundfont} />
<SetupLearn {set} {onunbind} />
<SetupDisplay level={crt} {bend} onchange={oncrt} {onbend} />
<SetupLog onopen={onopenlog} />
