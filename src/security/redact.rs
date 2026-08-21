/// Remove userinfo de URLs RTSP e valores de `password=` em texto de log/stderr.
pub fn redact_secrets_in_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find("rtsp://") {
        out.push_str(&rest[..idx]);
        out.push_str("rtsp://");
        let after = &rest[idx + 7..];
        let authority_end = after
            .find(|c: char| c == '/' || c.is_whitespace())
            .unwrap_or(after.len());
        let authority = &after[..authority_end];
        if let Some(at) = authority.rfind('@') {
            out.push_str("***@");
            out.push_str(&authority[at + 1..]);
        } else {
            out.push_str(authority);
        }
        rest = &after[authority_end..];
    }
    out.push_str(rest);
    redact_password_query(&out)
}

fn redact_password_query(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        if let Some(rel) = lower[i..].find("password=") {
            let start = i + rel;
            out.push_str(&input[i..start]);
            out.push_str("password=***");
            let value_start = start + "password=".len();
            let mut j = value_start;
            while j < bytes.len() && !matches!(bytes[j], b'&' | b' ' | b'\n' | b'\r' | b'\t') {
                j += 1;
            }
            i = j;
        } else {
            out.push_str(&input[i..]);
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_rtsp_userinfo() {
        let raw = "Opening rtsp://admin:hunter2@192.168.0.10:554/cam/realmonitor?channel=1";
        let redacted = redact_secrets_in_text(raw);
        assert!(!redacted.contains("hunter2"));
        assert!(!redacted.contains("admin:"));
        assert!(redacted.contains("rtsp://***@192.168.0.10:554/cam/realmonitor?channel=1"));
    }

    #[test]
    fn redacts_encoded_password_with_special_chars() {
        let raw = "rtsp://viewer:p%40ss@10.0.0.1/cam/playback";
        let redacted = redact_secrets_in_text(raw);
        assert!(!redacted.contains("p%40ss"));
        assert!(redacted.contains("rtsp://***@10.0.0.1/cam/playback"));
    }

    #[test]
    fn redacts_password_query_param() {
        let raw = "GET /login?user=admin&password=hunter2&x=1";
        let redacted = redact_secrets_in_text(raw);
        assert!(!redacted.contains("hunter2"));
        assert!(redacted.contains("password=***"));
    }
}
