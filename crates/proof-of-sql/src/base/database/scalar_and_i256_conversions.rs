use crate::base::scalar::Scalar;
use arrow::datatypes::i256;

const MIN_SUPPORTED_I256: i256 = i256::from_parts(
    326411208032252286695448638536326387210,
    -10633823966279326983230456482242756609,
);
const MAX_SUPPORTED_I256: i256 = i256::from_parts(
    13871158888686176767925968895441824246,
    10633823966279326983230456482242756608,
);

/// Converts a type implementing [Scalar] into an arrow i256
pub fn convert_scalar_to_i256<S: Scalar>(val: &S) -> i256 {
    let is_negative = val > &S::MAX_SIGNED;
    let abs_scalar = if is_negative { -*val } else { *val };
    let limbs: [u64; 4] = abs_scalar.into();

    let low = (limbs[0] as u128) | ((limbs[1] as u128) << 64);
    let high = (limbs[2] as i128) | ((limbs[3] as i128) << 64);

    let abs_i256 = i256::from_parts(low, high);
    if is_negative {
        i256::wrapping_neg(abs_i256)
    } else {
        abs_i256
    }
}

/// Converts an arrow i256 into limbed representation and then
/// into a type implementing [Scalar]
#[must_use] pub fn convert_i256_to_scalar<S: Scalar>(value: &i256) -> Option<S> {
    // Check if value is within the bounds
    if value < &MIN_SUPPORTED_I256 || value > &MAX_SUPPORTED_I256 {
        None
    } else {
        // Prepare the absolute value for conversion
        let abs_value = if value.is_negative() { -*value } else { *value };
        let (low, high) = abs_value.to_parts();
        let limbs = [
            low as u64,
            (low >> 64) as u64,
            high as u64,
            (high >> 64) as u64,
        ];

        // Convert limbs to Scalar and adjust for sign
        let scalar: S = limbs.into();
        Some(if value.is_negative() { -scalar } else { scalar })
    }
}

#[cfg(test)]
mod tests {

    use super::{convert_i256_to_scalar, convert_scalar_to_i256};
    use crate::base::{
        database::scalar_and_i256_conversions::{MAX_SUPPORTED_I256, MIN_SUPPORTED_I256},
        scalar::{Curve25519Scalar, Scalar},
    };
    use arrow::datatypes::i256;
    use num_traits::Zero;
    use rand::RngCore;
    /// Generate a random i256 within a supported range. Values generated by this function will
    /// fit into the i256 but will not exceed 252 bits of width.
    fn random_i256<R: RngCore + ?Sized>(rng: &mut R) -> i256 {
        use rand::Rng;
        let max_signed_as_parts: (u128, i128) =
            convert_scalar_to_i256(&Curve25519Scalar::MAX_SIGNED).to_parts();

        // Generate a random high part
        let high: i128 = rng.gen_range(-max_signed_as_parts.1..=max_signed_as_parts.1);

        // Generate a random low part, adjusted based on the high part
        let low: u128 = if high < max_signed_as_parts.1 {
            rng.gen()
        } else {
            rng.gen_range(0..=max_signed_as_parts.0)
        };

        i256::from_parts(low, high)
    }

    impl TryFrom<i256> for Curve25519Scalar {
        type Error = ();

        // Must fit inside 252 bits and so requires fallible
        fn try_from(value: i256) -> Result<Self, ()> {
            convert_i256_to_scalar(&value).ok_or(())
        }
    }
    impl From<Curve25519Scalar> for i256 {
        fn from(value: Curve25519Scalar) -> Self {
            convert_scalar_to_i256(&value)
        }
    }

