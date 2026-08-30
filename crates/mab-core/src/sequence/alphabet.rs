//! Sealed [`Alphabet`] trait and zero-sized marker alphabets.
//!
//! This module defines the three supported IUPAC alphabets used to
//! parameterize [`Sequence`](super::Sequence). The trait is sealed so that
//! no downstream crate can invent a new alphabet, preserving the invariant
//! that a sequence is always valid for one of the known alphabets.

use std::marker::PhantomData;

/// Private sealing module. Prevents external implementations of [`Alphabet`].
mod private {
    /// Sealing trait for [`Alphabet`].
    pub trait Sealed {}
}

/// A sealed alphabet marker used to type-check sequences at compile time.
///
/// Implemented only by the zero-sized marker types [`IupacDna`],
/// [`IupacRna`], and [`IupacAminoAcid`]. Because the trait is sealed, no
/// other types can implement it, ensuring alphabet safety by construction.
pub trait Alphabet: private::Sealed + Send + Sync + 'static {
    /// Human-readable name of the alphabet, used in diagnostics.
    const NAME: &'static str;

    /// Returns `true` if `byte` is a valid uppercase residue for this alphabet.
    ///
    /// Callers should uppercase input bytes before calling this function.
    fn is_valid(byte: u8) -> bool;
}

/// Marker type for IUPAC DNA sequences.
///
/// Valid residues are the 15 uppercase letters `ACGTRYSWKMBDHVN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IupacDna {
    _private: PhantomData<()>,
}

/// Marker type for IUPAC RNA sequences.
///
/// Valid residues are the 15 uppercase letters `ACGURYSWKMBDHVN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IupacRna {
    _private: PhantomData<()>,
}

/// Marker type for IUPAC amino acid sequences.
///
/// Valid residues are the 23 uppercase letters and symbols
/// `ACDEFGHIKLMNPQRSTVWY` plus `XBZ*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IupacAminoAcid {
    _private: PhantomData<()>,
}

impl private::Sealed for IupacDna {}
impl private::Sealed for IupacRna {}
impl private::Sealed for IupacAminoAcid {}

/// Build a 256-entry lookup table from a byte literal of valid residues.
///
/// The resulting table is `true` for each byte value present in `letters`.
// Uses a `while`/`index` loop because iterators and `for` loops are not
// `const`-stable; a `const fn` can only loop this way on stable Rust.
const fn build_table(letters: &[u8]) -> [bool; 256] {
    let mut table = [false; 256];
    let mut i = 0;
    while i < letters.len() {
        table[letters[i] as usize] = true;
        i += 1;
    }
    table
}

static DNA_VALID: [bool; 256] = build_table(b"ACGTRYSWKMBDHVN");
static RNA_VALID: [bool; 256] = build_table(b"ACGURYSWKMBDHVN");
static AMINO_VALID: [bool; 256] = build_table(b"ACDEFGHIKLMNPQRSTVWYXBZ*");

impl Alphabet for IupacDna {
    const NAME: &'static str = "IUPAC DNA";

    fn is_valid(byte: u8) -> bool {
        DNA_VALID[byte as usize]
    }
}

impl Alphabet for IupacRna {
    const NAME: &'static str = "IUPAC RNA";

    fn is_valid(byte: u8) -> bool {
        RNA_VALID[byte as usize]
    }
}

impl Alphabet for IupacAminoAcid {
    const NAME: &'static str = "IUPAC amino acid";

    fn is_valid(byte: u8) -> bool {
        AMINO_VALID[byte as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dna_accepts_full_alphabet() {
        for &byte in b"ACGTRYSWKMBDHVN" {
            assert!(IupacDna::is_valid(byte), "{byte} should be valid IUPAC DNA");
        }
    }

    #[test]
    fn rna_accepts_full_alphabet() {
        for &byte in b"ACGURYSWKMBDHVN" {
            assert!(IupacRna::is_valid(byte), "{byte} should be valid IUPAC RNA");
        }
    }

    #[test]
    fn amino_acid_accepts_full_alphabet() {
        for &byte in b"ACDEFGHIKLMNPQRSTVWYXBZ*" {
            assert!(
                IupacAminoAcid::is_valid(byte),
                "{byte} should be valid IUPAC amino acid"
            );
        }
    }

    #[test]
    fn dna_rejects_rna_specific_thymine_uracil_swap() {
        assert!(!IupacDna::is_valid(b'U'), "U is RNA-specific");
        assert!(IupacDna::is_valid(b'T'), "T is DNA-specific");
    }

    #[test]
    fn rna_rejects_dna_specific_thymine_uracil_swap() {
        assert!(!IupacRna::is_valid(b'T'), "T is DNA-specific");
        assert!(IupacRna::is_valid(b'U'), "U is RNA-specific");
    }

    #[test]
    fn lowercase_is_rejected_at_validation_level() {
        // Tables are uppercase-only; `try_new` uppercases before validating.
        for &byte in b"acgt" {
            assert!(
                !IupacDna::is_valid(byte),
                "{byte} should be invalid lowercase"
            );
        }
    }

    #[test]
    fn digits_and_whitespace_rejected() {
        for &byte in b"0123456789 \t\n\r" {
            assert!(!IupacDna::is_valid(byte), "{byte} should not be DNA");
            assert!(!IupacRna::is_valid(byte), "{byte} should not be RNA");
            assert!(
                !IupacAminoAcid::is_valid(byte),
                "{byte} should not be amino acid"
            );
        }
    }

    #[test]
    fn nul_rejected() {
        assert!(!IupacDna::is_valid(b'\0'));
        assert!(!IupacRna::is_valid(b'\0'));
        assert!(!IupacAminoAcid::is_valid(b'\0'));
    }
}
