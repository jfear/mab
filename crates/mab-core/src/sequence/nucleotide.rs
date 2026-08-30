//! Nucleotide-specific operations for [`Sequence`]s.
//!
//! Reverse complement is implemented for DNA only. RNA sequences may be
//! converted to DNA and vice versa; complement operations for RNA can be
//! added later if needed. See ADR-0006.

use super::{IupacDna, IupacRna, Sequence};

/// IUPAC DNA/RNA complement table over the full byte range.
///
/// The table defaults to identity for every byte and overrides the 15 IUPAC
/// ambiguity codes:
///
/// | Base | Complement |
/// |------|------------|
/// | A    | T          |
/// | C    | G          |
/// | G    | C          |
/// | T    | A          |
/// | R    | Y          |
/// | Y    | R          |
/// | S    | S          |
/// | W    | W          |
/// | K    | M          |
/// | M    | K          |
/// | B    | V          |
/// | D    | H          |
/// | H    | D          |
/// | V    | B          |
/// | N    | N          |
///
/// The 15-letter IUPAC alphabet is closed under this table: complementing
/// every base in any ambiguity set always yields another valid IUPAC code.
static COMPLEMENT: [u8; 256] = build_complement_table();

const COMPLEMENT_PAIRS: &[(u8, u8)] = &[
    (b'A', b'T'),
    (b'C', b'G'),
    (b'R', b'Y'),
    (b'S', b'S'),
    (b'W', b'W'),
    (b'K', b'M'),
    (b'B', b'V'),
    (b'D', b'H'),
    (b'N', b'N'),
];

const fn build_complement_table() -> [u8; 256] {
    // `for` loops and iterators are not `const`-stable,
    // so `while` with explicit indices is the only option here.
    let mut table = [0_u8; 256];
    let mut i = 0;
    while i < 256 {
        // `i` is always in the range 0..256, so it fits in `u8`.
        #[allow(clippy::cast_possible_truncation)]
        {
            table[i] = i as u8;
        }
        i += 1;
    }

    let mut j = 0;
    while j < COMPLEMENT_PAIRS.len() {
        let (a, b) = COMPLEMENT_PAIRS[j];
        table[a as usize] = b;
        table[b as usize] = a;
        j += 1;
    }

    table
}

impl Sequence<IupacDna> {
    /// Returns the complement of this DNA sequence.
    ///
    /// Every residue is mapped through the IUPAC complement table. All 15
    /// IUPAC DNA codes are supported, including ambiguity codes.
    #[must_use]
    pub fn complement(&self) -> Self {
        let residues: Vec<u8> = self.iter().map(|b| COMPLEMENT[b as usize]).collect();
        Self::from_validated_unchecked(residues)
    }

    /// Returns the reverse complement of this DNA sequence.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::sequence::{IupacDna, Sequence};
    ///
    /// let seq = Sequence::<IupacDna>::try_new("ACGTRYAT").unwrap();
    /// assert_eq!(seq.reverse_complement().as_str(), "ATRYACGT");
    /// ```
    #[must_use]
    pub fn reverse_complement(&self) -> Self {
        let residues: Vec<u8> = self.iter().rev().map(|b| COMPLEMENT[b as usize]).collect();
        Self::from_validated_unchecked(residues)
    }

    /// Convert this DNA sequence to an RNA sequence by replacing thymine
    /// (`T`) with uracil (`U`).
    #[must_use]
    pub fn to_rna(&self) -> Sequence<IupacRna> {
        let residues: Vec<u8> = self
            .iter()
            .map(|b| if b == b'T' { b'U' } else { b })
            .collect();
        Sequence::from_validated_unchecked(residues)
    }
}

impl Sequence<IupacRna> {
    /// Convert this RNA sequence to a DNA sequence by replacing uracil (`U`)
    /// with thymine (`T`).
    #[must_use]
    pub fn to_dna(&self) -> Sequence<IupacDna> {
        let residues: Vec<u8> = self
            .iter()
            .map(|b| if b == b'U' { b'T' } else { b })
            .collect();
        Sequence::from_validated_unchecked(residues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::Alphabet;

    #[test]
    fn complement_table_over_all_iupac_codes() {
        let dna = b"ACGTRYSWKMBDHVN";
        let expected = b"TGCAYRSWMKVHDBN";
        for (&input, &exp) in dna.iter().zip(expected.iter()) {
            assert_eq!(
                COMPLEMENT[input as usize], exp,
                "complement of {input} should be {exp}"
            );
        }
    }

    #[test]
    fn complement_table_is_closed_over_iupac_dna() {
        // Every IUPAC DNA code complements to another valid IUPAC DNA code.
        for &byte in b"ACGTRYSWKMBDHVN" {
            let comp = COMPLEMENT[byte as usize];
            assert!(
                IupacDna::is_valid(comp),
                "complement of {byte} -> {comp} is not valid DNA"
            );
        }
    }

    #[test]
    fn reverse_complement_worked_example() {
        let seq = Sequence::<IupacDna>::try_new("ACGTRYAT").unwrap();
        assert_eq!(seq.reverse_complement().as_str(), "ATRYACGT");
    }

    #[test]
    fn reverse_complement_is_involution() {
        let cases = ["ACGT", "ACGTRYSWKMBDHVN", "AAA", "RYSW", ""];
        for case in cases {
            let seq = Sequence::<IupacDna>::try_new(case).unwrap();
            assert_eq!(seq.reverse_complement().reverse_complement(), seq);
        }
    }

    #[test]
    fn complement_is_involution() {
        let seq = Sequence::<IupacDna>::try_new("ACGTRYSWKMBDHVN").unwrap();
        assert_eq!(seq.complement().complement(), seq);
    }

    #[test]
    fn dna_to_rna_roundtrip() {
        let dna = Sequence::<IupacDna>::try_new("ACGTRYSWKMBDHVN").unwrap();
        let rna = dna.to_rna();
        assert_eq!(rna.as_str(), "ACGURYSWKMBDHVN");
        assert_eq!(rna.to_dna(), dna);
    }

    #[test]
    fn rna_to_dna_roundtrip() {
        let rna = Sequence::<IupacRna>::try_new("ACGURYSWKMBDHVN").unwrap();
        let dna = rna.to_dna();
        assert_eq!(dna.as_str(), "ACGTRYSWKMBDHVN");
        assert_eq!(dna.to_rna(), rna);
    }

    #[test]
    fn lowercase_input_uppercased_before_nucleotide_ops() {
        let seq = Sequence::<IupacDna>::try_new("acgt").unwrap();
        assert_eq!(seq.complement().as_str(), "TGCA");
        assert_eq!(seq.reverse_complement().as_str(), "ACGT");
    }
}
