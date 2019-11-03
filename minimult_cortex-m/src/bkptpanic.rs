#[doc(hidden)]
#[macro_export]
macro_rules! bk_panic {
    ($($arg:tt)*) => ({
        if cfg!(debug_assertions) {
            panic!($($arg)*);
        }
        else {
            loop {
                cortex_m::asm::bkpt();
            }
        }
    });
}

#[doc(hidden)]
#[macro_export]
macro_rules! bk_assert {
    ($cond:expr) => ({
        if cfg!(debug_assertions) {
            assert!($cond);
        }
        else {
            if !$cond {
                loop {
                    cortex_m::asm::bkpt()
                }
            }
        }
    });
}

pub(crate) trait BKUnwrap<T>
{
    fn bk_unwrap(self) -> T;
}

impl<T> BKUnwrap<T> for Option<T>
{
    fn bk_unwrap(self) -> T
    {
        match self {
            Some(v) => v,
            None => bk_panic!("Unwrapping on `None`")
        }
    }
}