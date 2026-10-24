//! Synth parameters (keys synth spec §3–5, §8.1): the values of a patch, their ids, ranges,
//! tapers and units. Everything here is plain data; the audio thread reads and writes it
//! without allocating.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const ROUTES: usize = 16;
pub const UNISON_MAX: usize = 7;
pub const ENVS: usize = 3;
pub const LFOS: usize = 2;
pub const DESTS: usize = 27;
pub const SOURCES: usize = 10;

/// A field a `ParamId` reads and writes as `f32` (bools 0 / 1, selectors by index).
pub trait ParamField: Copy {
    fn to_f32(self) -> f32;
    fn from_f32(v: f32) -> Self;
}

impl ParamField for f32 {
    fn to_f32(self) -> f32 {
        self
    }
    fn from_f32(v: f32) -> Self {
        v
    }
}

impl ParamField for bool {
    fn to_f32(self) -> f32 {
        if self {
            1.0
        } else {
            0.0
        }
    }
    fn from_f32(v: f32) -> Self {
        v >= 0.5
    }
}

impl ParamField for i8 {
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
    fn from_f32(v: f32) -> Self {
        v.round() as i8
    }
}

impl ParamField for u8 {
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
    fn from_f32(v: f32) -> Self {
        v.round().max(0.0) as u8
    }
}

/// A selector: its serde ids and the labels the UI prints, in index order.
pub trait Choice: Copy + 'static {
    const IDS: &'static [&'static str];
    const LABELS: &'static [&'static str];
}

macro_rules! choice {
    ($name:ident { $first:ident = $fid:literal / $flabel:literal $(, $v:ident = $id:literal / $label:literal)* $(,)? }) => {
        #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
        pub enum $name {
            #[default]
            #[serde(rename = $fid)]
            $first,
            $(#[serde(rename = $id)] $v,)*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$name::$first $(, $name::$v)*];
        }

        impl Choice for $name {
            const IDS: &'static [&'static str] = &[$fid $(, $id)*];
            const LABELS: &'static [&'static str] = &[$flabel $(, $label)*];
        }

        impl ParamField for $name {
            fn to_f32(self) -> f32 {
                self as usize as f32
            }
            fn from_f32(v: f32) -> Self {
                let i = if v.is_finite() { v.round().max(0.0) as usize } else { 0 };
                $name::ALL[i.min($name::ALL.len() - 1)]
            }
        }
    };
}

choice!(TableId { Basic = "basic" / "BASIC", Pwm = "pwm" / "PWM", Harmonics = "harmonics" / "HARMONICS",
    Vowels = "vowels" / "VOWELS", FmSweep = "fmSweep" / "FM SWEEP", SyncSweep = "syncSweep" / "SYNC SWEEP",
    Organ = "organ" / "ORGAN", Digital = "digital" / "DIGITAL" });
choice!(Warp { Off = "off" / "OFF", Sync = "sync" / "SYNC", Bend = "bend" / "BEND", Pwm = "pwm" / "PWM",
    Fm = "fm" / "FM", Rm = "rm" / "RM" });
choice!(SubShape { Sine = "sine" / "SINE", Triangle = "triangle" / "TRI", Saw = "saw" / "SAW", Square = "square" / "SQUARE" });
choice!(NoiseColor { White = "white" / "WHITE", Pink = "pink" / "PINK" });
choice!(FilterType { Lp24 = "lp24" / "LP 24", Lp12 = "lp12" / "LP 12", Hp12 = "hp12" / "HP 12",
    Bp12 = "bp12" / "BP 12", Notch = "notch" / "NOTCH" });
choice!(LfoShape { Sine = "sine" / "SINE", Triangle = "triangle" / "TRI", SawUp = "sawUp" / "SAW UP",
    SawDown = "sawDown" / "SAW DN", Square = "square" / "SQUARE", SampleHold = "sampleHold" / "S&H",
    SmoothRandom = "smoothRandom" / "SMOOTH" });
choice!(SyncDiv { D1_32 = "1/32" / "1/32", D1_16T = "1/16T" / "1/16T", D1_16 = "1/16" / "1/16",
    D1_16D = "1/16D" / "1/16D", D1_8T = "1/8T" / "1/8T", D1_8 = "1/8" / "1/8", D1_8D = "1/8D" / "1/8D",
    D1_4T = "1/4T" / "1/4T", D1_4 = "1/4" / "1/4", D1_4D = "1/4D" / "1/4D", D1_2 = "1/2" / "1/2",
    D1_1 = "1/1" / "1/1", D2_1 = "2/1" / "2/1", D4_1 = "4/1" / "4/1", D8_1 = "8/1" / "8/1" });
