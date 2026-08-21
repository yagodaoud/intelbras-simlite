//! Credenciais: nunca em log, Display/Debug ou arquivo de config.

mod redact;
mod secret;
mod store;

pub use redact::redact_secrets_in_text;
pub use secret::SecretString;
pub use store::{CredentialStore, KeyringStore, MemoryStore, StoreError};
