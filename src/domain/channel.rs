use std::fmt;

use crate::domain::DomainError;

/// Canal 1-based do DVR (RTSP usa este índice).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Channel(u8);

impl Channel {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 128;

    pub fn new(n: u8) -> Result<Self, DomainError> {
        if (Self::MIN..=Self::MAX).contains(&n) {
            Ok(Self(n))
        } else {
            Err(DomainError::InvalidChannel(n))
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }

    /// Índice 0-based da CGI Dahua/Intelbras (`condition.Channel`).
    pub fn cgi_index(self) -> u8 {
        self.0 - 1
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Channel({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_range_1_to_128() {
        assert!(Channel::new(1).is_ok());
        assert!(Channel::new(6).is_ok());
        assert!(Channel::new(128).is_ok());
    }

    #[test]
    fn rejects_zero_and_overflow() {
        assert_eq!(Channel::new(0), Err(DomainError::InvalidChannel(0)));
        assert_eq!(Channel::new(129), Err(DomainError::InvalidChannel(129)));
    }

    #[test]
    fn cgi_index_is_zero_based() {
        assert_eq!(Channel::new(1).unwrap().cgi_index(), 0);
        assert_eq!(Channel::new(6).unwrap().cgi_index(), 5);
    }
}
