//! Error types for `mab-core`.
//!
//! Every fallible operation in this crate returns [`crate::Result`], whose
//! error type is the crate-local [`Error`] defined here. External errors are
//! converted at the boundary via [`From`] (`#[from]`) or wrapped with
//! context-bearing variants (`#[source]`). See ADR-0005.

/// The error type for all fallible operations in `mab-core`.
///
/// This enum starts empty and grows variants as fallible APIs are added.
/// It is non-exhaustive, so new variants can be added without a
/// semver-breaking release.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }
}
