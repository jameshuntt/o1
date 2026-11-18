//! The class key (design §4): everything that selects different machine code or a different
//! calibration family, as a sorted list of typed parts with at most one part per kind. An
//! [`OperationClass`] is an [`OperationId`] plus a key; selection is exact equality of the
//! whole class.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::identity::OperationId;

/// Access pattern (mirrors [`o1_contract::Access`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Access {
    /// Addresses in increasing order.
    Sequential,
    /// Addresses chosen by data.
    Random,
}

impl From<o1_contract::Access> for Access {
    fn from(a: o1_contract::Access) -> Self {
        match a {
            o1_contract::Access::Sequential => Self::Sequential,
            o1_contract::Access::Random => Self::Random,
        }
    }
}

/// Reuse (mirrors [`o1_contract::Reuse`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Reuse {
    /// Touched once (streaming).
    Once,
    /// Touched again (a reused buffer).
    Reused,
}

impl From<o1_contract::Reuse> for Reuse {
    fn from(r: o1_contract::Reuse) -> Self {
        match r {
            o1_contract::Reuse::Once => Self::Once,
            o1_contract::Reuse::Reused => Self::Reused,
        }
    }
}

/// Store kind (mirrors [`o1_contract::WritePolicy`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Write {
    /// Ordinary stores.
    Ordinary,
    /// Non-temporal stores (no backend exists; always refused when asked for as a leaf).
    NonTemporal,
}

impl From<o1_contract::WritePolicy> for Write {
    fn from(w: o1_contract::WritePolicy) -> Self {
        match w {
            o1_contract::WritePolicy::Ordinary => Self::Ordinary,
            o1_contract::WritePolicy::NonTemporal => Self::NonTemporal,
        }
    }
}

/// How the data is shared with other threads while the operation runs. The first three mirror
/// [`o1_contract::Sharing`]; the last two are shapes the ecosystem survey found that the
/// contract vocabulary does not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Sharing {
    /// No other thread accesses it.
    Exclusive,
    /// Others may read; nobody writes.
    SharedRead,
    /// Two sides, each location written by one and read by the other.
    Handoff,
    /// One writer with live readers on other threads.
    WriterWithReaders,
    /// Several writers of one location.
    ManyWriters,
}

impl From<o1_contract::Sharing> for Sharing {
    fn from(s: o1_contract::Sharing) -> Self {
        match s {
            o1_contract::Sharing::Exclusive => Self::Exclusive,
            o1_contract::Sharing::SharedRead => Self::SharedRead,
            o1_contract::Sharing::Handoff => Self::Handoff,
        }
    }
}

/// An atomic operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum AtomicOp {
    /// `load`.
    Load,
    /// `store`.
    Store,
    /// A read-modify-write such as `fetch_add`.
    Rmw,
    /// A compare-exchange (loop); never an RMW.
    CompareExchange,
}

/// A memory ordering (for a compare-exchange: the success ordering; the failure ordering is a
/// [`KeyPart::Mode`] when it matters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Ordering {
    /// `Relaxed`.
    Relaxed,
    /// `Acquire`.
    Acquire,
    /// `Release`.
    Release,
    /// `AcqRel`.
    AcqRel,
    /// `SeqCst`.
    SeqCst,
}

/// Placement of written control words relative to another thread's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Isolation {
    /// Each in its own padded unit.
    Padded,
    /// Adjacent words in one unit (false sharing).
    Adjacent,
    /// One word written by all (true sharing).
    SameWord,
}

/// Visiting order over blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Order {
    /// Increasing.
    Sequential,
    /// A pseudo-random permutation.
    Random,
}

/// One typed part of a class key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum KeyPart {
    /// Block size of a block pass (a const generic).
    BlockBytes(u32),
    /// Element size.
    ElementBytes(u32),
    /// Access pattern.
    Access(Access),
    /// Reuse.
    Reuse(Reuse),
    /// Store kind.
    Write(Write),
    /// Sharing shape.
    Sharing(Sharing),
    /// Atomic operation and its ordering.
    Atomic {
        /// The operation.
        op: AtomicOp,
        /// Its ordering.
        ordering: Ordering,
    },
    /// Placement of written control words.
    Isolation(Isolation),
    /// Threads that run the class as a whole.
    Threads(u32),
    /// A mode that selects a code path (text, compared for equality).
    Mode(String),
    /// The identity of a callback the operation calls.
    Callback(String),
    /// The `o1-contract` constant the class composes.
    Contract(String),
    /// Visiting order.
    Order(Order),
}

