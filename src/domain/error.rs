#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("canal inválido (use 1-128)")]
    InvalidChannel(u8),
    #[error("host inválido")]
    InvalidHost,
    #[error("usuário inválido")]
    InvalidUsername,
    #[error("id de dispositivo inválido")]
    InvalidDeviceId,
    #[error("porta inválida")]
    InvalidPort,
    #[error("câmera desconhecida")]
    UnknownCamera,
    #[error("intervalo de reprodução inválido")]
    InvalidPlaybackRange,
}
