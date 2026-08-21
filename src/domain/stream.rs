use crate::domain::Layout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamKind {
    /// Stream extra (sub) — mosaico leve.
    Extra,
    /// Stream principal — 1 câmera ou playback.
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

pub fn stream_for_view(mode: ViewMode, layout: Layout) -> StreamKind {
    match (mode, layout) {
        (ViewMode::Playback, _) | (ViewMode::Live, Layout::One) => StreamKind::Main,
        (ViewMode::Live, _) => StreamKind::Extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mosaic_uses_substream_single_uses_main() {
        assert_eq!(
            stream_for_view(ViewMode::Live, Layout::Four),
            StreamKind::Extra
        );
        assert_eq!(
            stream_for_view(ViewMode::Live, Layout::Two),
            StreamKind::Extra
        );
        assert_eq!(
            stream_for_view(ViewMode::Live, Layout::One),
            StreamKind::Main
        );
        assert_eq!(
            stream_for_view(ViewMode::Playback, Layout::Four),
            StreamKind::Main
        );
    }

    #[test]
    fn intelbras_subtype_matches_dahua() {
        assert_eq!(StreamKind::Main.subtype(), 0);
        assert_eq!(StreamKind::Extra.subtype(), 1);
    }
}