/// The kind of a [`KeyPart`] (what a refusal names when a part differs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PartKind {
    /// [`KeyPart::BlockBytes`].
    BlockBytes,
    /// [`KeyPart::ElementBytes`].
    ElementBytes,
    /// [`KeyPart::Access`].
    Access,
    /// [`KeyPart::Reuse`].
    Reuse,
    /// [`KeyPart::Write`].
    Write,
    /// [`KeyPart::Sharing`].
    Sharing,
    /// [`KeyPart::Atomic`].
    Atomic,
    /// [`KeyPart::Isolation`].
    Isolation,
    /// [`KeyPart::Threads`].
    Threads,
    /// [`KeyPart::Mode`].
    Mode,
    /// [`KeyPart::Callback`].
    Callback,
    /// [`KeyPart::Contract`].
    Contract,
    /// [`KeyPart::Order`].
    Order,
}

impl PartKind {
    /// `snake_case` name.
    pub fn name(self) -> &'static str {
        match self {
            Self::BlockBytes => "block_bytes",
            Self::ElementBytes => "element_bytes",
            Self::Access => "access",
            Self::Reuse => "reuse",
            Self::Write => "write",
            Self::Sharing => "sharing",
            Self::Atomic => "atomic",
            Self::Isolation => "isolation",
            Self::Threads => "threads",
            Self::Mode => "mode",
            Self::Callback => "callback",
            Self::Contract => "contract",
            Self::Order => "order",
        }
    }
}

impl KeyPart {
    /// The part's kind.
    pub fn kind(&self) -> PartKind {
        match self {
            Self::BlockBytes(_) => PartKind::BlockBytes,
            Self::ElementBytes(_) => PartKind::ElementBytes,
            Self::Access(_) => PartKind::Access,
            Self::Reuse(_) => PartKind::Reuse,
            Self::Write(_) => PartKind::Write,
            Self::Sharing(_) => PartKind::Sharing,
            Self::Atomic { .. } => PartKind::Atomic,
            Self::Isolation(_) => PartKind::Isolation,
            Self::Threads(_) => PartKind::Threads,
            Self::Mode(_) => PartKind::Mode,
            Self::Callback(_) => PartKind::Callback,
            Self::Contract(_) => PartKind::Contract,
            Self::Order(_) => PartKind::Order,
        }
    }
}

/// Why a key cannot be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    /// Two parts of one kind (a class has one access pattern, one ordering, …).
    DuplicatePart(PartKind),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePart(k) => write!(f, "two key parts of kind {}", k.name()),
        }
    }
}

/// A class key: parts sorted by kind, at most one per kind. Two keys are equal exactly when
/// they hold the same parts, whatever order they were given in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "Vec<KeyPart>", into = "Vec<KeyPart>")
)]
pub struct ClassKey(Vec<KeyPart>);

impl ClassKey {
    /// A key from `parts` (any order).
    ///
    /// # Errors
    ///
    /// [`KeyError::DuplicatePart`] when two parts have one kind.
    pub fn new(mut parts: Vec<KeyPart>) -> Result<Self, KeyError> {
        parts.sort();
        for w in parts.windows(2) {
            if w[0].kind() == w[1].kind() {
                return Err(KeyError::DuplicatePart(w[0].kind()));
            }
        }
        Ok(Self(parts))
    }

    /// The parts, sorted by kind.
    pub fn parts(&self) -> &[KeyPart] {
        &self.0
    }

    /// The part of `kind`, if any.
    pub fn get(&self, kind: PartKind) -> Option<&KeyPart> {
        self.0.iter().find(|p| p.kind() == kind)
    }

    /// The kinds whose parts differ between the two keys (present in one only, or different),
    /// ascending.
    pub fn differing(&self, other: &ClassKey) -> Vec<PartKind> {
        let mut kinds: Vec<PartKind> = self
            .0
            .iter()
            .chain(other.0.iter())
            .map(KeyPart::kind)
            .collect();
        kinds.sort();
        kinds.dedup();
        kinds.retain(|k| self.get(*k) != other.get(*k));
        kinds
    }
}

impl TryFrom<Vec<KeyPart>> for ClassKey {
    type Error = KeyError;

    fn try_from(parts: Vec<KeyPart>) -> Result<Self, KeyError> {
        Self::new(parts)
    }
}

impl From<ClassKey> for Vec<KeyPart> {
    fn from(k: ClassKey) -> Self {
        k.0
    }
}

/// An operation class: its identity and its key. Selection is `==` of the whole value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OperationClass {
    /// The operation.
    pub id: OperationId,
    /// Its key.
    pub key: ClassKey,
}

impl OperationClass {
    /// A class.
    pub fn new(id: OperationId, key: ClassKey) -> Self {
        Self { id, key }
    }

