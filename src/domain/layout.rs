use serde::{Deserialize, Serialize};

/// Forma do mosaico, derivada de quantas câmeras estão selecionadas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    One,
    Two,
    Four,
    Six,
}

impl Layout {
    pub fn capacity(self) -> usize {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
            Self::Six => 6,
        }
    }

    pub fn columns(self) -> usize {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 2,
            Self::Six => 3,
        }
    }

    pub fn rows(self) -> usize {
        match self {
            Self::One => 1,
            Self::Two => 1,
            Self::Four => 2,
            Self::Six => 2,
        }
    }

    /// 1 → 1x1, 2 → 2x1, 3–4 → 2x2, 5–6 → 3x2.
    pub fn for_count(n: usize) -> Self {
        match n {
            0 | 1 => Self::One,
            2 => Self::Two,
            3 | 4 => Self::Four,
            _ => Self::Six,
        }
    }

    pub fn from_capacity(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::One),
            2 => Some(Self::Two),
            4 => Some(Self::Four),
            6 => Some(Self::Six),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_grid_grows_with_selection() {
        assert_eq!(Layout::for_count(1).columns(), 1);
        assert_eq!(Layout::for_count(2).columns(), 2);
        assert_eq!(Layout::for_count(2).rows(), 1);
        assert_eq!(Layout::for_count(4).columns(), 2);
        assert_eq!(Layout::for_count(6).columns(), 3);
        assert_eq!(Layout::for_count(6).capacity(), 6);
    }
}
