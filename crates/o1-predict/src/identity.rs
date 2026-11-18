//! Operation identity: what an operation is ([`OperationId`]) and which code was measured
//! ([`Implementation`]).

use alloc::string::String;

/// The semantic name of an operation: the package and the path of the function or method, no
/// generics, no line numbers. Compared for equality, never parsed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OperationId {
    /// Package (crate) name, e.g. `o1-core`.
    pub package: String,
    /// Path of the function or method inside the package, e.g. `stream::BlockPass::transform`.
    pub path: String,
}

impl OperationId {
    /// An operation id.
    pub fn new(package: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            package: package.into(),
            path: path.into(),
        }
    }
}

/// What code was measured: the package version and the digest of the measured source scope
/// (`sha256:` over the scope, Unit 3/4's rule).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Implementation {
    /// Package version.
    pub version: String,
    /// Digest of the measured source scope.
    pub source_digest: String,
}

impl Implementation {
    /// An implementation.
    pub fn new(version: impl Into<String>, source_digest: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            source_digest: source_digest.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_equality_of_both_parts() {
        let a = OperationId::new("o1-core", "stream::BlockPass::transform");
        let b = OperationId::new("o1-core", "stream::BlockPass::transform");
        assert_eq!(a, b);
        assert_ne!(
            a,
            OperationId::new("o1-sync", "stream::BlockPass::transform")
        );
        assert_ne!(a, OperationId::new("o1-core", "stream::BlockPass::read"));
        // Never parsed: a path that looks like a prefix is another operation.
        assert_ne!(a, OperationId::new("o1-core", "stream::BlockPass"));
    }

    #[test]
    fn identity_is_stable_across_clones_and_order() {
        let a = OperationId::new("p", "x");
        let c = a.clone();
        assert_eq!(a, c);
        assert!(OperationId::new("a", "z") < OperationId::new("b", "a"));
        let i = Implementation::new("0.1.0", "sha256:00");
        assert_eq!(i, i.clone());
        assert_ne!(i, Implementation::new("0.1.0", "sha256:01"));
        assert_ne!(i, Implementation::new("0.1.1", "sha256:00"));
    }
}
