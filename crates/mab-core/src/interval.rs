//! A validated 0-based half-open coordinate interval.
//!
//! [`Interval`] is the shared coordinate primitive for annotations,
//! alignments, view windows, and any other subsystem that needs a validated
//! coordinate range. Validation (`start < end`) happens exactly once, at
//! construction; types that compose it inherit that guarantee. See
//! ADR-0007 (`docs/decisions/ADR-0007-sequence-document.md`).

use crate::{Error, Result};

/// A 0-based half-open interval `[start, end)`. Immutable.
///
/// Enforces `start < end` at construction. Reused by annotations,
/// alignments, view windows, and any other subsystem that needs a
/// validated coordinate range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Interval {
    start: usize,
    end: usize,
}

impl Interval {
    /// Create a validated `[start, end)` interval.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::Interval;
    ///
    /// let iv = Interval::new(10, 20).unwrap();
    /// assert_eq!(iv.start(), 10);
    /// assert_eq!(iv.end(), 20);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInterval`] if `start >= end`.
    pub const fn new(start: usize, end: usize) -> Result<Self> {
        if start >= end {
            return Err(Error::InvalidInterval { start, end });
        }
        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// `end - start`; always >= 1 by construction.
    #[must_use]
    pub const fn span(&self) -> usize {
        self.end - self.start
    }

    /// Two intervals share at least one position.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::Interval;
    ///
    /// let a = Interval::new(0, 5).unwrap();
    /// let b = Interval::new(3, 8).unwrap();
    /// assert!(a.overlaps(&b));
    /// ```
    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// `self.end == other.start || other.end == self.start`.
    #[must_use]
    pub const fn abuts(&self, other: &Self) -> bool {
        self.end == other.start || other.end == self.start
    }

    /// Intersection; `None` if disjoint.
    #[must_use]
    pub const fn intersection(&self, other: &Self) -> Option<Self> {
        // `Ord::max`/`Ord::min` are not const-stable, so use explicit `if`.
        let start = if self.start > other.start {
            self.start
        } else {
            other.start
        };
        let end = if self.end < other.end {
            self.end
        } else {
            other.end
        };
        if start < end {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Convex hull (smallest interval containing both). Always succeeds.
    #[must_use]
    pub const fn union(&self, other: &Self) -> Self {
        Self {
            start: if self.start < other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end > other.end {
                self.end
            } else {
                other.end
            },
        }
    }

    #[must_use]
    pub const fn contains_point(&self, point: usize) -> bool {
        self.start <= point && point < self.end
    }

    #[must_use]
    pub const fn contains_interval(&self, other: &Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

impl From<Interval> for std::ops::Range<usize> {
    fn from(iv: Interval) -> Self {
        iv.start..iv.end
    }
}

impl TryFrom<std::ops::Range<usize>> for Interval {
    type Error = Error;

    fn try_from(range: std::ops::Range<usize>) -> Result<Self> {
        Self::new(range.start, range.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_strictly_increasing_bounds() {
        let iv = Interval::new(0, 5).unwrap();
        assert_eq!(iv.start(), 0);
        assert_eq!(iv.end(), 5);
    }

    #[test]
    fn new_rejects_empty_and_reversed_bounds() {
        assert_eq!(
            Interval::new(5, 5),
            Err(Error::InvalidInterval { start: 5, end: 5 })
        );
        assert_eq!(
            Interval::new(7, 3),
            Err(Error::InvalidInterval { start: 7, end: 3 })
        );
    }

    #[test]
    fn span_is_end_minus_start() {
        assert_eq!(Interval::new(2, 7).unwrap().span(), 5);
    }

    #[test]
    fn overlaps_detects_shared_positions() {
        let a = Interval::new(0, 5).unwrap();
        let b = Interval::new(3, 8).unwrap();
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn overlaps_rejects_touching_intervals() {
        let a = Interval::new(0, 5).unwrap();
        let b = Interval::new(5, 10).unwrap();
        assert!(!a.overlaps(&b));
        assert!(!b.overlaps(&a));
    }

    #[test]
    fn abuts_detects_touching_intervals() {
        let a = Interval::new(0, 5).unwrap();
        let b = Interval::new(5, 10).unwrap();
        assert!(a.abuts(&b));
        assert!(b.abuts(&a));
    }

    #[test]
    fn abuts_rejects_gapped_intervals() {
        let a = Interval::new(0, 5).unwrap();
        let b = Interval::new(6, 10).unwrap();
        assert!(!a.abuts(&b));
        assert!(!b.abuts(&a));
    }

    #[test]
    fn intersection_returns_overlap() {
        let a = Interval::new(0, 8).unwrap();
        let b = Interval::new(5, 12).unwrap();
        assert_eq!(a.intersection(&b), Some(Interval::new(5, 8).unwrap()));
    }

    #[test]
    fn intersection_is_none_for_touching_intervals() {
        let a = Interval::new(0, 5).unwrap();
        let b = Interval::new(5, 10).unwrap();
        assert_eq!(a.intersection(&b), None);
    }

    #[test]
    fn union_returns_convex_hull() {
        let a = Interval::new(0, 5).unwrap();
        let b = Interval::new(8, 12).unwrap();
        assert_eq!(a.union(&b), Interval::new(0, 12).unwrap());

        let c = Interval::new(3, 8).unwrap();
        assert_eq!(a.union(&c), Interval::new(0, 8).unwrap());
    }

    #[test]
    fn contains_point_is_half_open() {
        let iv = Interval::new(0, 5).unwrap();
        assert!(iv.contains_point(0));
        assert!(iv.contains_point(4));
        assert!(!iv.contains_point(5));
    }

    #[test]
    fn contains_interval_requires_full_containment() {
        let outer = Interval::new(0, 10).unwrap();
        let inner = Interval::new(3, 7).unwrap();
        assert!(outer.contains_interval(&inner));
        assert!(!inner.contains_interval(&outer));

        let sticking_out = Interval::new(3, 8).unwrap();
        assert!(
            !Interval::new(0, 5)
                .unwrap()
                .contains_interval(&sticking_out)
        );
    }

    #[test]
    fn conversion_to_range_is_lossless() {
        let iv = Interval::new(2, 9).unwrap();
        let range = std::ops::Range::from(iv);
        assert_eq!(range, 2..9);
    }

    #[test]
    fn try_from_range_accepts_valid_range() {
        let iv = Interval::try_from(2..9).unwrap();
        assert_eq!(iv, Interval::new(2, 9).unwrap());
    }

    #[test]
    fn try_from_range_rejects_empty_range() {
        assert_eq!(
            Interval::try_from(5..5),
            Err(Error::InvalidInterval { start: 5, end: 5 })
        );
    }
}
