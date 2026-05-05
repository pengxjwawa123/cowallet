//! secp256k1 Curve Abstraction for DKLS23
//!
//! Wraps k256 crate with DKLS23-specific traits and operations.

use k256::{
    elliptic_curve::{
        group::Group,
        sec1::{FromEncodedPoint, ToEncodedPoint, EncodedPoint},
        PrimeField,
    },
    ProjectivePoint, Secp256k1,
};
use k256::Scalar as K256Scalar;
use rand_core::OsRng;
use zeroize::Zeroize;

/// A wrapper around k256 Scalar with secure zeroization
#[derive(Debug, Clone, PartialEq, Eq, Zeroize)]
#[zeroize(drop)]
pub struct ScalarWrap(pub(crate) K256Scalar);

impl ScalarWrap {
    /// Zero scalar (additive identity)
    pub fn zero() -> Self {
        Self(K256Scalar::ZERO)
    }

    /// One scalar (multiplicative identity)
    pub fn one() -> Self {
        Self(K256Scalar::ONE)
    }

    /// Generate a random scalar using cryptographically secure RNG
    pub fn random() -> Self {
        Self(K256Scalar::generate_vartime(&mut OsRng))
    }

    /// Generate a random non-zero scalar
    pub fn random_nonzero() -> Self {
        loop {
            let s = Self::random();
            if !s.is_zero() {
                return s;
            }
        }
    }

    /// Check if scalar is zero
    pub fn is_zero(&self) -> bool {
        self.0.is_zero().into()
    }

    /// Invert scalar (mod n)
    pub fn invert(&self) -> Option<Self> {
        let inv = self.0.invert();
        if bool::from(inv.is_some()) {
            Some(Self(inv.unwrap()))
        } else {
            None
        }
    }

    /// Negate scalar
    pub fn negate(&self) -> Self {
        Self(-self.0)
    }

    /// Convert to 32-byte big-endian representation
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes().into()
    }

    /// Convert from 32-byte big-endian representation
    pub fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        use k256::elliptic_curve::generic_array::GenericArray;
        use k256::elliptic_curve::subtle::CtOption;
        let arr = GenericArray::from_slice(bytes);
        let ct_option: CtOption<K256Scalar> = K256Scalar::from_repr(*arr);
        if bool::from(ct_option.is_some()) {
            Some(Self(ct_option.unwrap()))
        } else {
            None
        }
    }

    /// Get inner scalar (consumes self)
    pub fn into_inner(self) -> K256Scalar {
        self.0
    }

    /// Get reference to inner scalar
    pub fn inner(&self) -> &K256Scalar {
        &self.0
    }
}

impl std::ops::Add for ScalarWrap {
    type Output = Self;
    fn add(self, other: Self) -> Self::Output {
        Self(self.0 + other.0)
    }
}

impl std::ops::Sub for ScalarWrap {
    type Output = Self;
    fn sub(self, other: Self) -> Self::Output {
        Self(self.0 - other.0)
    }
}

impl std::ops::Mul for ScalarWrap {
    type Output = Self;
    fn mul(self, other: Self) -> Self::Output {
        Self(self.0 * other.0)
    }
}

impl<'a> std::ops::Mul<&'a ScalarWrap> for ScalarWrap {
    type Output = Self;
    fn mul(self, other: &'a ScalarWrap) -> Self::Output {
        Self(self.0 * other.0)
    }
}

/// Elliptic curve point wrapper (secp256k1 group element)
#[derive(Debug, Clone, PartialEq, Eq, Zeroize)]
#[zeroize(drop)]
pub struct PointWrap(pub(crate) ProjectivePoint);

impl PointWrap {
    /// Point at infinity (additive identity)
    pub fn identity() -> Self {
        Self(ProjectivePoint::IDENTITY)
    }

    /// Generator point G
    pub fn generator() -> Self {
        Self(ProjectivePoint::GENERATOR)
    }

    /// Generate random point (scalar * G)
    pub fn random() -> (ScalarWrap, Self) {
        let s = ScalarWrap::random();
        let p = Self::generator() * &s;
        (s, p)
    }

    /// Check if point is identity (infinity)
    pub fn is_identity(&self) -> bool {
        self.0.is_identity().into()
    }

    /// Negate point
    pub fn negate(&self) -> Self {
        Self(-self.0)
    }

