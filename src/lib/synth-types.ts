// Types of the keys synth (keys synth plan, Task 9). They mirror `lpd_core::synth` (serde camelCase).
import type { KeysSound } from './keys.ts';

export type TableId = 'basic' | 'pwm' | 'harmonics' | 'vowels' | 'fmSweep' | 'syncSweep' | 'organ' | 'digital';
export type Warp = 'off' | 'sync' | 'bend' | 'pwm' | 'fm' | 'rm';
export type SubShape = 'sine' | 'triangle' | 'saw' | 'square';
export type NoiseColor = 'white' | 'pink';
export type FilterType = 'lp24' | 'lp12' | 'hp12' | 'bp12' | 'notch';
export type LfoShape = 'sine' | 'triangle' | 'sawUp' | 'sawDown' | 'square' | 'sampleHold' | 'smoothRandom';
export type SyncDiv = '1/32' | '1/16T' | '1/16' | '1/16D' | '1/8T' | '1/8' | '1/8D' | '1/4T' | '1/4' | '1/4D' | '1/2' | '1/1' | '2/1' | '4/1' | '8/1';
export type LfoMode = 'trig' | 'free' | 'oneShot';
export type Polarity = 'bi' | 'uni';
export type VoiceMode = 'poly' | 'mono' | 'legato';
export type GlideMode = 'always' | 'legato';
export type DriveType = 'soft' | 'tube' | 'hard' | 'fold';
export type SourceId = 'env1' | 'env2' | 'env3' | 'lfo1' | 'lfo2' | 'vel' | 'note' | 'rand' | 'wheel' | 'bend';
export type Category = 'piano' | 'keys' | 'organ' | 'pad' | 'lead' | 'bass' | 'pluck' | 'fx';

export interface OscParams {
  on: boolean; table: TableId; pos: number; oct: number; semi: number; fine: number; warp: Warp; warpAmt: number;
  unison: number; detune: number; blend: number; width: number; phaseRand: number; level: number; pan: number; toFilter: boolean;
}
export interface SubParams { on: boolean; shape: SubShape; oct: number; level: number; toFilter: boolean }
export interface NoiseParams { on: boolean; color: NoiseColor; level: number; toFilter: boolean }
export interface FilterParams { on: boolean; type: FilterType; cutoff: number; reso: number; drive: number; keyTrack: number; envAmt: number }
export interface AmpParams { level: number; velSens: number }
export interface EnvParams { attack: number; decay: number; sustain: number; release: number }
export interface LfoParams { shape: LfoShape; rate: number; sync: boolean; syncDiv: SyncDiv; mode: LfoMode; delay: number; phase: number; polarity: Polarity }
export interface VoiceParams { mode: VoiceMode; glide: number; glideMode: GlideMode; bendRange: number; drift: number; volumeDb: number }
export interface DriveFx { on: boolean; type: DriveType; amount: number; tone: number; mix: number }
export interface ChorusFx { on: boolean; rate: number; depth: number; delay: number; feedback: number; mix: number }
export interface DelayFx { on: boolean; time: number; sync: boolean; syncDiv: SyncDiv; feedback: number; pingPong: boolean; tone: number; mix: number }
export interface ReverbFx { on: boolean; size: number; decay: number; damp: number; preDelay: number; mix: number }
export interface FxParams { drive: DriveFx; chorus: ChorusFx; delay: DelayFx; reverb: ReverbFx }
export interface Route { source: SourceId; dest: string | null; amount: number; aux: SourceId | null; bypass: boolean }
export interface SynthParams {
  oscA: OscParams; oscB: OscParams; sub: SubParams; noise: NoiseParams; filter: FilterParams; amp: AmpParams;
  env: EnvParams[]; lfo: LfoParams[]; voice: VoiceParams; routes: Route[]; fx: FxParams;
}
export interface Patch { patchVersion: number; name: string; category: Category; params: SynthParams }
export type PatchSource = { factory: string } | { user: string } | 'init';
export interface PatchRef { id: string; name: string; category: Category }
export interface KeysSettings { patch: Patch; patchSource: PatchSource; modified: boolean; gmSound: KeysSound; volumeDb: number; output: number | null }
export interface KeysMix { gmSound: KeysSound; volumeDb: number; output: number | null }

export type ParamKind = 'bool' | 'choice' | 'int' | 'float';
export type Taper = 'lin' | 'log' | 'cubic';
export type Unit = 'pct' | 'hz' | 'sec' | 'cents' | 'semis' | 'db' | 'pan' | 'deg' | 'envOct' | 'drive24' | 'drive36' | 'oct' | 'voices';
export interface ParamMeta {
  id: string; section: string; label: string; kind: ParamKind; default: number;
  min?: number; max?: number; taper?: Taper; unit?: Unit; choices?: string[]; labels?: string[]; dest?: string;
}
export interface DestMeta { id: string; label: string; param: string | null }
export interface SourceMeta { id: SourceId; label: string; color: string }
export interface CategoryMeta { id: Category; label: string; color: [number, number, number] }
export interface FactoryMeta { id: string; name: string; category: Category; params: SynthParams }
export interface SynthMeta {
  params: ParamMeta[]; dests: DestMeta[]; sources: SourceMeta[]; categories: CategoryMeta[];
  lcxl: string[][]; init: SynthParams; factory: FactoryMeta[];
}
/** The `synth-telemetry` event (plan 0.4). */
export interface SynthTelemetry {
  scope: number[]; spectrum: number[]; curve: number[]; sources: number[]; live: number[];
  env: [number, number][]; lfo: [number, number][]; cutoffHz: number; voices: number; cpu: number; resets: number;
}
