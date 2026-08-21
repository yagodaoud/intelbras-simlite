use crate::domain::DomainError;

/// Endereço do DVR/NVR. Porta fica em campo próprio para não misturar com IPv6.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub host: String,
    pub rtsp_port: u16,
    pub http_port: u16,
    pub username: String,
    pub channel_count: u8,
}

impl Device {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        host: impl Into<String>,
        rtsp_port: u16,
        http_port: u16,
        username: impl Into<String>,
        channel_count: u8,
    ) -> Result<Self, DomainError> {
        let (host, port_from_host) = split_host_port(&host.into())?;
        let host = sanitize_host(&host)?;
        let username = sanitize_username(&username.into())?;
        let id = sanitize_id(&id.into())?;
        if !(1..=128).contains(&channel_count) {
            return Err(DomainError::InvalidChannel(channel_count));
        }
        let rtsp_port = port_from_host.unwrap_or(rtsp_port);
        if rtsp_port == 0 || http_port == 0 {
            return Err(DomainError::InvalidPort);
        }
        Ok(Self {
            id,
            name: name.into(),
            host,
            rtsp_port,
            http_port,
            username,
            channel_count,
        })
    }
}

fn has_control_or_url_meta(s: &str) -> bool {
    s.bytes().any(|b| {
        b < 32
            || b == 127
            || b == b'@'
            || b == b'/'
            || b == b'\\'
            || b == b'?'
            || b == b'#'
            || b == b' '
            || b == b'%'
    })
}

/// Aceita `192.168.0.10` ou `192.168.0.10:554` (porta vai pro campo RTSP).
fn split_host_port(raw: &str) -> Result<(String, Option<u16>), DomainError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(DomainError::InvalidHost);
    }
    // IPv6 entre colchetes: [fe80::1]:554
    if let Some(rest) = raw.strip_prefix('[') {
        let Some((host, after)) = rest.split_once(']') else {
            return Err(DomainError::InvalidHost);
        };
        if after.is_empty() {
            return Ok((format!("[{host}]"), None));
        }
        let Some(port_str) = after.strip_prefix(':') else {
            return Err(DomainError::InvalidHost);
        };
        let port: u16 = port_str.parse().map_err(|_| DomainError::InvalidPort)?;
        if port == 0 {
            return Err(DomainError::InvalidPort);
        }
        return Ok((format!("[{host}]"), Some(port)));
    }
    // host:port — só um ':' e a parte depois é número
    if let Some((host, port_str)) = raw.rsplit_once(':')
        && !host.is_empty()
        && port_str.chars().all(|c| c.is_ascii_digit())
        && !host.contains(':')
    {
        let port: u16 = port_str.parse().map_err(|_| DomainError::InvalidPort)?;
        if port == 0 {
            return Err(DomainError::InvalidPort);
        }
        return Ok((host.to_string(), Some(port)));
    }
    Ok((raw.to_string(), None))
}

fn sanitize_host(host: &str) -> Result<String, DomainError> {
    let host = host.trim();
    if host.is_empty() || has_control_or_url_meta(host) {
        return Err(DomainError::InvalidHost);
    }
    Ok(host.to_string())
}

fn sanitize_username(user: &str) -> Result<String, DomainError> {
    let user = user.trim();
    if user.is_empty() || has_control_or_url_meta(user) || user.contains(':') {
        return Err(DomainError::InvalidUsername);
    }
    Ok(user.to_string())
}

fn sanitize_id(id: &str) -> Result<String, DomainError> {
    let id = id.trim();
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(DomainError::InvalidDeviceId);
    }
    Ok(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_device(host: &str, user: &str) -> Result<Device, DomainError> {
        Device::new("dvr-casa", "Casa", host, 554, 80, user, 6)
    }

    #[test]
    fn accepts_ipv4_and_hostname() {
        assert!(ok_device("192.168.0.10", "viewer").is_ok());
        assert!(ok_device("dvr.local", "viewer").is_ok());
    }

    #[test]
    fn splits_host_and_port_when_pasted_together() {
        let d = Device::new("dvr-casa", "Casa", "192.168.0.10:554", 80, 80, "viewer", 6).unwrap();
        assert_eq!(d.host, "192.168.0.10");
        assert_eq!(d.rtsp_port, 554);
    }

    #[test]
    fn rejects_credentials_smuggled_in_host() {
        let err = ok_device("admin:hunter2@192.168.0.10", "viewer").unwrap_err();
        assert_eq!(err, DomainError::InvalidHost);
        let shown = err.to_string();
        assert!(!shown.contains("hunter2"));
        assert!(!shown.contains("admin:"));
    }

    #[test]
    fn rejects_username_with_colon_or_at() {
        assert_eq!(
            ok_device("192.168.0.10", "ad:min").unwrap_err(),
            DomainError::InvalidUsername
        );
        assert_eq!(
            ok_device("192.168.0.10", "ad@min").unwrap_err(),
            DomainError::InvalidUsername
        );
    }

    #[test]
    fn rejects_crlf_injection() {
        assert!(ok_device("192.168.0.10\r\nEvil", "viewer").is_err());
    }
}
