//! Sequence metadata for documents.
//!
//! [`SequenceMetadata`] holds optional, source-derived metadata grouped so
//! that "what came from the file" is one value. Unknown key/value pairs from
//! a lossless import land in `extras`. See ADR-0007
//! (`docs/decisions/ADR-0007-sequence-document.md`).

use std::collections::BTreeMap;

/// Optional, source-derived metadata, grouped so "what came from the file"
/// is one value. Everything here may legitimately be absent.
///
/// Construct via [`SequenceMetadata::builder()`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SequenceMetadata {
    description: Option<String>,
    accession: Option<String>,
    organism: Option<String>,
    genetic_code: Option<u32>,
    taxonomy: Option<Vec<String>>,
    extras: BTreeMap<String, String>,
}

impl SequenceMetadata {
    /// Returns a new builder with all fields `None` / empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::SequenceMetadata;
    ///
    /// let metadata = SequenceMetadata::builder()
    ///     .accession("AF000017.1")
    ///     .organism("Homo sapiens")
    ///     .genetic_code(1)
    ///     .taxonomy(vec!["Eukaryota".to_owned(), "Metazoa".to_owned()])
    ///     .extra("keywords", "beta-galactosidase")
    ///     .build();
    ///
    /// assert_eq!(metadata.accession(), Some("AF000017.1"));
    /// assert_eq!(metadata.organism(), Some("Homo sapiens"));
    /// assert_eq!(metadata.genetic_code(), Some(1));
    /// assert_eq!(metadata.taxonomy(), Some(&["Eukaryota".to_owned(), "Metazoa".to_owned()][..]));
    /// assert_eq!(metadata.extras().get("keywords").map(String::as_str), Some("beta-galactosidase"));
    /// ```
    #[must_use]
    pub fn builder() -> SequenceMetadataBuilder {
        SequenceMetadataBuilder::default()
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    #[must_use]
    pub fn accession(&self) -> Option<&str> {
        self.accession.as_deref()
    }
    #[must_use]
    pub fn organism(&self) -> Option<&str> {
        self.organism.as_deref()
    }
    #[must_use]
    pub const fn genetic_code(&self) -> Option<u32> {
        self.genetic_code
    }
    #[must_use]
    pub fn taxonomy(&self) -> Option<&[String]> {
        self.taxonomy.as_deref()
    }
    #[must_use]
    pub const fn extras(&self) -> &BTreeMap<String, String> {
        &self.extras
    }
}

/// Infallible builder for [`SequenceMetadata`].
///
/// Every setter consumes and returns `Self` for chaining.
/// Call [`build()`](Self::build) to produce the final [`SequenceMetadata`].
#[derive(Debug, Clone, Default)]
pub struct SequenceMetadataBuilder {
    description: Option<String>,
    accession: Option<String>,
    organism: Option<String>,
    genetic_code: Option<u32>,
    taxonomy: Option<Vec<String>>,
    extras: BTreeMap<String, String>,
}

impl SequenceMetadataBuilder {
    #[must_use]
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }

    #[must_use]
    pub fn accession(mut self, value: impl Into<String>) -> Self {
        self.accession = Some(value.into());
        self
    }

    #[must_use]
    pub fn organism(mut self, value: impl Into<String>) -> Self {
        self.organism = Some(value.into());
        self
    }

    #[must_use]
    pub const fn genetic_code(mut self, value: u32) -> Self {
        self.genetic_code = Some(value);
        self
    }

    #[must_use]
    pub fn taxonomy(mut self, lineage: Vec<String>) -> Self {
        self.taxonomy = Some(lineage);
        self
    }

    /// Insert one key/value pair into `extras` (overwrites an existing key).
    #[must_use]
    pub fn extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extras.insert(key.into(), value.into());
        self
    }

    /// Replace the whole `extras` map.
    #[must_use]
    pub fn extras(mut self, map: BTreeMap<String, String>) -> Self {
        self.extras = map;
        self
    }

    /// Infallible: metadata has no invariants.
    #[must_use]
    pub fn build(self) -> SequenceMetadata {
        SequenceMetadata {
            description: self.description,
            accession: self.accession,
            organism: self.organism,
            genetic_code: self.genetic_code,
            taxonomy: self.taxonomy,
            extras: self.extras,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_none_with_empty_extras() {
        let meta = SequenceMetadata::default();
        assert_eq!(meta.description(), None);
        assert_eq!(meta.accession(), None);
        assert_eq!(meta.organism(), None);
        assert_eq!(meta.genetic_code(), None);
        assert_eq!(meta.taxonomy(), None);
        assert!(meta.extras().is_empty());
    }

    #[test]
    fn builder_populates_each_field() {
        let meta = SequenceMetadata::builder()
            .description("cloning vector")
            .accession("AF000017.1")
            .organism("Homo sapiens")
            .genetic_code(1)
            .taxonomy(vec!["Eukaryota".to_owned(), "Metazoa".to_owned()])
            .extra("keywords", "beta-galactosidase")
            .build();
        assert_eq!(meta.description(), Some("cloning vector"));
        assert_eq!(meta.accession(), Some("AF000017.1"));
        assert_eq!(meta.organism(), Some("Homo sapiens"));
        assert_eq!(meta.genetic_code(), Some(1));
        assert_eq!(
            meta.taxonomy(),
            Some(&["Eukaryota".to_owned(), "Metazoa".to_owned()][..])
        );
        assert_eq!(
            meta.extras().get("keywords").map(String::as_str),
            Some("beta-galactosidase")
        );
    }

    #[test]
    fn empty_build_equals_default() {
        assert_eq!(
            SequenceMetadata::builder().build(),
            SequenceMetadata::default()
        );
    }

    #[test]
    fn extra_overwrites_existing_key() {
        let meta = SequenceMetadata::builder()
            .extra("keywords", "first")
            .extra("keywords", "second")
            .build();
        assert_eq!(
            meta.extras().get("keywords").map(String::as_str),
            Some("second")
        );
        assert_eq!(meta.extras().len(), 1);
    }

    #[test]
    fn extras_replaces_whole_map() {
        let mut map = BTreeMap::new();
        map.insert("a".to_owned(), "1".to_owned());
        let meta = SequenceMetadata::builder()
            .extra("orphan", "value")
            .extras(map)
            .build();
        assert!(meta.extras().get("orphan").is_none());
        assert_eq!(meta.extras().get("a").map(String::as_str), Some("1"));
    }
}
