use crate::domain::Channel;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionDiff {
    pub start: Vec<Channel>,
    pub stop: Vec<Channel>,
}

/// Quais decoders ligar/desligar ao mudar o mosaico. Câmera fora da tela não decodifica.
pub fn session_diff(old: &[Channel], new: &[Channel]) -> SessionDiff {
    let stop = old
        .iter()
        .copied()
        .filter(|ch| !new.contains(ch))
        .collect();
    let start = new
        .iter()
        .copied()
        .filter(|ch| !old.contains(ch))
        .collect();
    SessionDiff { start, stop }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(n: u8) -> Channel {
        Channel::new(n).unwrap()
    }

    #[test]
    fn empty_to_selection_starts_all() {
        let diff = session_diff(&[], &[ch(1), ch(2)]);
        assert_eq!(diff.start, vec![ch(1), ch(2)]);
        assert!(diff.stop.is_empty());
    }

    #[test]
    fn reorder_does_not_restart_streams() {
        let diff = session_diff(&[ch(1), ch(2)], &[ch(2), ch(1)]);
        assert!(diff.start.is_empty());
        assert!(diff.stop.is_empty());
    }

    #[test]
    fn replace_one_stops_old_and_starts_new() {
        let diff = session_diff(&[ch(1), ch(2), ch(3), ch(4)], &[ch(2), ch(3), ch(4), ch(5)]);
        assert_eq!(diff.stop, vec![ch(1)]);
        assert_eq!(diff.start, vec![ch(5)]);
    }
}
