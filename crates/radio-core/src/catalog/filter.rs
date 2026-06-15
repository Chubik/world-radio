#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchQuery {
    pub name: Option<String>,
    pub countrycode: Option<String>,
    pub language: Option<String>,
    pub tag: Option<String>,
    pub codec: Option<String>,
    pub bitrate_min: Option<u32>,
}

impl SearchQuery {
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut p = Vec::new();
        if let Some(v) = &self.name {
            p.push(("name", v.clone()));
        }
        if let Some(v) = &self.countrycode {
            p.push(("countrycode", v.clone()));
        }
        if let Some(v) = &self.language {
            p.push(("language", v.clone()));
        }
        if let Some(v) = &self.tag {
            p.push(("tag", v.clone()));
        }
        if let Some(v) = &self.codec {
            p.push(("codec", v.clone()));
        }
        if let Some(v) = &self.bitrate_min {
            p.push(("bitrateMin", v.to_string()));
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_params_for_set_fields_only() {
        let q = SearchQuery {
            name: Some("jazz".into()),
            countrycode: Some("FR".into()),
            bitrate_min: Some(128),
            ..Default::default()
        };
        let p = q.to_params();
        assert!(p.contains(&("name", "jazz".to_string())));
        assert!(p.contains(&("countrycode", "FR".to_string())));
        assert!(p.contains(&("bitrateMin", "128".to_string())));
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn empty_query_produces_no_params() {
        assert!(SearchQuery::default().to_params().is_empty());
    }

    #[test]
    fn builds_params_for_language_tag_codec() {
        let q = SearchQuery {
            language: Some("french".into()),
            tag: Some("jazz".into()),
            codec: Some("MP3".into()),
            ..Default::default()
        };
        let p = q.to_params();
        assert!(p.contains(&("language", "french".to_string())));
        assert!(p.contains(&("tag", "jazz".to_string())));
        assert!(p.contains(&("codec", "MP3".to_string())));
        assert_eq!(p.len(), 3);
    }
}