    #[test]
    fn test_curve25519scalar_to_i256_conversion() {
        let positive_scalar = Curve25519Scalar::from(12345);
        let expected_i256 = i256::from(12345);
        assert_eq!(i256::from(positive_scalar), expected_i256);

        let negative_scalar = Curve25519Scalar::from(-12345);
        let expected_i256 = i256::from(-12345);
        assert_eq!(i256::from(negative_scalar), expected_i256);

        let max_scalar = Curve25519Scalar::MAX_SIGNED;
        let expected_max = i256::from(Curve25519Scalar::MAX_SIGNED);
        assert_eq!(i256::from(max_scalar), expected_max);

        let min_scalar = Curve25519Scalar::from(0);
        let expected_min = i256::from(Curve25519Scalar::from(0));
        assert_eq!(i256::from(min_scalar), expected_min);
    }

    #[test]
    fn test_curve25519scalar_i256_overflow_and_underflow() {
        // 2^256 overflows
        assert!(Curve25519Scalar::try_from(i256::MAX).is_err());

        // MAX_SIGNED + 1 overflows
        assert!(Curve25519Scalar::try_from(MAX_SUPPORTED_I256 + i256::from(1)).is_err());

        // -2^255 underflows
        assert!(i256::MIN < -(i256::from(Curve25519Scalar::MAX_SIGNED)));
        assert!(Curve25519Scalar::try_from(i256::MIN).is_err());

        // -MAX-SIGNED - 1 underflows
        assert!(Curve25519Scalar::try_from(MIN_SUPPORTED_I256 - i256::from(1)).is_err());
    }

    #[test]
    fn test_i256_curve25519scalar_negative() {
        // Test conversion from i256(-1) to Curve25519Scalar
        let neg_one_i256_curve25519scalar = Curve25519Scalar::try_from(i256::from(-1));
        assert!(neg_one_i256_curve25519scalar.is_ok());
        let neg_one_curve25519scalar = Curve25519Scalar::from(-1);
        assert_eq!(
            neg_one_i256_curve25519scalar.unwrap(),
            neg_one_curve25519scalar
        );
    }

    #[test]
    fn test_i256_curve25519scalar_zero() {
        // Test conversion from i256(0) to Curve25519Scalar
        let zero_i256_curve25519scalar = Curve25519Scalar::try_from(i256::from(0));
        assert!(zero_i256_curve25519scalar.is_ok());
        let zero_curve25519scalar = Curve25519Scalar::zero();
        assert_eq!(zero_i256_curve25519scalar.unwrap(), zero_curve25519scalar);
    }

    #[test]
    fn test_i256_curve25519scalar_positive() {
        // Test conversion from i256(42) to Curve25519Scalar
        let forty_two_i256_curve25519scalar = Curve25519Scalar::try_from(i256::from(42));
        let forty_two_curve25519scalar = Curve25519Scalar::from(42);
        assert_eq!(
            forty_two_i256_curve25519scalar.unwrap(),
            forty_two_curve25519scalar
        );
    }

    #[test]
    fn test_i256_curve25519scalar_max_signed() {
        let max_signed = MAX_SUPPORTED_I256;
        // max signed value
        let max_signed_scalar = Curve25519Scalar::MAX_SIGNED;
        // Test conversion from i256 to Curve25519Scalar
        let i256_curve25519scalar = Curve25519Scalar::try_from(max_signed);
        assert!(i256_curve25519scalar.is_ok());
        assert_eq!(i256_curve25519scalar.unwrap(), max_signed_scalar);
    }

    #[test]
    fn test_i256_curve25519scalar_min_signed() {
        let min_signed = MIN_SUPPORTED_I256;
        let i256_curve25519scalar = Curve25519Scalar::try_from(min_signed);
        // -MAX_SIGNED is ok
        assert!(i256_curve25519scalar.is_ok());
        assert_eq!(
            i256_curve25519scalar.unwrap(),
            Curve25519Scalar::MAX_SIGNED + Curve25519Scalar::from(1)
        );
    }

    #[test]
    fn test_i256_curve25519scalar_random() {
        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            let i256_value = random_i256(&mut rng);
            let curve25519_scalar =
                Curve25519Scalar::try_from(i256_value).expect("Conversion failed");
            let back_to_i256 = i256::from(curve25519_scalar);
            assert_eq!(i256_value, back_to_i256, "Round-trip conversion failed");
        }
    }
}