    /// A class from parts.
    ///
    /// # Errors
    ///
    /// [`KeyError`].
    pub fn of(id: OperationId, parts: Vec<KeyPart>) -> Result<Self, KeyError> {
        Ok(Self::new(id, ClassKey::new(parts)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use proptest::prelude::*;

    fn id() -> OperationId {
        OperationId::new("o1-core", "stream::BlockPass::transform")
    }

    #[test]
    fn order_of_parts_does_not_matter() {
        let a = ClassKey::new(vec![
            KeyPart::BlockBytes(256),
            KeyPart::Access(Access::Sequential),
            KeyPart::Callback("xor".into()),
        ])
        .unwrap();
        let b = ClassKey::new(vec![
            KeyPart::Callback("xor".into()),
            KeyPart::BlockBytes(256),
            KeyPart::Access(Access::Sequential),
        ])
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.parts()[0].kind(), PartKind::BlockBytes);
    }

    #[test]
    fn duplicate_kinds_are_refused() {
        let e = ClassKey::new(vec![KeyPart::BlockBytes(64), KeyPart::BlockBytes(256)]);
        assert_eq!(e, Err(KeyError::DuplicatePart(PartKind::BlockBytes)));
        let e = ClassKey::new(vec![
            KeyPart::Atomic {
                op: AtomicOp::Load,
                ordering: Ordering::Relaxed,
            },
            KeyPart::Atomic {
                op: AtomicOp::Load,
                ordering: Ordering::Relaxed,
            },
        ]);
        assert_eq!(e, Err(KeyError::DuplicatePart(PartKind::Atomic)));
    }

    #[test]
    fn key_matching_is_exact_equality() {
        let a = OperationClass::of(id(), vec![KeyPart::BlockBytes(256)]).unwrap();
        let b = OperationClass::of(id(), vec![KeyPart::BlockBytes(64)]).unwrap();
        let c = OperationClass::of(id(), vec![]).unwrap();
        let d = OperationClass::of(
            OperationId::new("o1-core", "stream::BlockPass::read"),
            vec![KeyPart::BlockBytes(256)],
        )
        .unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c, "a missing part is a different class");
        assert_ne!(a, d, "same key, other operation");
        assert_eq!(a, a.clone());
    }

    #[test]
    fn differing_names_every_kind_that_moved() {
        let a = ClassKey::new(vec![
            KeyPart::Atomic {
                op: AtomicOp::Rmw,
                ordering: Ordering::AcqRel,
            },
            KeyPart::Sharing(Sharing::Exclusive),
            KeyPart::Threads(1),
        ])
        .unwrap();
        let b = ClassKey::new(vec![
            KeyPart::Atomic {
                op: AtomicOp::Rmw,
                ordering: Ordering::Release,
            },
            KeyPart::Sharing(Sharing::WriterWithReaders),
            KeyPart::Mode("x".into()),
        ])
        .unwrap();
        assert_eq!(
            a.differing(&b),
            vec![
                PartKind::Sharing,
                PartKind::Atomic,
                PartKind::Threads,
                PartKind::Mode
            ]
        );
        assert!(a.differing(&a).is_empty());
    }

    #[test]
    fn contract_mirrors_convert() {
        assert_eq!(Access::from(o1_contract::Access::Random), Access::Random);
        assert_eq!(Reuse::from(o1_contract::Reuse::Once), Reuse::Once);
        assert_eq!(
            Write::from(o1_contract::WritePolicy::NonTemporal),
            Write::NonTemporal
        );
        assert_eq!(
            Sharing::from(o1_contract::Sharing::Handoff),
            Sharing::Handoff
        );
    }

    fn part() -> impl Strategy<Value = KeyPart> {
        prop_oneof![
            (1u32..5000).prop_map(KeyPart::BlockBytes),
            (1u32..64).prop_map(KeyPart::ElementBytes),
            prop_oneof![Just(Access::Sequential), Just(Access::Random)].prop_map(KeyPart::Access),
            (1u32..16).prop_map(KeyPart::Threads),
            "[a-z]{1,6}".prop_map(KeyPart::Mode),
        ]
    }

    proptest! {
        #[test]
        fn key_is_stable_under_permutation(parts in proptest::collection::vec(part(), 0..6), seed in any::<u64>()) {
            let mut seen = Vec::new();
            let mut uniq = Vec::new();
            for p in parts {
                if !seen.contains(&p.kind()) {
                    seen.push(p.kind());
                    uniq.push(p);
                }
            }
            let a = ClassKey::new(uniq.clone()).unwrap();
            let mut shuffled = uniq;
            let n = shuffled.len();
            if n > 1 {
                let r = (seed as usize) % n;
                shuffled.rotate_left(r);
                shuffled.reverse();
            }
            let b = ClassKey::new(shuffled).unwrap();
            prop_assert_eq!(&a, &b);
            prop_assert!(a.differing(&b).is_empty());
        }
    }
}
