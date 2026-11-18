//! Operation plans (design §10, §11): leaves with a class, a geometry and a count, composed by
//! `Seq`, `Repeat`, `Par` and `Branch`; and the leaves that are refused by construction
//! (`Unpriced`, `Callback`, `HistoryDependent`). A plan has a digest over its canonical text.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use crate::class::{KeyPart, OperationClass};
use crate::geometry::Geometry;
use crate::refusal::UnpricedKind;

/// Why a count is not known.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Unknown {
    /// A loop or retry without a bound.
    Unbounded,
    /// Depends on data the plan does not have (names it).
    DataDependent(String),
}

/// How many calibrated units (or repetitions) are executed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Count {
    /// Known before execution.
    Exact(u64),
    /// An estimate with a named source.
    Expected {
        /// The estimate.
        value: u64,
        /// Where it comes from.
        evidence: String,
    },
    /// The structural bound only: the prediction is an upper bound (lower = 0).
    WorstCase(u64),
    /// Refused.
    Unknown(Unknown),
}

/// The basis of a count, weakest last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Basis {
    /// Every count exact.
    Exact,
    /// An expected count somewhere.
    Expected,
    /// A worst-case bound somewhere.
    WorstCase,
}

/// One priced step: a class at a geometry, executed `count` units.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Leaf {
    /// Name (what a refusal names).
    pub name: String,
    /// The class.
    pub class: OperationClass,
    /// The point on its curve.
    pub geometry: Geometry,
    /// Calibrated units executed.
    pub count: Count,
}

/// An evidence-backed probability, in millionths.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Probability {
    /// Probability × 1 000 000.
    pub per_million: u32,
    /// Where it comes from.
    pub evidence: String,
}

/// One arm of a branch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Arm {
    /// Its probability, when known with evidence.
    pub probability: Option<Probability>,
    /// What runs.
    pub plan: Plan,
}

/// An operation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Plan {
    /// A priced step.
    Leaf(Leaf),
    /// One after another; interactions (cache state left by one for the next) not modelled.
    Seq(Vec<Plan>),
    /// The body `count` times.
    Repeat {
        /// Repetitions.
        count: Count,
        /// The body.
        body: Box<Plan>,
    },
    /// Branches on separate threads, all live at once.
    Par {
        /// The branches.
        branches: Vec<Plan>,
        /// Whether any two branches write the same data.
        shared_writes: bool,
    },
    /// Exactly one arm runs.
    Branch(Vec<Arm>),
    /// Work that is never priced (kernel entry, allocation, …).
    Unpriced {
        /// Name.
        name: String,
        /// Kind.
        kind: UnpricedKind,
    },
    /// Caller-supplied code with no calibration.
    Callback {
        /// Name.
        name: String,
        /// The callback's identity.
        callback: String,
    },
    /// A cost that depends on earlier calls.
    HistoryDependent {
        /// Name.
        name: String,
        /// What it depends on.
        what: String,
    },
}

fn text(out: &mut String, s: &str) {
    let _ = write!(out, "{}:{}", s.len(), s);
}

fn part(out: &mut String, p: &KeyPart) {
    match p {
        KeyPart::BlockBytes(v) => {
            let _ = write!(out, "block_bytes={v}");
        }
        KeyPart::ElementBytes(v) => {
            let _ = write!(out, "element_bytes={v}");
        }
        KeyPart::Access(a) => {
            let _ = write!(out, "access={a:?}");
        }
        KeyPart::Reuse(r) => {
            let _ = write!(out, "reuse={r:?}");
        }
        KeyPart::Write(w) => {
            let _ = write!(out, "write={w:?}");
        }
        KeyPart::Sharing(s) => {
            let _ = write!(out, "sharing={s:?}");
        }
        KeyPart::Atomic { op, ordering } => {
            let _ = write!(out, "atomic={op:?}/{ordering:?}");
        }
        KeyPart::Isolation(i) => {
            let _ = write!(out, "isolation={i:?}");
        }
        KeyPart::Threads(t) => {
            let _ = write!(out, "threads={t}");
        }
        KeyPart::Mode(m) => {
            out.push_str("mode=");
            text(out, m);
        }
        KeyPart::Callback(c) => {
            out.push_str("callback=");
            text(out, c);
        }
        KeyPart::Contract(c) => {
            out.push_str("contract=");
            text(out, c);
        }
        KeyPart::Order(o) => {
            let _ = write!(out, "order={o:?}");
        }
    }
}

