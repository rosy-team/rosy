use crate::RosyType;
use crate::{RE, CM, ST, LO, VE, DA, CD};

/// Get the return type of TYPE for a given input type.
/// TYPE always returns RE regardless of input.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::RE()),
        t if *t == RosyType::CD() => Some(RosyType::RE()),
        t if *t == RosyType::ST() => Some(RosyType::RE()),
        t if *t == RosyType::LO() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::RE()),
        _ => None,
    }
}

/// Trait for returning the COSY type code of Rosy data types.
pub trait RosyTYPE {
    fn rosy_type(&self) -> anyhow::Result<RE>;
}

/// TYPE for real numbers - code 1
impl RosyTYPE for RE {
    fn rosy_type(&self) -> anyhow::Result<RE> {
        Ok(1.0)
    }
}

/// TYPE for strings - code 2
impl RosyTYPE for ST {
    fn rosy_type(&self) -> anyhow::Result<RE> {
        Ok(2.0)
    }
}

/// TYPE for logical - code 3
impl RosyTYPE for LO {
    fn rosy_type(&self) -> anyhow::Result<RE> {
        Ok(3.0)
    }
}

/// TYPE for complex numbers - code 4
impl RosyTYPE for CM {
    fn rosy_type(&self) -> anyhow::Result<RE> {
        Ok(4.0)
    }
}

/// TYPE for vectors - code 5
impl RosyTYPE for VE {
    fn rosy_type(&self) -> anyhow::Result<RE> {
        Ok(5.0)
    }
}

/// TYPE for DA - code 6
impl RosyTYPE for DA {
    fn rosy_type(&self) -> anyhow::Result<RE> {
        Ok(6.0)
    }
}

/// TYPE for complex DA - code 7
impl RosyTYPE for CD {
    fn rosy_type(&self) -> anyhow::Result<RE> {
        Ok(7.0)
    }
}

#[cfg(test)]
mod tests {
    use super::RosyTYPE;
    use crate::{CM, CD, DA, LO, RE, ST, VE};

    #[serial_test::serial]
    #[test]
    fn type_codes_match_cosy_order_for_supported_types() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(1, 1)?;

        let re: RE = 0.1;
        let st: ST = "x".to_string();
        let lo: LO = true;
        let cm: CM = num_complex::Complex64::new(1.0, 2.0);
        let ve: VE = vec![0.1];
        let da = DA::constant(1.0);
        let cd = CD::constant(1.0);

        assert_eq!(re.rosy_type()?, 1.0);
        assert_eq!(st.rosy_type()?, 2.0);
        assert_eq!(lo.rosy_type()?, 3.0);
        assert_eq!(cm.rosy_type()?, 4.0);
        assert_eq!(ve.rosy_type()?, 5.0);
        assert_eq!(da.rosy_type()?, 6.0);
        assert_eq!(cd.rosy_type()?, 7.0);

        crate::taylor::cleanup_taylor();
        Ok(())
    }
}
