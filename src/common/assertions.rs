macro_rules! assert_lurk_err {
    ($expected:expr, $result:expr) => {
        assert_eq!($expected, $result.downcast::<LurkError>().expect("Lurk error type expected"))
    };
}

macro_rules! bail_unless_lurk_err {
    ($expected_lurk_err:expr, $actual:expr) => {
        match $actual {
            Err(err) => assert_lurk_err!($expected_lurk_err, err),
            Ok(ok) => panic!("Should fail with error {:}, instead returned {:#?}", $expected_lurk_err, ok),
        }
    };
}

pub(crate) use assert_lurk_err;
pub(crate) use bail_unless_lurk_err;
