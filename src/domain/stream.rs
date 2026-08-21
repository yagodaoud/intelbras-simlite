use crate::domain::Layout;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveProfile {
    #[default]
    Quality,
    Performance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamKind {
    /// Stream extra (sub) — mosaico leve.
    Extra,
    /// Stream principal — qualidade / 1 câmera / playback.
    Main,
}

impl StreamKind {
    pub fn subtype(self) -> u8 {
        match self {
            Self::Main => 0,
            Self::Extra => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Live,
    Playback,
}

pub fn stream_for_view(mode: ViewMode, layout: Layout, profile: LiveProfile) -> StreamKind {
    match mode {
        ViewMode::Playback => StreamKind::Main,
        ViewMode::Live => match profile {
            LiveProfile::Quality => StreamKind::Main,
            LiveProfile::Performance => {
                if layout == Layout::One {
                    StreamKind::Main
                } else {
                    StreamKind::Extra
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn performance_mosaic_uses_substream_quality_uses_main() {
        assert_eq!(
            stream_for_view(ViewMode::Live, Layout::Four, LiveProfile::Performance),
            StreamKind::Extra
        );
        assert_eq!(
            stream_for_view(ViewMode::Live, Layout::Four, LiveProfile::Quality),
            StreamKind::Main
        );
        assert_eq!(
            stream_for_view(ViewMode::Live, Layout::One, LiveProfile::Performance),
            StreamKind::Main
        );
        assert_eq!(
            stream_for_view(ViewMode::Playback, Layout::Four, LiveProfile::Performance),
            StreamKind::Main
        );
    }

    #[test]
    fn intelbras_subtype_matches_dahua() {
        assert_eq!(StreamKind::Main.subtype(), 0);
        assert_eq!(StreamKind::Extra.subtype(), 1);
    }
}