choice!(LfoMode { Trig = "trig" / "TRIG", Free = "free" / "FREE", OneShot = "oneShot" / "ONE-SHOT" });
choice!(Polarity { Bi = "bi" / "BI", Uni = "uni" / "UNI" });
choice!(VoiceMode { Poly = "poly" / "POLY", Mono = "mono" / "MONO", Legato = "legato" / "LEGATO" });
choice!(GlideMode { Always = "always" / "ALWAYS", Legato = "legato" / "LEGATO" });
choice!(DriveType { Soft = "soft" / "SOFT", Tube = "tube" / "TUBE", Hard = "hard" / "HARD", Fold = "fold" / "FOLD" });
choice!(Source { Env1 = "env1" / "ENV1", Env2 = "env2" / "ENV2", Env3 = "env3" / "ENV3", Lfo1 = "lfo1" / "LFO1",
    Lfo2 = "lfo2" / "LFO2", Vel = "vel" / "VEL", Note = "note" / "NOTE", Rand = "rand" / "RAND",
    Wheel = "wheel" / "WHEEL", Bend = "bend" / "BEND" });
choice!(Dest { OscAPos = "oscA.pos" / "OSC A · POS", OscAPitch = "oscA.pitch" / "OSC A · PITCH",
    OscAFine = "oscA.fine" / "OSC A · FINE", OscAWarp = "oscA.warpAmt" / "OSC A · WARP",
    OscADetune = "oscA.detune" / "OSC A · DETUNE", OscABlend = "oscA.blend" / "OSC A · BLEND",
    OscAWidth = "oscA.width" / "OSC A · WIDTH", OscALevel = "oscA.level" / "OSC A · LEVEL",
    OscAPan = "oscA.pan" / "OSC A · PAN",
    OscBPos = "oscB.pos" / "OSC B · POS", OscBPitch = "oscB.pitch" / "OSC B · PITCH",
    OscBFine = "oscB.fine" / "OSC B · FINE", OscBWarp = "oscB.warpAmt" / "OSC B · WARP",
    OscBDetune = "oscB.detune" / "OSC B · DETUNE", OscBBlend = "oscB.blend" / "OSC B · BLEND",
    OscBWidth = "oscB.width" / "OSC B · WIDTH", OscBLevel = "oscB.level" / "OSC B · LEVEL",
    OscBPan = "oscB.pan" / "OSC B · PAN",
    OscPitch = "osc.pitch" / "OSC · PITCH", SubLevel = "sub.level" / "SUB · LEVEL",
    NoiseLevel = "noise.level" / "NOISE · LEVEL", Cutoff = "filter.cutoff" / "FILTER · CUTOFF",
    Reso = "filter.reso" / "FILTER · RESO", Drive = "filter.drive" / "FILTER · DRIVE",
    AmpLevel = "amp.level" / "AMP · LEVEL", Lfo1Rate = "lfo1.rate" / "LFO1 · RATE",
    Lfo2Rate = "lfo2.rate" / "LFO2 · RATE" });

/// Badge colours of the sources, in `Source` order (spec 4.1).
pub const SOURCE_COLORS: [&str; SOURCES] = [
    "#ff5b1f", "#ffb21a", "#f2e15b", "#41c8ff", "#7f8cff", "#5cff9d", "#3fe0c5", "#d77bff",
    "#ff7fb0", "#c9d1d9",
];

/// The delay's divisions: the LFO list up to 1/2 (spec 5).
pub const DELAY_DIV_IDS: &[&str] = &[
    "1/32", "1/16T", "1/16", "1/16D", "1/8T", "1/8", "1/8D", "1/4T", "1/4", "1/4D", "1/2",
];

impl Source {
    pub fn color(self) -> &'static str {
        SOURCE_COLORS[self as usize]
    }
}

impl SyncDiv {
    /// Length in beats (a quarter note is one beat).
    pub fn beats(self) -> f32 {
        match self {
            SyncDiv::D1_32 => 0.125,
            SyncDiv::D1_16T => 1.0 / 6.0,
            SyncDiv::D1_16 => 0.25,
            SyncDiv::D1_16D => 0.375,
            SyncDiv::D1_8T => 1.0 / 3.0,
            SyncDiv::D1_8 => 0.5,
            SyncDiv::D1_8D => 0.75,
            SyncDiv::D1_4T => 2.0 / 3.0,
            SyncDiv::D1_4 => 1.0,
            SyncDiv::D1_4D => 1.5,
            SyncDiv::D1_2 => 2.0,
            SyncDiv::D1_1 => 4.0,
            SyncDiv::D2_1 => 8.0,
            SyncDiv::D4_1 => 16.0,
            SyncDiv::D8_1 => 32.0,
        }
    }

    pub fn seconds(self, bpm: f32) -> f32 {
        self.beats() * 60.0 / bpm.max(1.0)
    }
}

