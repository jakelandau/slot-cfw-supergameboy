use std::path::Path;

use slot_gfx::{Sizing, SRC_H, SRC_W, WHOLE_TEXTURE};
use slot_store::{ini, Platform, Scale, Stretch};

pub const VIDEO_MODE_FILE: &str = "Config/video_mode.txt";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VideoMode {
    #[default]
    Actual,
    Stretch,
}

impl VideoMode {
    pub fn as_str(self) -> &'static str {
        match self {
            VideoMode::Actual => "actual",
            VideoMode::Stretch => "stretch",
        }
    }

    pub fn parse(s: &str) -> Option<VideoMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "actual" => Some(VideoMode::Actual),
            "stretch" => Some(VideoMode::Stretch),
            _ => None,
        }
    }
}

pub fn video_mode_for(root: &Path, stem: &str) -> VideoMode {
    ini::value(root, VIDEO_MODE_FILE, stem)
        .as_deref()
        .and_then(VideoMode::parse)
        .unwrap_or_default()
}

pub fn write_video_mode(root: &Path, stem: &str, mode: VideoMode) -> std::io::Result<()> {
    ini::write(root, VIDEO_MODE_FILE, stem, mode.as_str())
}

/// What the frontend shows of a core's frame, and where in the game texture it lives.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Picture {
    pub sizing: Sizing,
    /// The region of the game texture that fills the band, in texture coordinates.
    pub source: [f32; 4],
    /// The size the picture lays out at: the canvas the core draws on, or just the game screen
    /// when the mode crops to it.
    pub size: (u32, u32),
    /// The frame the core actually draws inside that picture.
    pub content: (u32, u32),
}

/// The canvas a frame is drawn on: the platform's own, grown when a core draws a bigger picture,
/// which only SameBoy's bordered Super Game Boy frames are.
pub fn canvas_of(frame: (u32, u32)) -> (u32, u32) {
    (frame.0.max(SRC_W), frame.1.max(SRC_H))
}

/// `frame` is the picture the core draws: the platform's own unless a Super Game Boy border makes
/// it bigger, in which case it fills its canvas rather than sitting inside a larger one.
pub fn picture_for(
    platform: Platform,
    mode: VideoMode,
    scale: Scale,
    stretch: Stretch,
    frame: (u32, u32),
) -> Picture {
    let content = frame;
    let canvas = canvas_of(content);
    let (cw, ch) = canvas;
    let crop = |w: u32, h: u32| {
        let x = (cw - w) as f32 / 2.0 / cw as f32;
        let y = (ch - h) as f32 / 2.0 / ch as f32;
        [x, y, w as f32 / cw as f32, h as f32 / ch as f32]
    };
    let whole = Picture {
        sizing: Sizing::Band,
        source: WHOLE_TEXTURE,
        size: canvas,
        content,
    };
    #[cfg(feature = "nds")]
    if platform == Platform::Nds {
        return Picture {
            sizing: Sizing::Fit,
            source: WHOLE_TEXTURE,
            size: canvas,
            content,
        };
    }
    if platform.hardware() == Platform::Gba {
        let sizing = match scale {
            Scale::Integer => Sizing::Integer,
            Scale::FullScreen => Sizing::Panel,
            Scale::SpFrame | Scale::Fit => Sizing::Band,
        };
        return Picture {
            sizing,
            source: WHOLE_TEXTURE,
            size: canvas,
            content,
        };
    }
    let screen = |sizing| Picture {
        sizing,
        source: crop(content.0, content.1),
        size: content,
        content,
    };
    match (mode, scale, stretch) {
        (VideoMode::Actual, Scale::Integer, _) => screen(Sizing::Integer),
        (VideoMode::Actual, Scale::Fit, _) => screen(Sizing::Fit),
        (VideoMode::Actual, _, _) => whole,
        (VideoMode::Stretch, _, Stretch::Classic) => {
            let x = (cw - content.0) as f32 / 2.0 / cw as f32;
            let w = content.0 as f32 / cw as f32;
            Picture {
                sizing: Sizing::Band,
                source: [x, 0.0, w, 1.0],
                ..whole
            }
        }
        (VideoMode::Stretch, _, Stretch::FullScreen) => Picture {
            sizing: Sizing::Panel,
            ..screen(Sizing::Panel)
        },
    }
}
