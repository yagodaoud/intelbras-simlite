use std::collections::BTreeMap;

/// Parser do formato `key=value` da CGI Dahua/Intelbras.
pub fn parse_kvp(body: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in body.lines() {
        let line = line.trim().trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

pub fn grouped_items(map: &BTreeMap<String, String>) -> Vec<BTreeMap<String, String>> {
    let mut by_index: BTreeMap<usize, BTreeMap<String, String>> = BTreeMap::new();
    for (key, value) in map {
        let Some(rest) = key.strip_prefix("items[") else {
            continue;
        };
        let Some((idx_str, field)) = rest.split_once("].") else {
            continue;
        };
        let Ok(idx) = idx_str.parse::<usize>() else {
            continue;
        };
        by_index.entry(idx).or_default().insert(field.to_string(), value.clone());
    }
    by_index.into_values().collect()
}

pub fn is_safe_cgi_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 64
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_factory_token() {
        let map = parse_kvp("result=1A2b3C\r\n");
        assert_eq!(map.get("result").map(String::as_str), Some("1A2b3C"));
        assert!(is_safe_cgi_token("1A2b3C"));
        assert!(!is_safe_cgi_token("1&action=destroy"));
        assert!(!is_safe_cgi_token(""));
    }

    #[test]
    fn groups_recording_items() {
        let body = "\
found=2\r
items[0].Channel=0\r
items[0].StartTime=2021-10-04 09:00:02\r
items[0].EndTime=2021-10-04 10:00:02\r
items[0].FilePath=/mnt/dvr/a.dav\r
items[0].Length=100\r
items[1].Channel=0\r
items[1].StartTime=2021-10-04 10:00:02\r
items[1].EndTime=2021-10-04 11:00:02\r
items[1].FilePath=/mnt/dvr/b.dav\r
items[1].Length=200\r
";
        let items = grouped_items(&parse_kvp(body));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].get("FilePath").unwrap(), "/mnt/dvr/a.dav");
        assert_eq!(items[1].get("Length").unwrap(), "200");
    }
}
