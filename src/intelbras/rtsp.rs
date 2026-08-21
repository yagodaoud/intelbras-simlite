use std::fmt;

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::domain::{Device, PlaybackQuery, StreamKind};
use crate::security::SecretString;

/// URL RTSP cuja senha só aparece em `expose()`. Display/Debug são redigidos.
pub struct RtspUrl {
    inner: String,
}

impl RtspUrl {
    fn new(inner: String) -> Self {
        Self { inner }
    }

    pub fn expose(&self) -> &str {
        &self.inner
    }
}

impl fmt::Debug for RtspUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::security::redact_secrets_in_text(&self.inner))
    }
}

impl fmt::Display for RtspUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::security::redact_secrets_in_text(&self.inner))
    }
}

pub fn live_url(device: &Device, secret: &SecretString, channel: crate::domain::Channel, stream: StreamKind) -> RtspUrl {
    let auth = userinfo(&device.username, secret);
    RtspUrl::new(format!(
        "rtsp://{auth}@{host}:{port}/cam/realmonitor?channel={ch}&subtype={sub}",
        host = device.host,
        port = device.rtsp_port,
        ch = channel.get(),
        sub = stream.subtype(),
    ))
}

pub fn playback_url(device: &Device, secret: &SecretString, query: &PlaybackQuery) -> RtspUrl {
    let auth = userinfo(&device.username, secret);
    RtspUrl::new(format!(
        "rtsp://{auth}@{host}:{port}/cam/playback?channel={ch}&starttime={start}&endtime={end}",
        host = device.host,
        port = device.rtsp_port,
        ch = query.channel.get(),
        start = query.rtsp_start(),
        end = query.rtsp_end(),
    ))
}

fn userinfo(username: &str, secret: &SecretString) -> String {
    format!(
        "{}:{}",
        utf8_percent_encode(username, NON_ALPHANUMERIC),
        utf8_percent_encode(secret.expose(), NON_ALPHANUMERIC)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Channel, Device};
    use crate::security::SecretString;
    use chrono::NaiveDate;

    fn device() -> Device {
        Device::new("dvr-casa", "Casa", "192.168.0.10", 554, 80, "viewer", 6).unwrap()
    }

    fn secret() -> SecretString {
        SecretString::new("hunter2")
    }

    #[test]
    fn live_extra_stream_url() {
        let url = live_url(
            &device(),
            &secret(),
            Channel::new(3).unwrap(),
            StreamKind::Extra,
        );
        assert_eq!(
            url.expose(),
            "rtsp://viewer:hunter2@192.168.0.10:554/cam/realmonitor?channel=3&subtype=1"
        );
        assert!(!format!("{url}").contains("hunter2"));
        assert!(!format!("{url:?}").contains("hunter2"));
    }

    #[test]
    fn encodes_special_characters_in_password() {
        let url = live_url(
            &device(),
            &SecretString::new("p@ss:w d"),
            Channel::new(1).unwrap(),
            StreamKind::Main,
        );
        assert!(url.expose().contains("p%40ss%3Aw%20d"));
        assert!(!url.expose().contains("p@ss"));
        assert!(!format!("{url}").contains("p%40ss"));
    }

    #[test]
    fn playback_uses_dahua_timestamp_layout() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 21)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 8, 21)
            .unwrap()
            .and_hms_opt(10, 15, 0)
            .unwrap();
        let query = PlaybackQuery::new(Channel::new(1).unwrap(), start, end).unwrap();
        let url = playback_url(&device(), &secret(), &query);
        assert_eq!(
            url.expose(),
            "rtsp://viewer:hunter2@192.168.0.10:554/cam/playback?channel=1&starttime=2026_08_21_10_00_00&endtime=2026_08_21_10_15_00"
        );
        assert!(!format!("{url}").contains("hunter2"));
    }

    #[test]
    fn channel_is_numeric_never_interpolated_from_raw_string() {
        let url = live_url(
            &device(),
            &secret(),
            Channel::new(1).unwrap(),
            StreamKind::Extra,
        );
        assert!(!url.expose().contains("channel=1&subtype=1&evil"));
        assert!(url.expose().contains("channel=1&subtype=1"));
    }
}