fn count(out: &mut String, c: &Count) {
    match c {
        Count::Exact(n) => {
            let _ = write!(out, "exact({n})");
        }
        Count::Expected { value, evidence } => {
            let _ = write!(out, "expected({value},");
            text(out, evidence);
            out.push(')');
        }
        Count::WorstCase(n) => {
            let _ = write!(out, "worst_case({n})");
        }
        Count::Unknown(Unknown::Unbounded) => out.push_str("unknown(unbounded)"),
        Count::Unknown(Unknown::DataDependent(w)) => {
            out.push_str("unknown(data_dependent,");
            text(out, w);
            out.push(')');
        }
    }
}

impl Plan {
    /// The canonical text of the plan: every field, lengths before strings, so two plans have
    /// one text exactly when they are equal.
    pub fn canonical(&self) -> String {
        let mut out = String::new();
        self.write_canonical(&mut out);
        out
    }

    fn write_canonical(&self, out: &mut String) {
        match self {
            Plan::Leaf(l) => {
                out.push_str("leaf(");
                text(out, &l.name);
                out.push(',');
                text(out, &l.class.id.package);
                out.push(',');
                text(out, &l.class.id.path);
                out.push_str(",[");
                for p in l.class.key.parts() {
                    part(out, p);
                    out.push(';');
                }
                let _ = write!(
                    out,
                    "],{}={},",
                    l.geometry.parameter.name(),
                    l.geometry.value
                );
                count(out, &l.count);
                out.push(')');
            }
            Plan::Seq(v) => {
                out.push_str("seq(");
                for p in v {
                    p.write_canonical(out);
                    out.push(',');
                }
                out.push(')');
            }
            Plan::Repeat { count: c, body } => {
                out.push_str("repeat(");
                count(out, c);
                out.push(',');
                body.write_canonical(out);
                out.push(')');
            }
            Plan::Par {
                branches,
                shared_writes,
            } => {
                let _ = write!(out, "par({shared_writes},");
                for p in branches {
                    p.write_canonical(out);
                    out.push(',');
                }
                out.push(')');
            }
            Plan::Branch(arms) => {
                out.push_str("branch(");
                for a in arms {
                    match &a.probability {
                        Some(p) => {
                            let _ = write!(out, "p({},", p.per_million);
                            text(out, &p.evidence);
                            out.push(')');
                        }
                        None => out.push_str("p(none)"),
                    }
                    out.push(':');
                    a.plan.write_canonical(out);
                    out.push(',');
                }
                out.push(')');
            }
            Plan::Unpriced { name, kind } => {
                out.push_str("unpriced(");
                text(out, name);
                let _ = write!(out, ",{kind:?})");
            }
            Plan::Callback { name, callback } => {
                out.push_str("callback(");
                text(out, name);
                out.push(',');
                text(out, callback);
                out.push(')');
            }
            Plan::HistoryDependent { name, what } => {
                out.push_str("history(");
                text(out, name);
                out.push(',');
                text(out, what);
                out.push(')');
            }
        }
    }

    /// `sha256:` of [`Plan::canonical`].
    pub fn digest(&self) -> String {
        crate::digest::tagged(self.canonical().as_bytes())
    }

