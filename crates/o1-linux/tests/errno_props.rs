//! The error-return decoding is pure, so it is property-tested on every host.

use o1_linux::errno::{Errno, MAX_ERRNO, decode};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn decode_matches_signed_reference(ret in any::<usize>()) {
        let signed = ret as isize;
        let expect = if (-(MAX_ERRNO as isize)..=-1).contains(&signed) {
            Err(Errno::new((-signed) as u16).unwrap())
        } else {
            Ok(ret)
        };
        prop_assert_eq!(decode(ret), expect);
    }

    #[test]
    fn every_errno_roundtrips(e in 1u16..=4095) {
        let ret = (-(e as isize)) as usize;
        prop_assert_eq!(decode(ret).unwrap_err().raw(), e);
    }
}