impl Dest {
    /// The knob a destination moves; `None` for the three pitch destinations.
    pub fn param(self) -> Option<ParamId> {
        Some(match self {
            Dest::OscAPos => ParamId::OscAPos,
            Dest::OscAFine => ParamId::OscAFine,
            Dest::OscAWarp => ParamId::OscAWarpAmt,
            Dest::OscADetune => ParamId::OscADetune,
            Dest::OscABlend => ParamId::OscABlend,
            Dest::OscAWidth => ParamId::OscAWidth,
            Dest::OscALevel => ParamId::OscALevel,
            Dest::OscAPan => ParamId::OscAPan,
            Dest::OscBPos => ParamId::OscBPos,
            Dest::OscBFine => ParamId::OscBFine,
            Dest::OscBWarp => ParamId::OscBWarpAmt,
            Dest::OscBDetune => ParamId::OscBDetune,
            Dest::OscBBlend => ParamId::OscBBlend,
            Dest::OscBWidth => ParamId::OscBWidth,
            Dest::OscBLevel => ParamId::OscBLevel,
            Dest::OscBPan => ParamId::OscBPan,
            Dest::SubLevel => ParamId::SubLevel,
            Dest::NoiseLevel => ParamId::NoiseLevel,
            Dest::Cutoff => ParamId::FilterCutoff,
            Dest::Reso => ParamId::FilterReso,
            Dest::Drive => ParamId::FilterDrive,
            Dest::AmpLevel => ParamId::AmpLevel,
            Dest::Lfo1Rate => ParamId::Lfo1Rate,
            Dest::Lfo2Rate => ParamId::Lfo2Rate,
            Dest::OscAPitch | Dest::OscBPitch | Dest::OscPitch => return None,
        })
    }

