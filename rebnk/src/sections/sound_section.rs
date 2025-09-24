use std::collections::HashMap;

#[derive(Debug)]
struct SoundStructure {
    override_effects: bool,
    effects: Option<EffectsSection>,
    output_bus_id: u32, // 0 if inherited from parent
    parent_object_id: u32,

    override_playback_priority: bool,
    offset_priority_enabled: bool,

    additional_params: Vec<AdditionalParameter>,
    additional_param_values: Vec<AdditionalParamValue>,

    unknown_zero_byte: u8, // always 0x00

    positioning: Option<PositioningSection>,

    override_game_aux: bool,
    use_game_aux: bool,
    override_user_aux: bool,
    has_user_aux: bool,
    user_auxiliary_buses: Option<[u32; 4]>,

    unknown_playback_limit_flag: bool,
    playback_limit: Option<PlaybackLimit>,

    sound_instance_limit_scope: LimitScope,
    virtual_voice_behavior: VirtualVoiceBehavior,

    override_playback_limit: bool,
    override_virtual_voice: bool,

    state_groups: Vec<StateGroup>,
    rtpcs: Vec<RTPC>,
}

// EFFECTS

#[derive(Debug)]
struct EffectsSection {
    effect_count: u8,
    bypass_mask: u8, // bitmask
    effects: Vec<Effect>,
}

#[derive(Debug)]
struct Effect {
    index: u8, // 0 to 3
    effect_id: u32,
    zero_padding: [u8; 2], // two zero bytes
}

// ADDITIONAL PARAMETERS

#[derive(Debug)]
struct AdditionalParameter {
    param_type: AdditionalParamType,
}

#[derive(Debug)]
enum AdditionalParamType {
    Volume,
    Pitch,
    LowPassFilter,
    PlaybackPriority,
    OffsetPriority,
    Loop(u32), // number of loops or 0 = infinite
    MotionVolumeOffset,
    PannerX1,
    PannerX2,
    CenterPercent,
    AuxSendBusVolume(u8), // 0 to 3
    GameDefinedAuxVolume,
    OutputBusVolume,
    OutputBusLowPass,
    Unknown(u8),
}

#[derive(Debug)]
enum AdditionalParamValue {
    Float(f32),
    Uint32(u32),
}

// POSITIONING

#[derive(Debug)]
struct PositioningSection {
    mode: PositioningMode,
}

#[derive(Debug)]
enum PositioningMode {
    TwoD { enable_panner: bool },
    ThreeD {
        source: PositionSource,
        attenuation_id: u32,
        spatialization: bool,
        behavior: Option<ThreeDUserBehavior>,
    },
}

#[derive(Debug)]
enum PositionSource {
    UserDefined,
    GameDefined,
}

#[derive(Debug)]
struct ThreeDUserBehavior {
    play_type: PlayType,
    loop_enabled: Option<bool>,
    transition_time_ms: Option<u32>,
    follow_listener: bool,
}

#[derive(Debug)]
enum PlayType {
    SequenceStep,
    RandomStep,
    SequenceContinuous,
    RandomContinuous,
    SequenceStepPickNew,
    RandomStepPickNew,
}

// PLAYBACK LIMIT

#[derive(Debug)]
struct PlaybackLimit {
    equal_priority_behavior: EqualPriorityBehavior,
    limit_behavior: LimitBehavior,
    max_instances: u16,
}

#[derive(Debug)]
enum EqualPriorityBehavior {
    DiscardOldest,
    DiscardNewest,
}

#[derive(Debug)]
enum LimitBehavior {
    KillVoice,
    UseVirtualVoice,
}

#[derive(Debug)]
enum LimitScope {
    PerGameObject,
    Global,
}

#[derive(Debug)]
enum VirtualVoiceBehavior {
    Continue,
    Kill,
    SendToVirtual,
}

// STATE GROUP

#[derive(Debug)]
struct StateGroup {
    id: u32,
    change_timing: StateChangeTiming,
    custom_states: Vec<StateOverride>,
}

#[derive(Debug)]
enum StateChangeTiming {
    Immediate,
    NextGrid,
    NextBar,
    NextBeat,
    NextCue,
    CustomCue,
    EntryCue,
    ExitCue,
}

#[derive(Debug)]
struct StateOverride {
    state_id: u32,
    settings_object_id: u32,
}

// RTPC

#[derive(Debug)]
struct RTPC {
    game_parameter_id: u32,
    y_axis_type: RTPCType,
    unknown_id: u32,
    unknown_byte_1: u8,
    num_points: u8,
    unknown_byte_2: u8,
    points: Vec<RTPCPoint>,
}

#[derive(Debug)]
enum RTPCType {
    VoiceVolume,
    VoiceLowPass,
    Priority,
    InstanceLimit,
    AuxSend0,
    AuxSend1,
    AuxSend2,
    AuxSend3,
    GameAuxVolume,
    OutputVolume,
    OutputLowPass,
    BypassEffect0,
    BypassEffect1,
    BypassEffect2,
    BypassEffect3,
    BypassAll,
    MotionVolumeOffset,
    MotionLowPass,
    Unknown(u32),
}

#[derive(Debug)]
struct RTPCPoint {
    x: f32,
    y: f32,
    curve: CurveShape,
}

#[derive(Debug)]
enum CurveShape {
    LogBase3,
    SineFadeIn,
    LogBase1_41,
    InvertedSCurve,
    Linear,
    SCurve,
    ExpBase1_41,
    SineFadeOut,
    ExpBase3,
    Constant,
    Unknown(u32),
}
