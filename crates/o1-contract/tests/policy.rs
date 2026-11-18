//! Policy validation: every dynamic combination against an independently written truth
//! table, and every accepted static combination against its description.

use o1_contract::policy::{NonTemporalWrite, Random, ReuseOnce, Reused, Sequential, Unset};
use o1_contract::{
    Access, Axis, Bound, Cost, ExecutionContract, Isolation, Policy, PolicyDesc, PolicyError,
    Progress, Quantity, ResourceEnvelope, Reuse, Sharing, StaticPolicy, Unit, WritePolicy,
};

fn all_descs() -> Vec<PolicyDesc> {
    let access = [None, Some(Access::Sequential), Some(Access::Random)];
    let reuse = [None, Some(Reuse::Once), Some(Reuse::Reused)];
    let sharing = [
        None,
        Some(Sharing::Exclusive),
        Some(Sharing::SharedRead),
        Some(Sharing::Handoff),
    ];
    let write = [
        None,
        Some(WritePolicy::Ordinary),
        Some(WritePolicy::NonTemporal),
    ];
    let mut v = Vec::new();
    for a in access {
        for r in reuse {
            for s in sharing {
                for w in write {
                    v.push(PolicyDesc {
                        access: a,
                        reuse: r,
                        sharing: s,
                        write: w,
                    });
                }
            }
        }
    }
    v
}

/// The rules, written as a list of forbidden pairs rather than as the library's branches.
fn reference_validate(d: &PolicyDesc) -> Result<(), PolicyError> {
    let forbidden: [(bool, PolicyError); 2] = [
        (
            d.write == Some(WritePolicy::NonTemporal) && d.reuse == Some(Reuse::Reused),
            PolicyError::NonTemporalReused,
        ),
        (
            d.write.is_some() && d.sharing == Some(Sharing::SharedRead),
            PolicyError::WriteOnSharedRead,
        ),
    ];
    forbidden
        .into_iter()
        .find(|(hit, _)| *hit)
        .map_or(Ok(()), |(_, e)| Err(e))
}

fn reference_merge<T: PartialEq + Copy>(
    r: Option<T>,
    s: Option<T>,
    axis: Axis,
) -> Result<Option<T>, PolicyError> {
    match (r, s) {
        (Some(r), Some(s)) if r != s => Err(PolicyError::ContradictsStructure { axis }),
        _ => Ok(s.or(r)),
    }
}

#[test]
fn every_dynamic_combination_matches_the_truth_table() {
    let all = all_descs();
    assert_eq!(all.len(), 3 * 3 * 4 * 3);
    let valid = all.iter().filter(|d| d.validate().is_ok()).count();
    for d in &all {
        assert_eq!(d.validate(), reference_validate(d), "{d:?}");
    }
    // 108 combinations; NT+Reused removes 3·4 = 12; any write on SharedRead removes 3·3·2 = 18;
    // both at once (NT, Reused, SharedRead) counted twice: 3. 108 − 12 − 18 + 3 = 81.
    assert_eq!(valid, 81);
}

#[test]
fn every_request_against_every_structure() {
    let all = all_descs();
    for req in &all {
        for st in &all {
            let expect = (|| {
                let merged = PolicyDesc {
                    access: reference_merge(req.access, st.access, Axis::Access)?,
                    reuse: reference_merge(req.reuse, st.reuse, Axis::Reuse)?,
                    sharing: reference_merge(req.sharing, st.sharing, Axis::Sharing)?,
                    write: reference_merge(req.write, st.write, Axis::Write)?,
                };
                reference_validate(&merged).map(|()| merged)
            })();
            assert_eq!(req.validate_against(st), expect, "{req:?} on {st:?}");
        }
    }
}

macro_rules! static_desc {
    ($a:ty, $r:ty, $w:ty) => {
        <Policy<$a, $r, $w> as StaticPolicy>::DESC
    };
}

#[test]
fn every_accepted_static_combination_describes_itself() {
    // 3 access × 3 reuse × 2 write = 18 types; the 3 with Reused + NonTemporalWrite do not
    // compile (compile_fail doc-tests in `o1_contract::policy`). The other 15:
    let got = [
        static_desc!(Unset, Unset, Unset),
        static_desc!(Unset, Unset, NonTemporalWrite),
        static_desc!(Unset, ReuseOnce, Unset),
        static_desc!(Unset, ReuseOnce, NonTemporalWrite),
        static_desc!(Unset, Reused, Unset),
        static_desc!(Sequential, Unset, Unset),
        static_desc!(Sequential, Unset, NonTemporalWrite),
        static_desc!(Sequential, ReuseOnce, Unset),
        static_desc!(Sequential, ReuseOnce, NonTemporalWrite),
        static_desc!(Sequential, Reused, Unset),
        static_desc!(Random, Unset, Unset),
        static_desc!(Random, Unset, NonTemporalWrite),
        static_desc!(Random, ReuseOnce, Unset),
        static_desc!(Random, ReuseOnce, NonTemporalWrite),
        static_desc!(Random, Reused, Unset),
    ];
    // The same set, derived from the dynamic rules: sharing unset, write ∈ {unset, NT}.
    let expect: Vec<PolicyDesc> = all_descs()
        .into_iter()
        .filter(|d| d.sharing.is_none() && d.write != Some(WritePolicy::Ordinary))
        .filter(|d| d.validate().is_ok())
        .collect();
    assert_eq!(got.len(), expect.len());
    for d in &got {
        assert!(expect.contains(d), "{d:?}");
    }
}

#[test]
fn builder_reaches_the_same_types() {
    let p = Policy::new().streaming().non_temporal_write();
    assert_eq!(
        p.desc(),
        static_desc!(Sequential, ReuseOnce, NonTemporalWrite)
    );
    let q = Policy::new().non_temporal_write().random().reuse_once();
    assert_eq!(q.desc(), static_desc!(Random, ReuseOnce, NonTemporalWrite));
    assert_eq!(
        Policy::new().reused().sequential().desc().access,
        Some(Access::Sequential)
    );
    assert_eq!(core::mem::size_of_val(&p), 0);
    assert_eq!(core::mem::size_of::<Policy<Random, Reused, Unset>>(), 0);
    const P: Policy<Sequential, ReuseOnce> = Policy::new().streaming();
    assert_eq!(P.desc().reuse, Some(Reuse::Once));
}

#[test]
fn bound_is_derived_from_cost() {
    let one = ResourceEnvelope::ZERO.with(Quantity::Ops, 1);
    let mut c = ExecutionContract {
        progress: Progress::NoSynchronization,
        policy: PolicyDesc::NONE,
        isolation: Isolation::NoClaim,
        cost: Cost::constant(one),
    };
    assert_eq!(c.bound(), Bound::Constant);
    c.cost.per_unit = one;
    c.cost.unit = Unit::Byte;
    assert_eq!(c.bound(), Bound::PerUnit(Unit::Byte));
    c.cost.max_units = Some(4096);
    assert_eq!(
        c.bound(),
        Bound::Capacity {
            unit: Unit::Byte,
            max: 4096
        }
    );
    assert!(c.validate().is_ok());
    c.policy.write = Some(WritePolicy::Ordinary);
    c.policy.sharing = Some(Sharing::SharedRead);
    assert_eq!(c.validate(), Err(PolicyError::WriteOnSharedRead));
}
