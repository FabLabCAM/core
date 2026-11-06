//! One connection, one session: never serialized or logged, never shared across endpoints.
use zeroize::Zeroizing;

#[derive(Default)]
pub(super) struct SessionCredentials {
    entry: Option<(String, Zeroizing<String>)>,
}

impl SessionCredentials {
    pub fn clear(&mut self) {
        self.entry = None; // Zeroizing wipes the cached allocation on drop.
    }
    pub fn get(&mut self, identity: &str, load: impl FnOnce() -> Result<String, String>) -> Result<String, String> {
        if let Some((key, value)) = &self.entry {
            if key == identity {
                return Ok(value.to_string());
            }
        }
        self.clear();
        let value = load()?; // Failures are never cached: the next explicit attempt may succeed.
        self.entry = Some((identity.to_owned(), Zeroizing::new(value.clone())));
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_requests_and_tool_returns_read_the_vault_once() {
        let mut cache = SessionCredentials::default();
        assert_eq!(cache.get("Mistral|https://service|model", || Ok("synthetic".into())).unwrap(), "synthetic");
        assert_eq!(cache.get("Mistral|https://service|model", || panic!("Unexpected vault access")).unwrap(), "synthetic");
        cache.clear();
        assert_eq!(cache.get("Mistral|https://service|model", || Ok("rotated".into())).unwrap(), "rotated");
    }

    #[test]
    fn changed_connection_and_failed_reads_cannot_reuse_previous_secret() {
        let mut cache = SessionCredentials::default();
        cache.get("provider|host-a|model", || Ok("old".into())).unwrap();
        assert!(cache.get("provider|host-b|model", || Err("denied".into())).is_err());
        assert_eq!(cache.get("provider|host-a|model", || Ok("new".into())).unwrap(), "new");
        assert_eq!(cache.get("provider|host-a|other-model", || Ok("other".into())).unwrap(), "other");
    }

    #[test]
    fn absence_is_cached_but_a_new_session_reloads() {
        let mut cache = SessionCredentials::default();
        assert_eq!(cache.get("Ollama|localhost|model", || Ok(String::new())).unwrap(), "");
        assert_eq!(cache.get("Ollama|localhost|model", || panic!("Unexpected vault access")).unwrap(), "");
        let mut restarted = SessionCredentials::default();
        assert_eq!(restarted.get("Ollama|localhost|model", || Ok("new".into())).unwrap(), "new");
    }
}