    /// Convert to compressed SEC1 bytes (33 bytes)
    pub fn to_bytes_compressed(&self) -> [u8; 33] {
        let encoded = self.0.to_encoded_point(true);
        let mut bytes = [0u8; 33];
        bytes.copy_from_slice(encoded.as_bytes());
        bytes
    }

    /// Convert to uncompressed SEC1 bytes (65 bytes)
    pub fn to_bytes_uncompressed(&self) -> [u8; 65] {
        let encoded = self.0.to_encoded_point(false);
        let mut bytes = [0u8; 65];
        bytes.copy_from_slice(encoded.as_bytes());
        bytes
    }

    /// Convert from SEC1 bytes (compressed or uncompressed)
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() == 33 {
            // Compressed
            use k256::elliptic_curve::generic_array::typenum::U33;
            use k256::elliptic_curve::generic_array::GenericArray;
            let ga: &GenericArray<u8, U33> = bytes.try_into().ok()?;
            let point = EncodedPoint::<Secp256k1>::from_bytes(ga).ok()?;
            let ct = ProjectivePoint::from_encoded_point(&point);
            if bool::from(ct.is_some()) {
                Some(Self(ct.unwrap()))
            } else {
                None
            }
        } else if bytes.len() == 65 {
            // Uncompressed
            use k256::elliptic_curve::generic_array::typenum::U65;
            use k256::elliptic_curve::generic_array::GenericArray;
            let ga: &GenericArray<u8, U65> = bytes.try_into().ok()?;
            let point = EncodedPoint::<Secp256k1>::from_bytes(ga).ok()?;
            let ct = ProjectivePoint::from_encoded_point(&point);
            if bool::from(ct.is_some()) {
                Some(Self(ct.unwrap()))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get x-coordinate (32 bytes)
    pub fn x_coord(&self) -> [u8; 32] {
        let encoded = self.0.to_encoded_point(false);
        let mut x = [0u8; 32];
        x.copy_from_slice(&encoded.as_bytes()[1..33]);
        x
    }

    /// Get y-coordinate (32 bytes)
    pub fn y_coord(&self) -> [u8; 32] {
        let encoded = self.0.to_encoded_point(false);
        let mut y = [0u8; 32];
        y.copy_from_slice(&encoded.as_bytes()[33..65]);
        y
    }

    /// Check y parity (0 = even, 1 = odd)
    pub fn y_parity(&self) -> u8 {
        let y = self.y_coord();
        y[31] & 1
    }

    /// Get inner point
    pub fn inner(&self) -> &ProjectivePoint {
        &self.0
    }
}

impl std::ops::Add for PointWrap {
    type Output = Self;
    fn add(self, other: Self) -> Self::Output {
        Self(self.0 + other.0)
    }
}

impl std::ops::Sub for PointWrap {
    type Output = Self;
    fn sub(self, other: Self) -> Self::Output {
        Self(self.0 - other.0)
    }
}

impl std::ops::Mul<ScalarWrap> for PointWrap {
    type Output = Self;
    fn mul(self, scalar: ScalarWrap) -> Self::Output {
        Self(self.0 * scalar.0)
    }
}

impl<'a> std::ops::Mul<&'a ScalarWrap> for PointWrap {
    type Output = Self;
    fn mul(self, scalar: &'a ScalarWrap) -> Self::Output {
        Self(self.0 * scalar.0)
    }
}

// Export with simpler names
pub type Scalar = ScalarWrap;
pub type Point = PointWrap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_basics() {
        let a = Scalar::random();
        let b = Scalar::random();
        let c = a.clone() + b.clone();
        let d = c - b;
        assert_eq!(d, a);
    }

    #[test]
    fn test_point_scalar_mul() {
        let s = Scalar::random();
        let g = Point::generator();
        let p = g * s.clone();
        assert!(!p.is_identity());
    }

    #[test]
    fn test_scalar_invert() {
        let s = Scalar::random_nonzero();
        let inv = s.invert().unwrap();
        let one = s.clone() * inv;
        assert_eq!(one, Scalar::one());
    }

    #[test]
    fn test_scalar_bytes_roundtrip() {
        let s = Scalar::random_nonzero();
        let bytes = s.to_bytes();
        let s2 = Scalar::from_bytes(&bytes).unwrap();
        assert_eq!(s, s2);
    }
}
