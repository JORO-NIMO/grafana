//! Media Track Models & Presentation Subsystem

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TrackType {
    Audio = 1,
    Video = 2,
    Screen = 3,
    Presentation = 4,
}

#[derive(Debug, Clone)]
pub struct TrackNode {
    pub track_id: u32,
    pub track_type: TrackType,
    pub is_active: bool,
    pub owner_peer_id: String,
}

#[derive(Debug, Clone)]
pub struct VideoFrame<'a> {
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub timestamp_ms: u32,
    pub fourcc: u32,
    pub is_keyframe: bool,
}

#[derive(Debug, Clone)]
pub struct AudioFrame<'a> {
    pub pcm_samples: &'a [i16],
    pub sample_rate: u32,
    pub channels: u8,
    pub timestamp_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PresEventType {
    SlideChange = 1,
    PointerMove = 2,
    Annotate = 3,
    Stop = 4,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationEvent {
    pub event_type: PresEventType,
    pub current_slide: u32,
    pub total_slides: u32,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub color_hex: u32,
}

#[derive(Debug, Clone)]
pub struct PresentationState {
    pub is_presenting: bool,
    pub track_id: u32,
    pub presenter_id: String,
    pub current_slide: u32,
    pub total_slides: u32,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub color_hex: u32,
}

impl Default for PresentationState {
    fn default() -> Self {
        Self {
            is_presenting: false,
            track_id: 0,
            presenter_id: String::new(),
            current_slide: 0,
            total_slides: 0,
            pointer_x: 0.5,
            pointer_y: 0.5,
            color_hex: 0xFF0000,
        }
    }
}
