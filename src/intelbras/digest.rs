use crate::intelbras::cgi::RecordingFinderError;
use crate::security::SecretString;

pub struct DigestHttp {
    host: String,
    port: u16,
    username: String,
    password: SecretString,
}

impl DigestHttp {
    pub fn new(host: &str, port: u16, username: &str, password: &SecretString) -> Self {
        Self {
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: SecretString::new(password.expose()),
        }
    }

    pub fn get(&self, path_and_query: &str) -> Result<String, RecordingFinderError> {
        if !is_safe_path(path_and_query) {
            return Err(RecordingFinderError::Protocol);
        }
        let url = format!("http://{}:{}{}", self.host, self.port, path_and_query);
        match ureq::get(&url).call() {
            Ok(resp) => read_body(resp),
            Err(ureq::Error::Status(401, resp)) => self.retry_digest(&url, path_and_query, resp),
            Err(ureq::Error::Status(403, _)) => Err(RecordingFinderError::Unauthorized),
            Err(_) => Err(RecordingFinderError::Transport),
        }
    }

    fn retry_digest(
        &self,
        url: &str,
        uri: &str,
        resp: ureq::Response,
    ) -> Result<String, RecordingFinderError> {
        let header = resp
            .header("www-authenticate")
            .ok_or(RecordingFinderError::Unauthorized)?
            .to_string();
        let mut prompt =
            digest_auth::parse(&header).map_err(|_| RecordingFinderError::Protocol)?;
        let context = digest_auth::AuthContext::new(&self.username, self.password.expose(), uri);
        let answer = prompt
            .respond(&context)
            .map_err(|_| RecordingFinderError::Unauthorized)?;
        match ureq::get(url)
            .set("Authorization", &answer.to_string())
            .call()
        {
            Ok(resp) => read_body(resp),
            Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => {
                Err(RecordingFinderError::Unauthorized)
            }
            Err(_) => Err(RecordingFinderError::Transport),
        }
    }
}

fn read_body(resp: ureq::Response) -> Result<String, RecordingFinderError> {
    resp.into_string().map_err(|_| RecordingFinderError::Transport)
}

fn is_safe_path(path: &str) -> bool {
    path.starts_with("/cgi-bin/")
        && !path.contains("://")
        && !path.contains("..")
        && !path.contains('\n')
        && !path.contains('\r')
        && path.len() < 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_open_redirect_and_traversal() {
        assert!(!is_safe_path("http://evil/cgi-bin/x"));
        assert!(!is_safe_path("/cgi-bin/../etc/passwd"));
        assert!(!is_safe_path("/cgi-bin/x\r\nHost: evil"));
        assert!(is_safe_path(
            "/cgi-bin/mediaFileFind.cgi?action=factory.create"
        ));
    }
}
