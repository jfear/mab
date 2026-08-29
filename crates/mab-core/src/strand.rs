//! Biological strand (direction) of a sequence, alignment, or annotation.

use std::fmt;
use std::str::FromStr;

use crate::Error;

/// Biological strand (direction) of a sequence, alignment, or annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Strand {
    /// Forward strand (5'→3' on the reference).
    Forward,

    /// Reverse strand (3'→5' on the reference; reverse complement).
    Reverse,

    /// No strand information.
    #[default]
    Undirected,
}

impl Strand {
    /// Extract strand from a SAM/BAM flag.
    ///
    /// SAM flag bit 0x10 (16) indicates the sequence is reverse complemented.
    #[must_use]
    pub const fn from_sam_flag(flag: u16) -> Self {
        if flag & 0x10 != 0 {
            Self::Reverse
        } else {
            Self::Forward
        }
    }

    /// Returns `true` if this is the forward strand.
    #[must_use]
    pub const fn is_forward(self) -> bool {
        matches!(self, Self::Forward)
    }

    /// Returns `true` if this is the reverse strand.
    #[must_use]
    pub const fn is_reverse(self) -> bool {
        matches!(self, Self::Reverse)
    }

    /// Returns `true` if strand information is absent.
    #[must_use]
    pub const fn is_undirected(self) -> bool {
        matches!(self, Self::Undirected)
    }

    /// Returns the complementary strand.
    ///
    /// `Forward` becomes `Reverse`, `Reverse` becomes `Forward`,
    /// and `Undirected` remains unchanged.
    #[must_use]
    pub const fn complement(self) -> Self {
        match self {
            Self::Forward => Self::Reverse,
            Self::Reverse => Self::Forward,
            Self::Undirected => Self::Undirected,
        }
    }
}

impl fmt::Display for Strand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forward => f.write_str("+"),
            Self::Reverse => f.write_str("-"),
            Self::Undirected => f.write_str("."),
        }
    }
}

impl FromStr for Strand {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+" => Ok(Self::Forward),
            "-" => Ok(Self::Reverse),
            "." => Ok(Self::Undirected),
            _ => Err(Error::StrandParse(s.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats() {
        assert_eq!(Strand::Forward.to_string(), "+");
        assert_eq!(Strand::Reverse.to_string(), "-");
        assert_eq!(Strand::Undirected.to_string(), ".");
    }

    #[test]
    fn from_str_symbolic() {
        assert_eq!("+".parse::<Strand>(), Ok(Strand::Forward));
        assert_eq!("-".parse::<Strand>(), Ok(Strand::Reverse));
        assert_eq!(".".parse::<Strand>(), Ok(Strand::Undirected));
    }

    #[test]
    fn from_str_invalid() {
        assert!("?".parse::<Strand>().is_err());
        assert!("".parse::<Strand>().is_err());
        assert!("forward".parse::<Strand>().is_err());
    }

    #[test]
    fn from_sam_flag() {
        assert_eq!(Strand::from_sam_flag(0), Strand::Forward);
        assert_eq!(Strand::from_sam_flag(16), Strand::Reverse);
        // 17 = 0x11 = paired (0x1) + reversed (0x10)
        assert_eq!(Strand::from_sam_flag(17), Strand::Reverse);
        // 99 = 0x63 = paired + proper + mate reversed + first in pair (no 0x10)
        assert_eq!(Strand::from_sam_flag(99), Strand::Forward);
    }

    #[test]
    fn complement() {
        assert_eq!(Strand::Forward.complement(), Strand::Reverse);
        assert_eq!(Strand::Reverse.complement(), Strand::Forward);
        assert_eq!(Strand::Undirected.complement(), Strand::Undirected);
    }

    #[test]
    fn complement_roundtrip() {
        // Complementing twice returns the original.
        for s in [Strand::Forward, Strand::Reverse, Strand::Undirected] {
            assert_eq!(s.complement().complement(), s);
        }
    }

    #[test]
    fn predicates() {
        assert!(Strand::Forward.is_forward());
        assert!(!Strand::Forward.is_reverse());
        assert!(!Strand::Forward.is_undirected());

        assert!(!Strand::Reverse.is_forward());
        assert!(Strand::Reverse.is_reverse());
        assert!(!Strand::Reverse.is_undirected());

        assert!(!Strand::Undirected.is_forward());
        assert!(!Strand::Undirected.is_reverse());
        assert!(Strand::Undirected.is_undirected());
    }
}
