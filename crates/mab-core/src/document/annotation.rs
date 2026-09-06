//! Sequence annotations for documents.
//!
//! [`AnnotationInterval`] composes the shared [`Interval`] primitive with
//! optional partial-end flags, and [`SequenceAnnotation`] is a GenBank-style
//! feature (name, kind, intervals, strand, qualifiers). See ADR-0007
//! (`docs/decisions/ADR-0007-sequence-document.md`).

use crate::{Error, Interval, Result, Strand};

/// A 0-based half-open interval `[start, end)` on the sequence, with
/// optional partial-end flags for `GenBank` `<`/`>` markers. Immutable.
///
/// Composes the shared [`Interval`] primitive; the `start < end` invariant
/// is enforced by `Interval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnnotationInterval {
    interval: Interval,
    start_partial: bool,
    end_partial: bool,
}

impl AnnotationInterval {
    /// Construct from raw coordinates. Validates `start < end` via
    /// `Interval::new`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInterval`] if `start >= end`. Zero-length
    /// intervals are invalid in v1 (`GenBank` between-sites `123^124` are
    /// out of scope).
    pub fn new(start: usize, end: usize, start_partial: bool, end_partial: bool) -> Result<Self> {
        let interval = Interval::new(start, end)?;
        Ok(Self {
            interval,
            start_partial,
            end_partial,
        })
    }

    /// Construct from a pre-validated `Interval`. Infallible.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::document::AnnotationInterval;
    /// use mab_core::Interval;
    ///
    /// let interval = Interval::new(10, 20).unwrap();
    /// let ann = AnnotationInterval::from_interval(interval, true, false);
    /// assert_eq!(ann.start(), 10);
    /// assert_eq!(ann.end(), 20);
    /// assert!(ann.start_partial());
    /// assert!(!ann.end_partial());
    /// ```
    #[must_use]
    pub const fn from_interval(interval: Interval, start_partial: bool, end_partial: bool) -> Self {
        Self {
            interval,
            start_partial,
            end_partial,
        }
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.interval.start()
    }
    #[must_use]
    pub const fn end(&self) -> usize {
        self.interval.end()
    }
    #[must_use]
    pub const fn start_partial(&self) -> bool {
        self.start_partial
    }
    #[must_use]
    pub const fn end_partial(&self) -> bool {
        self.end_partial
    }

    /// The underlying `Interval`.
    #[must_use]
    pub const fn interval(&self) -> &Interval {
        &self.interval
    }

    /// `end - start`; always >= 1 by construction.
    ///
    /// Named `span` (not `len`) to avoid `clippy::len_without_is_empty`:
    /// intervals are never empty by construction.
    #[must_use]
    pub const fn span(&self) -> usize {
        self.interval.span()
    }
}

/// A sequence annotation (e.g. a `GenBank` feature). Immutable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SequenceAnnotation {
    name: String,
    kind: String,
    intervals: Vec<AnnotationInterval>,
    strand: Strand,
    qualifiers: Vec<(String, String)>,
}

