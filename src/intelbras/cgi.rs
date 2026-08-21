use chrono::NaiveDateTime;

use crate::domain::{parse_cgi_ts, Channel, Device, PlaybackQuery};
use crate::intelbras::digest::DigestHttp;
use crate::intelbras::kvp::{grouped_items, is_safe_cgi_token, parse_kvp};
use crate::security::SecretString;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recording {
    pub channel: Channel,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub bytes: Option<u64>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RecordingFinderError {
    #[error("falha ao falar com o DVR")]
    Transport,
    #[error("autenticação recusada")]
    Unauthorized,
    #[error("resposta do DVR inválida")]
    Protocol,
}

pub trait CgiTransport {
    fn get(&self, path_and_query: &str) -> Result<String, RecordingFinderError>;
}

pub fn find_recordings<T: CgiTransport>(
    transport: &T,
    query: &PlaybackQuery,
) -> Result<Vec<Recording>, RecordingFinderError> {
    let created = transport.get("/cgi-bin/mediaFileFind.cgi?action=factory.create")?;
    let token = parse_kvp(&created)
        .remove("result")
        .ok_or(RecordingFinderError::Protocol)?;
    if !is_safe_cgi_token(&token) {
        return Err(RecordingFinderError::Protocol);
    }

    let find = format!(
        "/cgi-bin/mediaFileFind.cgi?action=findFile&object={token}&condition.Channel={cgi}&condition.StartTime={start}&condition.EndTime={end}&condition.Types[0]=dav",
        cgi = query.channel.cgi_index(),
        start = encode_cgi_time(&query.cgi_start()),
        end = encode_cgi_time(&query.cgi_end()),
    );
    let find_body = transport.get(&find)?;
    if !find_body.trim().eq_ignore_ascii_case("ok") && !parse_kvp(&find_body).contains_key("found") {
        let lower = find_body.to_ascii_lowercase();
        if lower.contains("unauthorized") || lower.contains("unauthenticated") {
            return Err(RecordingFinderError::Unauthorized);
        }
    }

    let mut recordings = Vec::new();
    loop {
        let page = transport.get(&format!(
            "/cgi-bin/mediaFileFind.cgi?action=findNextFile&object={token}&count=100"
        ))?;
        let map = parse_kvp(&page);
        let found = map
            .get("found")
            .or_else(|| map.get("count"))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        if found == 0 {
            break;
        }
        for item in grouped_items(&map) {
            if let Some(rec) = parse_recording(&item) {
                recordings.push(rec);
            }
        }
        if found < 100 {
            break;
        }
    }

    let _ = transport.get(&format!(
        "/cgi-bin/mediaFileFind.cgi?action=close&object={token}"
    ));
    let _ = transport.get(&format!(
        "/cgi-bin/mediaFileFind.cgi?action=destroy&object={token}"
    ));
    Ok(recordings)
}

fn encode_cgi_time(ts: &str) -> String {
    ts.replace(' ', "%20")
}

fn parse_recording(item: &std::collections::BTreeMap<String, String>) -> Option<Recording> {
    let cgi_channel = item.get("Channel")?.parse::<u8>().ok()?;
    let channel = Channel::new(cgi_channel.saturating_add(1)).ok()?;
    let start = parse_cgi_ts(item.get("StartTime")?).ok()?;
    let end = parse_cgi_ts(item.get("EndTime")?).ok()?;
    if end <= start {
        return None;
    }
    let bytes = item.get("Length").and_then(|s| s.parse().ok());
    Some(Recording {
        channel,
        start,
        end,
        bytes,
    })
}

/// Transporte HTTP real (digest) contra o DVR na LAN.
pub struct DigestCgi {
    inner: DigestHttp,
}

impl DigestCgi {
    pub fn new(device: &Device, secret: &SecretString) -> Self {
        Self {
            inner: DigestHttp::new(&device.host, device.http_port, &device.username, secret),
        }
    }
}

impl CgiTransport for DigestCgi {
    fn get(&self, path_and_query: &str) -> Result<String, RecordingFinderError> {
        self.inner.get(path_and_query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    struct Scripted {
        calls: RefCell<Vec<String>>,
        responses: RefCell<VecDeque<Result<String, RecordingFinderError>>>,
    }

    impl Scripted {
        fn new(responses: Vec<Result<String, RecordingFinderError>>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                responses: RefCell::new(responses.into()),
            }
        }
    }

    impl CgiTransport for Scripted {
        fn get(&self, path_and_query: &str) -> Result<String, RecordingFinderError> {
            self.calls.borrow_mut().push(path_and_query.to_string());
            self.responses
                .borrow_mut()
                .pop_front()
                .unwrap_or(Err(RecordingFinderError::Transport))
        }
    }

    fn query() -> PlaybackQuery {
        let start = NaiveDate::from_ymd_opt(2021, 10, 4)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let end = NaiveDate::from_ymd_opt(2021, 10, 4)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        PlaybackQuery::new(Channel::new(5).unwrap(), start, end).unwrap()
    }

    #[test]
    fn walks_create_find_next_close_and_maps_zero_based_channel() {
        let body = "\
found=1
items[0].Channel=4
items[0].StartTime=2021-10-04 09:00:02
items[0].EndTime=2021-10-04 10:00:02
items[0].Length=953417728
";
        let t = Scripted::new(vec![
            Ok("result=1A2b3C\r\n".into()),
            Ok("OK\r\n".into()),
            Ok(body.into()),
            Ok("OK\r\n".into()),
            Ok("OK\r\n".into()),
        ]);
        let recs = find_recordings(&t, &query()).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].channel.get(), 5);
        let calls = t.calls.borrow();
        assert!(calls[0].contains("action=factory.create"));
        assert!(calls[1].contains("object=1A2b3C"));
        assert!(calls[1].contains("condition.Channel=4"));
        assert!(calls[1].contains("StartTime=2021-10-04%2009:00:00"));
        assert!(calls[2].contains("findNextFile"));
        assert!(calls[3].contains("action=close"));
        assert!(calls[4].contains("action=destroy"));
    }

    #[test]
    fn rejects_injected_cgi_token() {
        let t = Scripted::new(vec![Ok("result=1&action=destroy\r\n".into())]);
        assert_eq!(
            find_recordings(&t, &query()).unwrap_err(),
            RecordingFinderError::Protocol
        );
        assert_eq!(t.calls.borrow().len(), 1);
    }

    #[test]
    fn space_in_cgi_time_is_encoded_not_raw() {
        let t = Scripted::new(vec![
            Ok("result=abc".into()),
            Ok("OK".into()),
            Ok("found=0".into()),
            Ok("OK".into()),
            Ok("OK".into()),
        ]);
        find_recordings(&t, &query()).unwrap();
        let find = &t.calls.borrow()[1];
        assert!(!find.contains("StartTime=2021-10-04 09:00:00"));
        assert!(find.contains("StartTime=2021-10-04%2009:00:00"));
    }
}
