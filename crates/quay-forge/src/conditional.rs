use reqwest::header::HeaderMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CacheValidators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl CacheValidators {
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        let validators = Self {
            etag: header_text(headers, "etag"),
            last_modified: header_text(headers, "last-modified"),
        };
        if validators.is_empty() {
            None
        } else {
            Some(validators)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_modified.is_none()
    }
}

pub fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use reqwest::header::{HeaderMap, HeaderValue};

    use super::CacheValidators;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            if let Ok(value) = HeaderValue::from_str(value) {
                headers.insert(*name, value);
            }
        }
        headers
    }

    #[test]
    fn a_response_without_validators_yields_nothing_to_cache() {
        assert!(CacheValidators::from_headers(&headers(&[])).is_none());
    }

    #[test]
    fn an_etag_alone_is_enough_to_revalidate() {
        match CacheValidators::from_headers(&headers(&[("etag", "W/\"abc\"")])) {
            None => panic!("an etag must be captured"),
            Some(validators) => {
                assert_eq!(validators.etag.as_deref(), Some("W/\"abc\""));
                assert!(validators.last_modified.is_none());
            }
        }
    }

    #[test]
    fn a_last_modified_alone_is_enough_to_revalidate() {
        let captured = CacheValidators::from_headers(&headers(&[(
            "last-modified",
            "Wed, 02 Sep 2026 10:00:00 GMT",
        )]));
        match captured {
            None => panic!("a last-modified must be captured"),
            Some(validators) => {
                assert!(validators.etag.is_none());
                assert!(validators.last_modified.is_some());
            }
        }
    }

    #[test]
    fn an_empty_validator_set_is_reported_as_empty() {
        assert!(CacheValidators::default().is_empty());
    }
}
