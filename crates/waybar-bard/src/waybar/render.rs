use std::io::Write;

use anyhow::{Context, Result};
use compact_str::{CompactString, ToCompactString, format_compact};

use crate::models::WaybarOutput;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum RenderedFrame {
    Hidden,
    #[default]
    NoPlayer,
    Paused,
    NoLyrics {
        artist: CompactString,
        title: CompactString,
    },
    Lyrics {
        current: CompactString,
        alt: CompactString,
    },
}
pub fn render_if_changed<W: Write>(
    writer: &mut W,
    last_frame: &mut Option<RenderedFrame>,
    next_frame: RenderedFrame,
) -> Result<bool> {
    if last_frame.as_ref() == Some(&next_frame) {
        return Ok(false);
    }

    render_just(writer, &next_frame)?;
    last_frame.replace(next_frame.clone());

    Ok(true)
}

fn render_just<W: Write>(writer: &mut W, frame: &RenderedFrame) -> Result<()> {
    let output = match frame {
        RenderedFrame::Hidden => empty_output("hidden"),
        RenderedFrame::NoPlayer => empty_output("no-player"),
        RenderedFrame::Paused => empty_output("paused"),
        RenderedFrame::NoLyrics { artist, title } => {
            let text = format_compact!("{artist} - {title}");
            WaybarOutput {
                text: text.clone(),
                alt: CompactString::new(""),
                tooltip: text,
                class: "no-lyrics".to_compact_string(),
            }
        }
        RenderedFrame::Lyrics { current, alt } => WaybarOutput {
            text: if current.is_empty() {
                "...".to_compact_string()
            } else {
                current.clone()
            },
            alt: alt.clone(),
            tooltip: CompactString::new(""),
            class: "has-lyrics".to_compact_string(),
        },
    };

    serde_json::to_writer(&mut *writer, &output).context("Could not serialize Waybar output")?;
    writer
        .write_all(b"\n")
        .context("Could not terminate Waybar output line")?;
    writer.flush().context("Could not flush Waybar output")?;
    Ok(())
}

fn empty_output(class: &str) -> WaybarOutput {
    WaybarOutput {
        text: CompactString::new(""),
        alt: CompactString::new(""),
        tooltip: CompactString::new(""),
        class: class.to_compact_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use compact_str::ToCompactString;

    use super::{RenderedFrame, render_if_changed};

    fn rendered_value(frame: RenderedFrame) -> serde_json::Value {
        let mut output = Vec::new();
        let mut last = None;
        render_if_changed(&mut output, &mut last, frame).unwrap();
        serde_json::from_slice(&output).unwrap()
    }

    #[test]
    fn renders_stable_state_classes() {
        for (frame, expected) in [
            (RenderedFrame::Hidden, "hidden"),
            (RenderedFrame::NoPlayer, "no-player"),
            (RenderedFrame::Paused, "paused"),
        ] {
            let output = rendered_value(frame);
            assert_eq!(output["class"], expected);
            assert_eq!(output["text"], "");
        }

        let no_lyrics = rendered_value(RenderedFrame::NoLyrics {
            artist: "Artist".to_compact_string(),
            title: "Title".to_compact_string(),
        });
        assert_eq!(no_lyrics["class"], "no-lyrics");
        assert_eq!(no_lyrics["text"], "Artist - Title");

        let lyrics = rendered_value(RenderedFrame::Lyrics {
            current: "<line & text>".to_compact_string(),
            alt: "translation".to_compact_string(),
        });
        assert_eq!(lyrics["class"], "has-lyrics");
        assert_eq!(lyrics["text"], "<line & text>");
        assert_eq!(lyrics["alt"], "translation");
    }

    #[test]
    fn suppresses_duplicate_frames_and_writes_json_lines() {
        let mut output = Vec::new();
        let mut last_frame = None;

        assert!(render_if_changed(&mut output, &mut last_frame, RenderedFrame::NoPlayer).unwrap());
        assert!(!render_if_changed(&mut output, &mut last_frame, RenderedFrame::NoPlayer).unwrap());
        assert!(render_if_changed(&mut output, &mut last_frame, RenderedFrame::Hidden).unwrap());

        assert_eq!(String::from_utf8(output).unwrap().lines().count(), 2);
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn failed_write_does_not_update_last_frame() {
        let mut writer = FailingWriter;
        let mut last_frame = None;

        assert!(render_if_changed(&mut writer, &mut last_frame, RenderedFrame::NoPlayer).is_err());
        assert!(last_frame.is_none());
    }
}