    pub fn is_pitch(self) -> bool {
        matches!(self, Dest::OscAPitch | Dest::OscBPitch | Dest::OscPitch)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct OscParams {
    pub on: bool,
    pub table: TableId,
    pub pos: f32,
    pub oct: i8,
    pub semi: i8,
    pub fine: f32,
    pub warp: Warp,
    pub warp_amt: f32,
    pub unison: u8,
    pub detune: f32,
    pub blend: f32,
    pub width: f32,
    pub phase_rand: f32,
    pub level: f32,
    pub pan: f32,
    pub to_filter: bool,
}

impl Default for OscParams {
    fn default() -> Self {
        OscParams {
            on: true,
            table: TableId::Basic,
            pos: 0.667,
            oct: 0,
            semi: 0,
            fine: 0.0,
            warp: Warp::Off,
            warp_amt: 0.0,
            unison: 1,
            detune: 0.25,
            blend: 0.75,
            width: 0.5,
            phase_rand: 1.0,
            level: 0.75,
            pan: 0.0,
            to_filter: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SubParams {
    pub on: bool,
    pub shape: SubShape,
    pub oct: i8,
    pub level: f32,
    pub to_filter: bool,
}

impl Default for SubParams {
    fn default() -> Self {
        SubParams {
            on: false,
            shape: SubShape::Sine,
            oct: -1,
            level: 0.6,
            to_filter: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NoiseParams {
    pub on: bool,
    pub color: NoiseColor,
    pub level: f32,
    pub to_filter: bool,
}

impl Default for NoiseParams {
    fn default() -> Self {
        NoiseParams {
            on: false,
            color: NoiseColor::White,
            level: 0.3,
            to_filter: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct FilterParams {
    pub on: bool,
    #[serde(rename = "type")]
    pub kind: FilterType,
    pub cutoff: f32,
    pub reso: f32,
    pub drive: f32,
    pub key_track: f32,
    pub env_amt: f32,
}

impl Default for FilterParams {
    fn default() -> Self {
        FilterParams {
            on: true,
            kind: FilterType::Lp24,
            cutoff: 12_000.0,
            reso: 0.0,
            drive: 0.0,
            key_track: 0.0,
            env_amt: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AmpParams {
    pub level: f32,
    pub vel_sens: f32,
}

impl Default for AmpParams {
    fn default() -> Self {
        AmpParams {
            level: 0.8,
            vel_sens: 0.7,
        }
    }
}

/// Times in seconds. `Default` is ENV1's (sustain 1.0); `SynthParams::default` gives ENV2 and ENV3 sustain 0.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct EnvParams {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for EnvParams {
    fn default() -> Self {
        EnvParams {
            attack: 0.002,
            decay: 0.5,
            sustain: 1.0,
            release: 0.15,
        }
    }
}

/// `Default` is LFO1's; `SynthParams::default` gives LFO2 a 0.5 Hz FREE triangle.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LfoParams {
    pub shape: LfoShape,
    pub rate: f32,
    pub sync: bool,
    pub sync_div: SyncDiv,
    pub mode: LfoMode,
    pub delay: f32,
    pub phase: f32,
    pub polarity: Polarity,
}

impl Default for LfoParams {
    fn default() -> Self {
        LfoParams {
            shape: LfoShape::Sine,
            rate: 5.5,
            sync: false,
            sync_div: SyncDiv::D1_4,
            mode: LfoMode::Trig,
            delay: 0.0,
            phase: 0.0,
            polarity: Polarity::Bi,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceParams {
    pub mode: VoiceMode,
    pub glide: f32,
    pub glide_mode: GlideMode,
    pub bend_range: u8,
    pub drift: f32,
    pub volume_db: f32,
}

impl Default for VoiceParams {
    fn default() -> Self {
        VoiceParams {
            mode: VoiceMode::Poly,
            glide: 0.0,
            glide_mode: GlideMode::Always,
            bend_range: 2,
            drift: 0.0,
            volume_db: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DriveFx {
    pub on: bool,
    #[serde(rename = "type")]
    pub kind: DriveType,
    pub amount: f32,
    pub tone: f32,
    pub mix: f32,
}

impl Default for DriveFx {
    fn default() -> Self {
        DriveFx {
            on: false,
            kind: DriveType::Soft,
            amount: 0.3,
            tone: 20_000.0,
            mix: 1.0,
        }
    }
}

/// `delay` in seconds (5–30 ms).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChorusFx {
    pub on: bool,
    pub rate: f32,
    pub depth: f32,
    pub delay: f32,
    pub feedback: f32,
    pub mix: f32,
}

impl Default for ChorusFx {
    fn default() -> Self {
        ChorusFx {
            on: false,
            rate: 0.6,
            depth: 0.5,
            delay: 0.012,
            feedback: 0.0,
            mix: 0.5,
        }
    }
}

/// `time` in seconds (1 ms – 2 s).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DelayFx {
    pub on: bool,
    pub time: f32,
    pub sync: bool,
    pub sync_div: SyncDiv,
    pub feedback: f32,
    pub ping_pong: bool,
    pub tone: f32,
    pub mix: f32,
}

impl Default for DelayFx {
    fn default() -> Self {
        DelayFx {
            on: false,
            time: 0.375,
            sync: false,
            sync_div: SyncDiv::D1_8D,
            feedback: 0.35,
            ping_pong: false,
            tone: 6_000.0,
            mix: 0.25,
        }
    }
}

/// `decay` (RT60) and `pre_delay` in seconds.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ReverbFx {
    pub on: bool,
    pub size: f32,
    pub decay: f32,
    pub damp: f32,
    pub pre_delay: f32,
    pub mix: f32,
}

impl Default for ReverbFx {
    fn default() -> Self {
        ReverbFx {
            on: false,
            size: 0.6,
            decay: 2.5,
            damp: 0.5,
            pre_delay: 0.010,
            mix: 0.25,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FxParams {
    pub drive: DriveFx,
    pub chorus: ChorusFx,
    pub delay: DelayFx,
    pub reverb: ReverbFx,
}

/// One modulation route (spec 4.4). `dest: None` is a fresh row that does nothing yet.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub source: Source,
    #[serde(default)]
    pub dest: Option<Dest>,
    pub amount: f32,
    #[serde(default)]
    pub aux: Option<Source>,
    #[serde(default)]
    pub bypass: bool,
}

/// Up to 16 routes, always a dense prefix (serialized as a list of the used slots).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RouteTable {
    slots: [Option<Route>; ROUTES],
}

impl RouteTable {
    pub fn len(&self) -> usize {
        self.slots.iter().take_while(|s| s.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.slots[0].is_none()
    }

    pub fn get(&self, slot: usize) -> Option<&Route> {
        self.slots.get(slot).and_then(|s| s.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Route> {
        self.slots.iter().map_while(|s| s.as_ref())
    }

    /// `Some` at an existing slot replaces it, at `len` appends (refused when full or past the
    /// end); `None` removes the slot and moves the later routes up. Amounts are clamped to ±1.
    pub fn set(&mut self, slot: usize, route: Option<Route>) -> bool {
        let len = self.len();
        match route {
            Some(mut r) if slot <= len && slot < ROUTES => {
                r.amount = if r.amount.is_finite() {
                    r.amount.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                self.slots[slot] = Some(r);
                true
            }
            Some(_) => false,
            None if slot < len => {
                for i in slot..ROUTES - 1 {
                    self.slots[i] = self.slots[i + 1];
                }
                self.slots[ROUTES - 1] = None;
                true
            }
            None => false,
        }
    }

    pub fn push(&mut self, route: Route) -> bool {
        self.set(self.len(), Some(route))
    }
}

impl Serialize for RouteTable {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for RouteTable {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut table = RouteTable::default();
        for route in Vec::<Route>::deserialize(d)?.into_iter().take(ROUTES) {
            table.push(route);
        }
        Ok(table)
    }
}

/// Every value of a patch (spec 8.1 `params`). Copy, no strings: the audio thread copies it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SynthParams {
    pub osc_a: OscParams,
    pub osc_b: OscParams,
    pub sub: SubParams,
    pub noise: NoiseParams,
    pub filter: FilterParams,
    pub amp: AmpParams,
    pub env: [EnvParams; ENVS],
    pub lfo: [LfoParams; LFOS],
    pub voice: VoiceParams,
    pub routes: RouteTable,
    pub fx: FxParams,
}

impl Default for SynthParams {
    /// INIT: a plain saw into an LP 24 at 12 kHz, no FX, no routes.
    fn default() -> Self {
        let free_env = EnvParams {
            sustain: 0.0,
            ..EnvParams::default()
        };
        SynthParams {
            osc_a: OscParams::default(),
            osc_b: OscParams {
                on: false,
                ..OscParams::default()
            },
            sub: SubParams::default(),
            noise: NoiseParams::default(),
            filter: FilterParams::default(),
            amp: AmpParams::default(),
            env: [EnvParams::default(), free_env, free_env],
            lfo: [
                LfoParams::default(),
                LfoParams {
                    shape: LfoShape::Triangle,
                    rate: 0.5,
                    mode: LfoMode::Free,
                    ..LfoParams::default()
                },
            ],
            voice: VoiceParams::default(),
            routes: RouteTable::default(),
            fx: FxParams::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taper {
    Lin,
    Log,
    Cubic,
}

/// How the UI prints a value (Task 14 formats each one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Pct,
    Hz,
    Sec,
    Cents,
    Semis,
    Db,
    Pan,
    Deg,
    EnvOct,
    Drive24,
    Drive36,
    Oct,
    Voices,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Bool,
    Choice {
        ids: &'static [&'static str],
        labels: &'static [&'static str],
    },
    Int {
        min: i32,
        max: i32,
        unit: Unit,
    },
    Float {
        min: f32,
        max: f32,
        taper: Taper,
        unit: Unit,
    },
}

impl Kind {
    /// The nearest legal value (NaN becomes the minimum).
    pub fn clamp(self, v: f32) -> f32 {
        let v = if v.is_nan() { f32::NEG_INFINITY } else { v };
        match self {
            Kind::Bool => {
                if v >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Kind::Choice { ids, .. } => v.round().clamp(0.0, (ids.len() - 1) as f32),
            Kind::Int { min, max, .. } => v.round().clamp(min as f32, max as f32),
            Kind::Float { min, max, .. } => v.clamp(min, max),
        }
    }

    /// Value → knob position 0–1.
    pub fn to_norm(self, v: f32) -> f32 {
        let n = match self {
            Kind::Bool => {
                if v >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Kind::Choice { ids, .. } => {
                if ids.len() > 1 {
                    v / (ids.len() - 1) as f32
                } else {
                    0.0
                }
            }
            Kind::Int { min, max, .. } => (v - min as f32) / (max - min).max(1) as f32,
            Kind::Float {
                min, max, taper, ..
            } => match taper {
                Taper::Lin => (v - min) / (max - min),
                Taper::Log => (v.max(min) / min).ln() / (max / min).ln(),
                Taper::Cubic => (v.max(0.0) / max).cbrt(),
            },
        };
        if n.is_nan() {
            0.0
        } else {
            n.clamp(0.0, 1.0)
        }
    }

    /// Knob position 0–1 → value (quantized for bools, selectors and integers).
    pub fn from_norm(self, n: f32) -> f32 {
        let n = if n.is_nan() { 0.0 } else { n.clamp(0.0, 1.0) };
        match self {
            Kind::Bool => {
                if n >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Kind::Choice { ids, .. } => (n * (ids.len() - 1) as f32).round(),
            Kind::Int { min, max, .. } => (min as f32 + n * (max - min) as f32).round(),
            Kind::Float {
                min, max, taper, ..
            } => match taper {
                Taper::Lin => min + n * (max - min),
                Taper::Log => min * (max / min).powf(n),
                Taper::Cubic => max * n * n * n,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamMeta {
    pub id: &'static str,
    /// Section title on the synth plate (also the MIDI learn label prefix).
    pub section: &'static str,
    pub label: &'static str,
    pub kind: Kind,
}

const fn f(min: f32, max: f32, taper: Taper, unit: Unit) -> Kind {
    Kind::Float {
        min,
        max,
        taper,
        unit,
    }
}
const fn int(min: i32, max: i32, unit: Unit) -> Kind {
    Kind::Int { min, max, unit }
}
const fn pick<T: Choice>() -> Kind {
    Kind::Choice {
        ids: T::IDS,
        labels: T::LABELS,
    }
}
const B: Kind = Kind::Bool;
const PCT: Kind = f(0.0, 1.0, Taper::Lin, Unit::Pct);
use Taper::{Cubic, Lin, Log};

macro_rules! params {
    ($($v:ident = $id:literal, $section:literal, $label:literal, [$($path:tt)+], $kind:expr;)+) => {
        /// Every synth control (spec 3 and 5), with its string id (`oscA.pos`, `fx.reverb.mix`, …).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum ParamId { $($v),+ }

        impl ParamId {
            pub const ALL: &'static [ParamId] = &[$(ParamId::$v),+];

            pub fn meta(self) -> ParamMeta {
                match self {
                    $(ParamId::$v => ParamMeta { id: $id, section: $section, label: $label, kind: $kind },)+
                }
            }

            pub fn from_id(id: &str) -> Option<ParamId> {
                match id {
                    $($id => Some(ParamId::$v),)+
                    _ => None,
                }
            }

            pub fn get(self, p: &SynthParams) -> f32 {
                match self {
                    $(ParamId::$v => ParamField::to_f32(p.$($path)+),)+
                }
            }

            /// Writes the value clamped and quantized to the control's kind.
            pub fn set(self, p: &mut SynthParams, value: f32) {
                let v = self.meta().kind.clamp(value);
                match self {
                    $(ParamId::$v => p.$($path)+ = ParamField::from_f32(v),)+
                }
            }
        }
    };
}

params! {
    OscAOn = "oscA.on", "OSC A", "ON", [osc_a.on], B;
    OscATable = "oscA.table", "OSC A", "TABLE", [osc_a.table], pick::<TableId>();
    OscAPos = "oscA.pos", "OSC A", "POS", [osc_a.pos], PCT;
    OscAOct = "oscA.oct", "OSC A", "OCT", [osc_a.oct], int(-3, 3, Unit::Oct);
    OscASemi = "oscA.semi", "OSC A", "SEMI", [osc_a.semi], int(-12, 12, Unit::Semis);
    OscAFine = "oscA.fine", "OSC A", "FINE", [osc_a.fine], f(-100.0, 100.0, Lin, Unit::Cents);
    OscAWarp = "oscA.warp", "OSC A", "WARP", [osc_a.warp], pick::<Warp>();
    OscAWarpAmt = "oscA.warpAmt", "OSC A", "WARP AMT", [osc_a.warp_amt], PCT;
    OscAUnison = "oscA.unison", "OSC A", "UNISON", [osc_a.unison], int(1, 7, Unit::Voices);
    OscADetune = "oscA.detune", "OSC A", "DETUNE", [osc_a.detune], PCT;
    OscABlend = "oscA.blend", "OSC A", "BLEND", [osc_a.blend], PCT;
    OscAWidth = "oscA.width", "OSC A", "WIDTH", [osc_a.width], PCT;
    OscAPhaseRand = "oscA.phaseRand", "OSC A", "PHASE RAND", [osc_a.phase_rand], PCT;
    OscALevel = "oscA.level", "OSC A", "LEVEL", [osc_a.level], PCT;
    OscAPan = "oscA.pan", "OSC A", "PAN", [osc_a.pan], f(-1.0, 1.0, Lin, Unit::Pan);
    OscAToFilter = "oscA.toFilter", "OSC A", "→ FILTER", [osc_a.to_filter], B;
    OscBOn = "oscB.on", "OSC B", "ON", [osc_b.on], B;
    OscBTable = "oscB.table", "OSC B", "TABLE", [osc_b.table], pick::<TableId>();
    OscBPos = "oscB.pos", "OSC B", "POS", [osc_b.pos], PCT;
    OscBOct = "oscB.oct", "OSC B", "OCT", [osc_b.oct], int(-3, 3, Unit::Oct);
    OscBSemi = "oscB.semi", "OSC B", "SEMI", [osc_b.semi], int(-12, 12, Unit::Semis);
    OscBFine = "oscB.fine", "OSC B", "FINE", [osc_b.fine], f(-100.0, 100.0, Lin, Unit::Cents);
    OscBWarp = "oscB.warp", "OSC B", "WARP", [osc_b.warp], pick::<Warp>();
    OscBWarpAmt = "oscB.warpAmt", "OSC B", "WARP AMT", [osc_b.warp_amt], PCT;
    OscBUnison = "oscB.unison", "OSC B", "UNISON", [osc_b.unison], int(1, 7, Unit::Voices);
    OscBDetune = "oscB.detune", "OSC B", "DETUNE", [osc_b.detune], PCT;
    OscBBlend = "oscB.blend", "OSC B", "BLEND", [osc_b.blend], PCT;
    OscBWidth = "oscB.width", "OSC B", "WIDTH", [osc_b.width], PCT;
    OscBPhaseRand = "oscB.phaseRand", "OSC B", "PHASE RAND", [osc_b.phase_rand], PCT;
    OscBLevel = "oscB.level", "OSC B", "LEVEL", [osc_b.level], PCT;
    OscBPan = "oscB.pan", "OSC B", "PAN", [osc_b.pan], f(-1.0, 1.0, Lin, Unit::Pan);
    OscBToFilter = "oscB.toFilter", "OSC B", "→ FILTER", [osc_b.to_filter], B;
    SubOn = "sub.on", "SUB", "ON", [sub.on], B;
    SubShape = "sub.shape", "SUB", "SHAPE", [sub.shape], pick::<SubShape>();
    SubOct = "sub.oct", "SUB", "OCT", [sub.oct], int(-2, -1, Unit::Oct);
    SubLevel = "sub.level", "SUB", "LEVEL", [sub.level], PCT;
    SubToFilter = "sub.toFilter", "SUB", "→ FILTER", [sub.to_filter], B;
    NoiseOn = "noise.on", "NOISE", "ON", [noise.on], B;
    NoiseColor = "noise.color", "NOISE", "COLOR", [noise.color], pick::<NoiseColor>();
    NoiseLevel = "noise.level", "NOISE", "LEVEL", [noise.level], PCT;
    NoiseToFilter = "noise.toFilter", "NOISE", "→ FILTER", [noise.to_filter], B;
    FilterOn = "filter.on", "FILTER", "ON", [filter.on], B;
    FilterType = "filter.type", "FILTER", "TYPE", [filter.kind], pick::<FilterType>();
    FilterCutoff = "filter.cutoff", "FILTER", "CUTOFF", [filter.cutoff], f(20.0, 20_000.0, Log, Unit::Hz);
    FilterReso = "filter.reso", "FILTER", "RESO", [filter.reso], PCT;
    FilterDrive = "filter.drive", "FILTER", "DRIVE", [filter.drive], f(0.0, 1.0, Lin, Unit::Drive24);
    FilterKeyTrack = "filter.keyTrack", "FILTER", "KEY", [filter.key_track], PCT;
    FilterEnvAmt = "filter.envAmt", "FILTER", "ENV AMT", [filter.env_amt], f(-1.0, 1.0, Lin, Unit::EnvOct);
    AmpLevel = "amp.level", "AMP", "LEVEL", [amp.level], PCT;
    AmpVelSens = "amp.velSens", "AMP", "VEL", [amp.vel_sens], PCT;
    Env1Attack = "env1.attack", "ENV1", "A", [env[0].attack], f(0.0, 10.0, Cubic, Unit::Sec);
    Env1Decay = "env1.decay", "ENV1", "D", [env[0].decay], f(0.0, 10.0, Cubic, Unit::Sec);
    Env1Sustain = "env1.sustain", "ENV1", "S", [env[0].sustain], PCT;
    Env1Release = "env1.release", "ENV1", "R", [env[0].release], f(0.0, 20.0, Cubic, Unit::Sec);
    Env2Attack = "env2.attack", "ENV2", "A", [env[1].attack], f(0.0, 10.0, Cubic, Unit::Sec);
    Env2Decay = "env2.decay", "ENV2", "D", [env[1].decay], f(0.0, 10.0, Cubic, Unit::Sec);
    Env2Sustain = "env2.sustain", "ENV2", "S", [env[1].sustain], PCT;
    Env2Release = "env2.release", "ENV2", "R", [env[1].release], f(0.0, 20.0, Cubic, Unit::Sec);
    Env3Attack = "env3.attack", "ENV3", "A", [env[2].attack], f(0.0, 10.0, Cubic, Unit::Sec);
    Env3Decay = "env3.decay", "ENV3", "D", [env[2].decay], f(0.0, 10.0, Cubic, Unit::Sec);
    Env3Sustain = "env3.sustain", "ENV3", "S", [env[2].sustain], PCT;
    Env3Release = "env3.release", "ENV3", "R", [env[2].release], f(0.0, 20.0, Cubic, Unit::Sec);
    Lfo1Shape = "lfo1.shape", "LFO1", "SHAPE", [lfo[0].shape], pick::<LfoShape>();
    Lfo1Rate = "lfo1.rate", "LFO1", "RATE", [lfo[0].rate], f(0.01, 50.0, Log, Unit::Hz);
    Lfo1Sync = "lfo1.sync", "LFO1", "SYNC", [lfo[0].sync], B;
    Lfo1SyncDiv = "lfo1.syncDiv", "LFO1", "DIV", [lfo[0].sync_div], pick::<SyncDiv>();
    Lfo1Mode = "lfo1.mode", "LFO1", "MODE", [lfo[0].mode], pick::<LfoMode>();
    Lfo1Delay = "lfo1.delay", "LFO1", "DELAY", [lfo[0].delay], f(0.0, 5.0, Cubic, Unit::Sec);
    Lfo1Phase = "lfo1.phase", "LFO1", "PHASE", [lfo[0].phase], f(0.0, 1.0, Lin, Unit::Deg);
    Lfo1Polarity = "lfo1.polarity", "LFO1", "POL", [lfo[0].polarity], pick::<Polarity>();
    Lfo2Shape = "lfo2.shape", "LFO2", "SHAPE", [lfo[1].shape], pick::<LfoShape>();
    Lfo2Rate = "lfo2.rate", "LFO2", "RATE", [lfo[1].rate], f(0.01, 50.0, Log, Unit::Hz);
    Lfo2Sync = "lfo2.sync", "LFO2", "SYNC", [lfo[1].sync], B;
    Lfo2SyncDiv = "lfo2.syncDiv", "LFO2", "DIV", [lfo[1].sync_div], pick::<SyncDiv>();
    Lfo2Mode = "lfo2.mode", "LFO2", "MODE", [lfo[1].mode], pick::<LfoMode>();
    Lfo2Delay = "lfo2.delay", "LFO2", "DELAY", [lfo[1].delay], f(0.0, 5.0, Cubic, Unit::Sec);
    Lfo2Phase = "lfo2.phase", "LFO2", "PHASE", [lfo[1].phase], f(0.0, 1.0, Lin, Unit::Deg);
    Lfo2Polarity = "lfo2.polarity", "LFO2", "POL", [lfo[1].polarity], pick::<Polarity>();
    VoiceMode = "voice.mode", "VOICE", "MODE", [voice.mode], pick::<VoiceMode>();
    VoiceGlide = "voice.glide", "VOICE", "GLIDE", [voice.glide], f(0.0, 2.0, Cubic, Unit::Sec);
    VoiceGlideMode = "voice.glideMode", "VOICE", "GLIDE MODE", [voice.glide_mode], pick::<GlideMode>();
    VoiceBendRange = "voice.bendRange", "VOICE", "BEND", [voice.bend_range], int(0, 24, Unit::Semis);
    VoiceDrift = "voice.drift", "VOICE", "DRIFT", [voice.drift], PCT;
    VoiceVolume = "voice.volumeDb", "VOICE", "VOLUME", [voice.volume_db], f(-60.0, 6.0, Lin, Unit::Db);
    FxDriveOn = "fx.drive.on", "DRIVE", "ON", [fx.drive.on], B;
    FxDriveType = "fx.drive.type", "DRIVE", "TYPE", [fx.drive.kind], pick::<DriveType>();
    FxDriveAmount = "fx.drive.amount", "DRIVE", "DRIVE", [fx.drive.amount], f(0.0, 1.0, Lin, Unit::Drive36);
    FxDriveTone = "fx.drive.tone", "DRIVE", "TONE", [fx.drive.tone], f(500.0, 20_000.0, Log, Unit::Hz);
    FxDriveMix = "fx.drive.mix", "DRIVE", "MIX", [fx.drive.mix], PCT;
    FxChorusOn = "fx.chorus.on", "CHORUS", "ON", [fx.chorus.on], B;
    FxChorusRate = "fx.chorus.rate", "CHORUS", "RATE", [fx.chorus.rate], f(0.05, 5.0, Log, Unit::Hz);
    FxChorusDepth = "fx.chorus.depth", "CHORUS", "DEPTH", [fx.chorus.depth], PCT;
    FxChorusDelay = "fx.chorus.delay", "CHORUS", "DELAY", [fx.chorus.delay], f(0.005, 0.03, Lin, Unit::Sec);
    FxChorusFeedback = "fx.chorus.feedback", "CHORUS", "FDBK", [fx.chorus.feedback], f(0.0, 0.9, Lin, Unit::Pct);
    FxChorusMix = "fx.chorus.mix", "CHORUS", "MIX", [fx.chorus.mix], PCT;
    FxDelayOn = "fx.delay.on", "DELAY", "ON", [fx.delay.on], B;
    FxDelayTime = "fx.delay.time", "DELAY", "TIME", [fx.delay.time], f(0.001, 2.0, Log, Unit::Sec);
    FxDelaySync = "fx.delay.sync", "DELAY", "SYNC", [fx.delay.sync], B;
    FxDelaySyncDiv = "fx.delay.syncDiv", "DELAY", "DIV", [fx.delay.sync_div], Kind::Choice { ids: DELAY_DIV_IDS, labels: DELAY_DIV_IDS };
    FxDelayFeedback = "fx.delay.feedback", "DELAY", "FDBK", [fx.delay.feedback], f(0.0, 0.95, Lin, Unit::Pct);
    FxDelayPingPong = "fx.delay.pingPong", "DELAY", "PING-PONG", [fx.delay.ping_pong], B;
    FxDelayTone = "fx.delay.tone", "DELAY", "TONE", [fx.delay.tone], f(500.0, 20_000.0, Log, Unit::Hz);
    FxDelayMix = "fx.delay.mix", "DELAY", "MIX", [fx.delay.mix], PCT;
    FxReverbOn = "fx.reverb.on", "REVERB", "ON", [fx.reverb.on], B;
    FxReverbSize = "fx.reverb.size", "REVERB", "SIZE", [fx.reverb.size], PCT;
    FxReverbDecay = "fx.reverb.decay", "REVERB", "DECAY", [fx.reverb.decay], f(0.2, 20.0, Log, Unit::Sec);
    FxReverbDamp = "fx.reverb.damp", "REVERB", "DAMP", [fx.reverb.damp], PCT;
    FxReverbPreDelay = "fx.reverb.preDelay", "REVERB", "PRE", [fx.reverb.pre_delay], f(0.0, 0.2, Lin, Unit::Sec);
    FxReverbMix = "fx.reverb.mix", "REVERB", "MIX", [fx.reverb.mix], PCT;
}
