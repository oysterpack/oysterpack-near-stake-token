use fraction::{BigUint, DynaFraction};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use std::{
    convert::TryFrom,
    fmt::{self, Display, Formatter},
};

pub type U128DynaFraction = DynaFraction<u128>;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct MixedFaction(pub u128, pub u128, pub u128);

impl MixedFaction {
    /// Returns the integer part of the value
    pub fn trunc(&self) -> u128 {
        self.0
    }

    pub fn fract_numer(&self) -> u128 {
        self.1
    }

    pub fn fract_denom(&self) -> u128 {
        self.2
    }
}

impl TryFrom<U128DynaFraction> for MixedFaction {
    type Error = BigUint;

    fn try_from(value: U128DynaFraction) -> Result<Self, Self::Error> {
        let trunc = value.clone().trunc().numer().cloned().unwrap().unpack()?;
        let fract = value.clone().fract();
        if fract == 0.into() {
            Ok(Self(trunc, 0, 1))
        } else {
            let fract_numer = fract.numer().cloned().unwrap().unpack()?;
            let fract_denom = fract.denom().cloned().unwrap().unpack()?;
            Ok(Self(trunc, fract_numer, fract_denom))
        }
    }
}

impl From<MixedFaction> for U128DynaFraction {
    fn from(value: MixedFaction) -> Self {
        U128DynaFraction::from(value.0) + U128DynaFraction::new(value.1, value.2)
    }
}

impl From<&MixedFaction> for U128DynaFraction {
    fn from(value: &MixedFaction) -> Self {
        U128DynaFraction::from(value.0) + U128DynaFraction::new(value.1, value.2)
    }
}

impl From<u128> for MixedFaction {
    fn from(value: u128) -> Self {
        MixedFaction(value, 0, 1)
    }
}

impl Display for MixedFaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.1 == 0 {
            self.0.fmt(f)
        } else {
            U128DynaFraction::from(self).fmt(f)
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    pub fn try_from_u128_dyna_fraction() {
        let fraction = MixedFaction::try_from(U128DynaFraction::new(5_u128, 3_u128)).unwrap();
        assert_eq!(fraction, MixedFaction(1, 2, 3));
        println!("{}", fraction);

        let fraction = MixedFaction::try_from(U128DynaFraction::new(5_u128, 1_u128)).unwrap();
        assert_eq!(fraction, MixedFaction(5, 0, 1));

        let fraction = MixedFaction::try_from(U128DynaFraction::new(2_u128, 3_u128)).unwrap();
        assert_eq!(fraction, MixedFaction(0, 2, 3));

        assert!(MixedFaction::try_from(U128DynaFraction::from(u128::MAX) * 2.into()).is_err())
    }
}
