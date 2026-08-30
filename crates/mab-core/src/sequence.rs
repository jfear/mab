//! Generic, validated biological sequences.
//!
//! This module provides [`Sequence<A>`], an immutable, owned wrapper around
//! `Vec<u8>` parameterized by a sealed [`Alphabet`] marker. The invariant
//! enforced by the public API is that a `Sequence` is always uppercase and
//! every byte is valid for its alphabet. See ADR-0006.
//!
//! Alphabet-specific operations (reverse complement, DNA/RNA conversion)
//! live in the [`nucleotide`] submodule and are implemented on concrete
//! sequence types such as `Sequence<IupacDna>`.

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::RangeBounds;

use crate::{Error, Result};

pub mod alphabet;
pub mod nucleotide;

pub use alphabet::{Alphabet, IupacAminoAcid, IupacDna, IupacRna};

/// An immutable, validated biological sequence.
///
/// `Sequence<A>` wraps a `Vec<u8>` and is parameterized by a zero-sized
/// [`Alphabet`] marker. The type parameter lets the compiler enforce that
/// DNA, RNA, and amino acid sequences are not mixed accidentally.
///
/// # Invariants
///
/// - All residues are ASCII uppercase.
/// - Every byte is valid according to the alphabet `A`.
/// - There is no public mutation path that bypasses validation.
///
/// Sequences may be empty. Empty input is considered valid for every
/// alphabet.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Sequence<A: Alphabet> {
    residues: Vec<u8>,
    _alphabet: PhantomData<A>,
}

impl<A: Alphabet> Sequence<A> {
    /// Create a new sequence from a byte source, uppercasing and validating
    /// every residue.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResidue`] if any byte is not valid for the
    /// alphabet after uppercasing. The error reports the first invalid byte,
    /// its zero-based position, and the alphabet name.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::sequence::{IupacDna, Sequence};
    ///
    /// let seq = Sequence::<IupacDna>::try_new("acgt").unwrap();
    /// assert_eq!(seq.as_str(), "ACGT");
    /// ```
    pub fn try_new(residues: impl Into<Vec<u8>>) -> Result<Self> {
        let mut residues = residues.into();
        residues.make_ascii_uppercase();
        for (position, &residue) in residues.iter().enumerate() {
            if !A::is_valid(residue) {
                return Err(Error::InvalidResidue {
                    residue,
                    position,
                    alphabet: A::NAME,
                });
            }
        }
        Ok(Self {
            residues,
            _alphabet: PhantomData,
        })
    }

    /// Create a sequence from bytes that are already known to be uppercase
    /// and valid for the alphabet.
    ///
    /// # Safety
    ///
    /// Callers must guarantee that every byte in `residues` is valid for the
    /// alphabet `A`. Violating this invariant may break downstream code that
    /// relies on sequences being valid (for example, nucleotide tables).
    pub(crate) fn from_validated_unchecked(residues: Vec<u8>) -> Self {
        debug_assert!(
            residues.iter().all(|&b| A::is_valid(b)),
            "from_validated_unchecked called with invalid residues"
        );
        Self {
            residues,
            _alphabet: PhantomData,
        }
    }

    /// Returns the number of residues in the sequence.
    #[must_use]
    pub fn len(&self) -> usize {
        self.residues.len()
    }

    /// Returns `true` if the sequence contains no residues.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.residues.is_empty()
    }

    /// Returns the residues as a byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.residues
    }

    /// Returns the residues as an ASCII string slice.
    ///
    /// This conversion is infallible because all valid residues are ASCII.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // SAFETY: every valid residue byte is ASCII, so the buffer is valid UTF-8.
        unsafe { std::str::from_utf8_unchecked(&self.residues) }
    }

    /// Returns an iterator over the residues as bytes.
    pub fn iter(&self) -> std::iter::Copied<std::slice::Iter<'_, u8>> {
        self.residues.iter().copied()
    }

    /// Returns a slice of the sequence within the given bounds, or `None` if
    /// the range is out of bounds.
    ///
    /// Unlike Rust slicing, this method never panics. It returns a byte
    /// slice rather than a `Sequence` view; zero-copy sequence views are
    /// deferred to future work.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::sequence::{IupacDna, Sequence};
    ///
    /// let seq = Sequence::<IupacDna>::try_new("ACGTRYSWKMBDHVN").unwrap();
    /// assert_eq!(seq.slice(0..4), Some(b"ACGT".as_slice()));
    /// assert_eq!(seq.slice(4..=7), Some(b"RYSW".as_slice()));
    /// assert!(seq.slice(10..30).is_none());
    /// ```
    #[must_use]
    pub fn slice<R>(&self, range: R) -> Option<&[u8]>
    where
        R: RangeBounds<usize>,
    {
        use std::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1)?,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1)?,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => self.len(),
        };
        self.residues.get(start..end)
    }
}

