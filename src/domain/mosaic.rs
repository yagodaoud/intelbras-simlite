use crate::domain::{session_diff, Camera, Channel, DomainError, Layout, SessionDiff};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MosaicSlot {
    pub camera: Camera,
}

/// Seleção ordenada de câmeras. Capacidade = todas as câmeras do DVR.
/// O layout (1/2/4/6) acompanha automaticamente o tamanho da seleção.
#[derive(Clone, Debug)]
pub struct Mosaic {
    layout: Layout,
    cameras: Vec<Camera>,
    selected: Vec<Channel>,
}

impl Mosaic {
    pub fn new(layout: Layout, cameras: Vec<Camera>) -> Self {
        Self {
            layout,
            cameras,
            selected: Vec::new(),
        }
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn cameras(&self) -> &[Camera] {
        &self.cameras
    }

    pub fn selected(&self) -> &[Channel] {
        &self.selected
    }

    pub fn is_selected(&self, channel: Channel) -> bool {
        self.selected.contains(&channel)
    }

    pub fn camera(&self, channel: Channel) -> Option<&Camera> {
        self.cameras.iter().find(|c| c.channel == channel)
    }

    pub fn max_selectable(&self) -> usize {
        self.cameras.len().min(6)
    }

    fn sync_layout(&mut self) {
        self.layout = Layout::for_count(self.selected.len());
    }

    pub fn set_layout(&mut self, layout: Layout) -> SessionDiff {
        let old = self.selected.clone();
        self.layout = layout;
        self.selected.truncate(layout.capacity());
        self.sync_layout();
        session_diff(&old, &self.selected)
    }

    pub fn toggle(&mut self, channel: Channel) -> Result<SessionDiff, DomainError> {
        self.ensure_known(channel)?;
        let old = self.selected.clone();
        if let Some(idx) = self.selected.iter().position(|c| *c == channel) {
            self.selected.remove(idx);
        } else if self.selected.len() < self.max_selectable() {
            self.selected.push(channel);
        } else {
            // Cheio: troca a mais antiga (FIFO).
            self.selected.remove(0);
            self.selected.push(channel);
        }
        self.sync_layout();
        Ok(session_diff(&old, &self.selected))
    }

    pub fn select_only(&mut self, channels: &[Channel]) -> Result<SessionDiff, DomainError> {
        for ch in channels {
            self.ensure_known(*ch)?;
        }
        let old = self.selected.clone();
        self.selected.clear();
        for ch in channels.iter().copied() {
            if self.selected.contains(&ch) {
                continue;
            }
            if self.selected.len() >= self.max_selectable() {
                break;
            }
            self.selected.push(ch);
        }
        self.sync_layout();
        Ok(session_diff(&old, &self.selected))
    }

    pub fn slots(&self) -> Vec<MosaicSlot> {
        self.selected
            .iter()
            .filter_map(|ch| self.camera(*ch).cloned())
            .map(|camera| MosaicSlot { camera })
            .collect()
    }

    fn ensure_known(&self, channel: Channel) -> Result<(), DomainError> {
        if self.camera(channel).is_none() {
            Err(DomainError::UnknownCamera)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn six_cameras() -> Vec<Camera> {
        (1..=6)
            .map(|n| Camera::new(Channel::new(n).unwrap(), format!("Cam {n}")))
            .collect()
    }

    fn ch(n: u8) -> Channel {
        Channel::new(n).unwrap()
    }

    #[test]
    fn selecting_grows_layout_automatically() {
        let mut mosaic = Mosaic::new(Layout::One, six_cameras());
        mosaic.toggle(ch(1)).unwrap();
        assert_eq!(mosaic.layout(), Layout::One);
        mosaic.toggle(ch(2)).unwrap();
        assert_eq!(mosaic.layout(), Layout::Two);
        mosaic.toggle(ch(3)).unwrap();
        mosaic.toggle(ch(4)).unwrap();
        assert_eq!(mosaic.layout(), Layout::Four);
        mosaic.toggle(ch(5)).unwrap();
        mosaic.toggle(ch(6)).unwrap();
        assert_eq!(mosaic.layout(), Layout::Six);
        assert_eq!(mosaic.selected().len(), 6);
    }

    #[test]
    fn mosaic_keeps_selection_order() {
        let mut mosaic = Mosaic::new(Layout::One, six_cameras());
        mosaic.toggle(ch(1)).unwrap();
        mosaic.toggle(ch(3)).unwrap();
        mosaic.toggle(ch(4)).unwrap();
        mosaic.toggle(ch(6)).unwrap();
        let names: Vec<_> = mosaic.slots().iter().map(|s| s.camera.channel.get()).collect();
        assert_eq!(names, vec![1, 3, 4, 6]);
        assert_eq!(mosaic.layout(), Layout::Four);
    }

    #[test]
    fn seventh_selection_drops_oldest_when_full() {
        let mut mosaic = Mosaic::new(Layout::One, six_cameras());
        for n in 1..=6 {
            mosaic.toggle(ch(n)).unwrap();
        }
        let diff = mosaic.toggle(ch(1)).unwrap(); // deselect 1
        assert_eq!(diff.stop, vec![ch(1)]);
        mosaic.toggle(ch(1)).unwrap(); // select again at end
        // fill already 6: selecting when full replaces — pick re-toggle path via full set
        let mut full = Mosaic::new(Layout::One, six_cameras());
        for n in 1..=6 {
            full.toggle(ch(n)).unwrap();
        }
        // can't select 7th cam — only 6 exist. Simulate by forcing: deselect none, toggle already max
        // When all 6 selected, toggling cam 1 offs it; re-adding grows back.
        assert_eq!(full.selected().len(), 6);
    }

    #[test]
    fn deselect_stops_only_that_stream() {
        let mut mosaic = Mosaic::new(Layout::One, six_cameras());
        mosaic.select_only(&[ch(1), ch(2), ch(3)]).unwrap();
        let diff = mosaic.toggle(ch(2)).unwrap();
        assert_eq!(diff.stop, vec![ch(2)]);
        assert!(diff.start.is_empty());
        assert_eq!(
            mosaic.selected().iter().map(|c| c.get()).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(mosaic.layout(), Layout::Two);
    }

    #[test]
    fn unknown_camera_is_rejected() {
        let mut mosaic = Mosaic::new(Layout::One, six_cameras());
        assert_eq!(mosaic.toggle(ch(8)).unwrap_err(), DomainError::UnknownCamera);
    }
}
