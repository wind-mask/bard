use std::io::Write;

use anyhow::{Context, Result};
use compact_str::{CompactString, ToCompactString, format_compact};

use crate::models::WaybarOutput;

#[derive(Clone, Debug, PartialEq)]
pub enum RenderedFrame {
    Hidden,
    NoSong,
    SongInfo {
        artist: CompactString,
        title: CompactString,
    },
    Lyrics {
        current: CompactString,
        next: CompactString,
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
    last_frame.replace(next_frame);
    Ok(true)
}

fn render_just<W: Write>(writer: &mut W, frame: &RenderedFrame) -> Result<()> {
    let output = match frame {
        RenderedFrame::Hidden => WaybarOutput {
            text: CompactString::new(""),
            alt: CompactString::new(""),
            tooltip: CompactString::new(""),
            class: "hidden".to_compact_string(),
        },
        RenderedFrame::NoSong => WaybarOutput {
            text: CompactString::new(""),
            alt: CompactString::new(""),
            tooltip: CompactString::new(""),
            class: "no-song".to_compact_string(),
        },
        RenderedFrame::SongInfo { artist, title } => {
            let text = format_compact!("{artist} - {title}");
            WaybarOutput {
                text: text.clone(),
                alt: CompactString::new(""),
                tooltip: text,
                class: "has-song".to_compact_string(),
            }
        }
        RenderedFrame::Lyrics { current, next } => WaybarOutput {
            text: if current.is_empty() {
                "...".to_compact_string()
            } else {
                current.clone()
            },
            alt: next.clone(),
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

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::{RenderedFrame, render_if_changed};

    #[test]
    fn suppresses_duplicate_frames_and_writes_json_lines() {
        let mut output = Vec::new();
        let mut last_frame = None;

        assert!(render_if_changed(&mut output, &mut last_frame, RenderedFrame::NoSong).unwrap());
        assert!(!render_if_changed(&mut output, &mut last_frame, RenderedFrame::NoSong).unwrap());
        assert!(render_if_changed(&mut output, &mut last_frame, RenderedFrame::Hidden).unwrap());

        let lines = String::from_utf8(output).unwrap();
        let lines = lines.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[0]).unwrap()["class"],
            "no-song"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[1]).unwrap()["class"],
            "hidden"
        );
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

        assert!(render_if_changed(&mut writer, &mut last_frame, RenderedFrame::NoSong).is_err());
        assert!(last_frame.is_none());
    }
}
