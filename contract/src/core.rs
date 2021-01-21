mod hash;
mod mixed_fraction;

pub use hash::Hash;
pub use mixed_fraction::{MixedFaction, U128DynaFraction};

use uint::construct_uint;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}
