//! Sequence topology for documents.
//!
//! See ADR-0007 (`docs/decisions/ADR-0007-sequence-document.md`).

/// Sequence topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Topology {
    /// Linear topology (default).
    #[default]
    Linear,
    /// Circular topology (e.g. plasmids, mitochondrial DNA).
    Circular,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_linear() {
        assert_eq!(Topology::default(), Topology::Linear);
    }

    #[test]
    fn topology_is_copy() {
        let t = Topology::Circular;
        let t2 = t;
        assert_eq!(t, t2);
    }
}