impl SequenceAnnotation {
    /// Sorts `intervals` ascending by `start` before storing.
    ///
    /// # Errors
    ///
    /// - [`Error::EmptyAnnotationKind`] if `kind` is empty.
    /// - [`Error::EmptyAnnotationIntervals`] if `intervals` is empty.
    /// - [`Error::OverlappingIntervals`] if any two intervals overlap after
    ///   sorting. Adjacent intervals (`next.start == prev.end`) are allowed;
    ///   the constructor never merges.
    /// - [`Error::EmptyQualifierKey`] if any qualifier key is empty
    ///   (empty *values* are allowed: presence qualifiers like `/pseudo`).
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::document::{AnnotationInterval, SequenceAnnotation};
    /// use mab_core::Strand;
    ///
    /// let annotation = SequenceAnnotation::new(
    ///     "lacZ",
    ///     "CDS",
    ///     Strand::Forward,
    ///     vec![AnnotationInterval::new(100, 300, false, false).unwrap()],
    ///     vec![("gene".to_owned(), "lacZ".to_owned())],
    /// )
    /// .unwrap();
    /// assert_eq!(annotation.name(), "lacZ");
    /// assert_eq!(annotation.kind(), "CDS");
    /// assert_eq!(annotation.strand(), Strand::Forward);
    /// assert_eq!(annotation.intervals().len(), 1);
    /// ```
    pub fn new(
        name: impl Into<String>,
        kind: impl Into<String>,
        strand: Strand,
        mut intervals: Vec<AnnotationInterval>,
        qualifiers: Vec<(String, String)>,
    ) -> Result<Self> {
        let kind = kind.into();
        if kind.is_empty() {
            return Err(Error::EmptyAnnotationKind);
        }
        if intervals.is_empty() {
            return Err(Error::EmptyAnnotationIntervals);
        }

        // Sort ascending by start.
        intervals.sort_by_key(AnnotationInterval::start);

        // Check for overlaps: after sorting, next.start < prev.end ⇒ overlap.
        for w in intervals.windows(2) {
            if w[1].start() < w[0].end() {
                return Err(Error::OverlappingIntervals {
                    start: w[0].start(),
                    end: w[0].end(),
                });
            }
        }

        // Validate qualifier keys.
        for (k, _) in &qualifiers {
            if k.is_empty() {
                return Err(Error::EmptyQualifierKey);
            }
        }

        Ok(Self {
            name: name.into(),
            kind,
            intervals,
            strand,
            qualifiers,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }
    #[must_use]
    pub const fn strand(&self) -> Strand {
        self.strand
    }
    #[must_use]
    pub fn intervals(&self) -> &[AnnotationInterval] {
        &self.intervals
    }
    #[must_use]
    pub fn qualifiers(&self) -> &[(String, String)] {
        &self.qualifiers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(start: usize, end: usize) -> AnnotationInterval {
        AnnotationInterval::new(start, end, false, false).unwrap()
    }

    fn simple_annotation() -> Result<SequenceAnnotation> {
        SequenceAnnotation::new(
            "lacZ",
            "CDS",
            Strand::Forward,
            vec![interval(100, 300)],
            vec![],
        )
    }

    #[test]
    fn new_accepts_strictly_increasing_bounds() {
        let iv = AnnotationInterval::new(10, 20, false, false).unwrap();
        assert_eq!(iv.start(), 10);
        assert_eq!(iv.end(), 20);
        assert!(!iv.start_partial());
        assert!(!iv.end_partial());
    }

    #[test]
    fn new_rejects_empty_and_reversed_bounds() {
        assert_eq!(
            AnnotationInterval::new(5, 5, false, false),
            Err(Error::InvalidInterval { start: 5, end: 5 })
        );
        assert_eq!(
            AnnotationInterval::new(7, 3, false, false),
            Err(Error::InvalidInterval { start: 7, end: 3 })
        );
    }

    #[test]
    fn from_interval_is_infallible_and_preserves_flags() {
        let iv = Interval::new(2, 9).unwrap();
        let ann = AnnotationInterval::from_interval(iv, true, true);
        assert_eq!(ann.interval(), &iv);
        assert!(ann.start_partial());
        assert!(ann.end_partial());

        let other = AnnotationInterval::from_interval(iv, false, false);
        assert!(!other.start_partial());
        assert!(!other.end_partial());
    }

    #[test]
    fn interval_returns_underlying_interval() {
        let iv = Interval::new(3, 8).unwrap();
        let ann = AnnotationInterval::from_interval(iv, false, false);
        assert_eq!(ann.interval(), &iv);
    }

    #[test]
    fn partial_flags_stored_independently_of_bounds() {
        let ann = AnnotationInterval::new(0, 1, true, false).unwrap();
        assert!(ann.start_partial());
        assert!(!ann.end_partial());
        let ann = AnnotationInterval::new(0, 1, false, true).unwrap();
        assert!(!ann.start_partial());
        assert!(ann.end_partial());
    }

    #[test]
    fn span_is_end_minus_start() {
        assert_eq!(interval(2, 7).span(), 5);
    }

    #[test]
    fn empty_kind_rejected() {
        let err =
            SequenceAnnotation::new("x", "", Strand::Undirected, vec![interval(0, 1)], vec![])
                .unwrap_err();
        assert_eq!(err, Error::EmptyAnnotationKind);
    }

    #[test]
    fn no_intervals_rejected() {
        let err =
            SequenceAnnotation::new("x", "CDS", Strand::Undirected, vec![], vec![]).unwrap_err();
        assert_eq!(err, Error::EmptyAnnotationIntervals);
    }

    #[test]
    fn overlapping_intervals_rejected() {
        let err = SequenceAnnotation::new(
            "x",
            "CDS",
            Strand::Undirected,
            vec![interval(0, 5), interval(3, 8)],
            vec![],
        )
        .unwrap_err();
        assert_eq!(err, Error::OverlappingIntervals { start: 0, end: 5 });
    }

    #[test]
    fn empty_qualifier_key_rejected() {
        let err = SequenceAnnotation::new(
            "x",
            "CDS",
            Strand::Undirected,
            vec![interval(0, 1)],
            vec![
                ("gene".to_owned(), "lacZ".to_owned()),
                (String::new(), "v".to_owned()),
            ],
        )
        .unwrap_err();
        assert_eq!(err, Error::EmptyQualifierKey);
    }

    #[test]
    fn empty_qualifier_value_allowed() {
        let ann = SequenceAnnotation::new(
            "x",
            "CDS",
            Strand::Undirected,
            vec![interval(0, 1)],
            vec![("pseudo".to_owned(), String::new())],
        )
        .unwrap();
        assert_eq!(ann.qualifiers(), &[("pseudo".to_owned(), String::new())]);
    }

    #[test]
    fn unsorted_input_stored_sorted_ascending() {
        let ann = SequenceAnnotation::new(
            "x",
            "join",
            Strand::Undirected,
            vec![interval(50, 60), interval(0, 10), interval(20, 30)],
            vec![],
        )
        .unwrap();
        let starts: Vec<usize> = ann
            .intervals()
            .iter()
            .map(AnnotationInterval::start)
            .collect();
        assert_eq!(starts, vec![0, 20, 50]);
        assert_eq!(ann.intervals()[0].end(), 10);
        assert_eq!(ann.intervals()[2].end(), 60);
    }

    #[test]
    fn adjacent_intervals_accepted() {
        let ann = SequenceAnnotation::new(
            "x",
            "join",
            Strand::Undirected,
            vec![interval(0, 10), interval(10, 20)],
            vec![],
        )
        .unwrap();
        assert_eq!(ann.intervals().len(), 2);
    }

    #[test]
    fn duplicate_qualifier_keys_and_order_preserved() {
        let ann = SequenceAnnotation::new(
            "x",
            "CDS",
            Strand::Undirected,
            vec![interval(0, 1)],
            vec![
                ("db_xref".to_owned(), "a".to_owned()),
                ("db_xref".to_owned(), "b".to_owned()),
                ("gene".to_owned(), "lacZ".to_owned()),
            ],
        )
        .unwrap();
        assert_eq!(
            ann.qualifiers(),
            &[
                ("db_xref".to_owned(), "a".to_owned()),
                ("db_xref".to_owned(), "b".to_owned()),
                ("gene".to_owned(), "lacZ".to_owned()),
            ]
        );
    }

    #[test]
    fn empty_name_allowed() {
        let ann =
            SequenceAnnotation::new("", "CDS", Strand::Undirected, vec![interval(0, 1)], vec![])
                .unwrap();
        assert_eq!(ann.name(), "");
    }

    #[test]
    fn accessors_return_constructed_values() {
        let ann = simple_annotation().unwrap();
        assert_eq!(ann.name(), "lacZ");
        assert_eq!(ann.kind(), "CDS");
        assert_eq!(ann.strand(), Strand::Forward);
        assert_eq!(ann.intervals(), &[interval(100, 300)]);
    }
}