    /// Every branch-free plan: one per combination of arm choices, each `Branch` replaced by
    /// its chosen arm's plan, with the choices (arm index per branch, in walk order).
    pub fn paths(&self) -> Vec<(Vec<usize>, Plan)> {
        match self {
            Plan::Leaf(_)
            | Plan::Unpriced { .. }
            | Plan::Callback { .. }
            | Plan::HistoryDependent { .. } => alloc::vec![(Vec::new(), self.clone())],
            Plan::Repeat { count: c, body } => body
                .paths()
                .into_iter()
                .map(|(ch, b)| {
                    (
                        ch,
                        Plan::Repeat {
                            count: c.clone(),
                            body: Box::new(b),
                        },
                    )
                })
                .collect(),
            Plan::Seq(v) => combine(v)
                .into_iter()
                .map(|(ch, ps)| (ch, Plan::Seq(ps)))
                .collect(),
            Plan::Par {
                branches,
                shared_writes,
            } => combine(branches)
                .into_iter()
                .map(|(ch, ps)| {
                    (
                        ch,
                        Plan::Par {
                            branches: ps,
                            shared_writes: *shared_writes,
                        },
                    )
                })
                .collect(),
            Plan::Branch(arms) => {
                let mut out = Vec::new();
                for (i, a) in arms.iter().enumerate() {
                    for (mut ch, p) in a.plan.paths() {
                        ch.insert(0, i);
                        out.push((ch, p));
                    }
                }
                out
            }
        }
    }
}

fn combine(children: &[Plan]) -> Vec<(Vec<usize>, Vec<Plan>)> {
    let mut acc: Vec<(Vec<usize>, Vec<Plan>)> = alloc::vec![(Vec::new(), Vec::new())];
    for c in children {
        let options = c.paths();
        let mut next = Vec::with_capacity(acc.len() * options.len());
        for (ch, ps) in &acc {
            for (och, op) in &options {
                let mut ch2 = ch.clone();
                ch2.extend_from_slice(och);
                let mut ps2 = ps.clone();
                ps2.push(op.clone());
                next.push((ch2, ps2));
            }
        }
        acc = next;
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Parameter;
    use crate::selector::tests::transform;
    use alloc::vec;

    fn leaf(name: &str, v: u64) -> Plan {
        Plan::Leaf(Leaf {
            name: name.into(),
            class: transform(256),
            geometry: Geometry::new(Parameter::ExtentBytes, v).unwrap(),
            count: Count::Exact(v),
        })
    }

    #[test]
    fn digest_is_stable_and_distinguishes() {
        let a = Plan::Seq(vec![leaf("a", 16384), leaf("b", 65536)]);
        let b = Plan::Seq(vec![leaf("a", 16384), leaf("b", 65536)]);
        assert_eq!(a.digest(), b.digest());
        assert!(a.digest().starts_with("sha256:"));
        let c = Plan::Seq(vec![leaf("b", 65536), leaf("a", 16384)]);
        assert_ne!(a.digest(), c.digest(), "order matters in a Seq");
        let d = Plan::Seq(vec![leaf("a", 16384), leaf("b", 65537)]);
        assert_ne!(a.digest(), d.digest());
        // Length-prefixed strings: no two names collide by concatenation.
        let e = Plan::Unpriced {
            name: "ab".into(),
            kind: UnpricedKind::Allocation,
        };
        let f = Plan::Unpriced {
            name: "a".into(),
            kind: UnpricedKind::Allocation,
        };
        assert_ne!(e.canonical(), f.canonical());
    }

    #[test]
    fn paths_enumerate_every_arm_combination() {
        let br = |a: Plan, b: Plan| {
            Plan::Branch(vec![
                Arm {
                    probability: None,
                    plan: a,
                },
                Arm {
                    probability: None,
                    plan: b,
                },
            ])
        };
        let p = Plan::Seq(vec![
            br(leaf("a", 1), leaf("b", 2)),
            br(leaf("c", 3), leaf("d", 4)),
        ]);
        let paths = p.paths();
        assert_eq!(paths.len(), 4);
        assert_eq!(paths[0].0, vec![0, 0]);
        assert_eq!(paths[3].0, vec![1, 1]);
        assert_eq!(paths[3].1, Plan::Seq(vec![leaf("b", 2), leaf("d", 4)]));
        assert_eq!(leaf("a", 1).paths().len(), 1);
    }
}
