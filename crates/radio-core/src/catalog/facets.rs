#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facets {
    pub countries: Vec<(String, u32)>,
    pub tags: Vec<(String, u32)>,
    pub codecs: Vec<(String, u32)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_facets_are_empty() {
        let f = Facets::default();
        assert!(f.countries.is_empty());
        assert!(f.tags.is_empty());
        assert!(f.codecs.is_empty());
    }
}
