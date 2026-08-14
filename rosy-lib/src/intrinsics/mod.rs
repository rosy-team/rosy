pub mod cm;
pub mod st;
pub mod lo;
pub mod from_st;
pub mod length;
pub mod sin;
pub mod cos;
pub mod asin;
pub mod acos;
pub mod atan;
pub mod sinh;
pub mod cosh;
pub mod tanh;
pub mod sqr;
pub mod sqrt;
pub mod exp;
pub mod log;
pub mod tan;
pub mod vmax;
pub mod vmin;
pub mod abs;
pub mod norm;
pub mod cons;
pub mod int_fn;
pub mod nint;
pub mod type_fn;
pub mod trim;
pub mod ltrim;
pub mod isrt;
pub mod isrt3;
pub mod cmplx;
pub mod conj;
pub mod mem_size;
pub mod derive;
pub mod real_fn;
pub mod imag_fn;
pub mod re_convert;
pub mod ve_convert;
pub mod varmem;
pub mod varpoi;
pub mod erf;
pub mod werf;
pub mod position;

pub use cm::RosyCM;
pub use st::RosyST;
pub use lo::RosyLO;
pub use from_st::RosyFromST;
pub use length::RosyLENGTH;
pub use sin::RosySIN;
pub use cos::RosyCOS;
pub use asin::RosyASIN;
pub use acos::RosyACOS;
pub use atan::RosyATAN;
pub use sinh::RosySINH;
pub use cosh::RosyCOSH;
pub use tanh::RosyTANH;
pub use sqr::RosySQR;
pub use sqrt::RosySQRT;
pub use exp::RosyEXP;
pub use log::RosyLOG;
pub use tan::RosyTAN;
pub use vmax::RosyVMAX;
pub use vmin::RosyVMIN;
pub use abs::RosyABS;
pub use norm::RosyNORM;
pub use cons::RosyCONS;
pub use int_fn::RosyINT;
pub use nint::RosyNINT;
pub use type_fn::RosyTYPE;
pub use trim::RosyTRIM;
pub use ltrim::RosyLTRIM;
pub use isrt::RosyISRT;
pub use isrt3::RosyISRT3;
pub use cmplx::RosyCMPLX;
pub use conj::RosyCONJ;
pub use mem_size::{RosyLST, RosyLCM, RosyLCD, RosyLRE, RosyLLO, RosyLVE, RosyLDA};
pub use derive::RosyDerive;
pub use real_fn::RosyREAL;
pub use imag_fn::RosyIMAG;
pub use re_convert::RosyREConvert;
pub use ve_convert::RosyVEConvert;
pub use varmem::RosyVARMEM;
pub use varpoi::RosyVARPOI;
pub use erf::RosyERF;
pub use werf::RosyWERF;
pub use position::RosyPOSITION;

/// Represents a parsed intrinsic type rule from the source code.
#[derive(Debug, Clone)]
pub struct IntrinsicTypeRule {
    pub input: &'static str,
    pub result: &'static str,
    pub test_val: &'static str,
}
impl IntrinsicTypeRule {
    /// Create a new intrinsic type rule.
    pub const fn new(
        input: &'static str,
        result: &'static str,
        test_val: &'static str
    ) -> Self {
        Self { input, result, test_val }
    }
}
