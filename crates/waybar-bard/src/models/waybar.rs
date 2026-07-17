use compact_str::CompactString;
use serde::Serialize;

#[derive(Serialize)]
pub struct WaybarOutput {
    pub text: CompactString,
    pub alt: CompactString,
    pub tooltip: CompactString,
    pub class: CompactString,
}
