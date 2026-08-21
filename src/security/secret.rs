use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Senha em memória, apagada no Drop. Debug/Display nunca revelam o valor.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(***)")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_do_not_leak_password() {
        let secret = SecretString::new("hunter2");
        let debug = format!("{secret:?}");
        let display = format!("{secret}");
        assert!(!debug.contains("hunter2"));
        assert!(!display.contains("hunter2"));
        assert!(debug.contains("***"));
        assert_eq!(secret.expose(), "hunter2");
    }
}