impl<A: Alphabet> AsRef<[u8]> for Sequence<A> {
    fn as_ref(&self) -> &[u8] {
        &self.residues
    }
}

impl<'a, A: Alphabet> IntoIterator for &'a Sequence<A> {
    type Item = u8;
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, u8>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<A: Alphabet> fmt::Debug for Sequence<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const TRUNCATE_AT: usize = 30;

        let s = self.as_str();
        if s.len() > TRUNCATE_AT {
            write!(
                f,
                "Sequence<{}>(\"{}…\" (len {}))",
                A::NAME,
                &s[..TRUNCATE_AT],
                s.len()
            )
        } else {
            write!(f, "Sequence<{}>(\"{}\")", A::NAME, s)
        }
    }
}

impl<A: Alphabet> fmt::Display for Sequence<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_uppercases_in_place() {
        let seq = Sequence::<IupacDna>::try_new("acgt").unwrap();
        assert_eq!(seq.as_str(), "ACGT");
        assert_eq!(seq.as_bytes(), b"ACGT");
    }

    #[test]
    fn empty_sequence_is_valid() {
        let dna = Sequence::<IupacDna>::try_new("").unwrap();
        let rna = Sequence::<IupacRna>::try_new(Vec::new()).unwrap();
        let aa = Sequence::<IupacAminoAcid>::try_new([] as [u8; 0]).unwrap();
        assert!(dna.is_empty());
        assert!(rna.is_empty());
        assert!(aa.is_empty());
    }

    #[test]
    fn try_new_rejects_invalid_residue_and_reports_position() {
        let err = Sequence::<IupacDna>::try_new("ACGT!AT").unwrap_err();
        assert_eq!(
            err,
            Error::InvalidResidue {
                residue: b'!',
                position: 4,
                alphabet: IupacDna::NAME,
            }
        );
    }

    #[test]
    fn dna_rejects_rna_specific_residue() {
        // U is valid for RNA, but not for DNA.
        assert!(Sequence::<IupacDna>::try_new("ACGU").is_err());
    }

    #[test]
    fn accessors_report_expected_values() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        assert_eq!(seq.len(), 4);
        assert!(!seq.is_empty());
        assert_eq!(seq.as_bytes(), b"ACGT");
        assert_eq!(seq.as_str(), "ACGT");
        assert_eq!(seq.iter().collect::<Vec<_>>(), vec![b'A', b'C', b'G', b'T']);
    }

    #[test]
    fn slicing_edges() {
        let seq = Sequence::<IupacDna>::try_new("ACGTRYSWKMBDHVN").unwrap();
        assert_eq!(seq.slice(..), Some(b"ACGTRYSWKMBDHVN".as_slice()));
        assert_eq!(seq.slice(0..4), Some(b"ACGT".as_slice()));
        assert_eq!(seq.slice(4..=7), Some(b"RYSW".as_slice()));
        assert_eq!(seq.slice(15..15), Some(b"".as_slice()));
        assert!(seq.slice(10..30).is_none());
        assert!(seq.slice(..16).is_none());
    }

    #[test]
    fn partial_eq_across_same_alphabet() {
        let a = Sequence::<IupacDna>::try_new("acgt").unwrap();
        let b = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn as_ref_u8_slice() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        let slice: &[u8] = seq.as_ref();
        assert_eq!(slice, b"ACGT");
    }

    #[test]
    fn display_is_residue_string() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        assert_eq!(seq.to_string(), "ACGT");
    }

    #[test]
    fn debug_truncates_long_sequences() {
        let seq = Sequence::<IupacDna>::try_new("ACGT".repeat(20)).unwrap();
        let dbg = format!("{seq:?}");
        assert!(dbg.starts_with("Sequence<IUPAC DNA>(\""));
        assert!(dbg.contains("…"));
        assert!(dbg.ends_with(')'));
    }

    #[test]
    fn debug_short_sequences_not_truncated() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        assert_eq!(format!("{seq:?}"), "Sequence<IUPAC DNA>(\"ACGT\")");
    }
}
