pub mod abs;
pub mod acos;
pub mod asin;
pub mod atan;
pub mod cm;
pub mod cmplx;
pub mod conj;
pub mod cons;
pub mod cos;
pub mod cosh;
pub mod derive;
pub mod erf;
pub mod exp;
pub mod from_st;
pub mod imag_fn;
pub mod int_fn;
pub mod isrt;
pub mod isrt3;
pub mod length;
pub mod lo;
pub mod log;
pub mod ltrim;
pub mod mem_size;
pub mod nint;
pub mod norm;
pub mod position;
pub mod re_convert;
pub mod real_fn;
pub mod sin;
pub mod sinh;
pub mod sqr;
pub mod sqrt;
pub mod st;
pub mod tan;
pub mod tanh;
pub mod trim;
pub mod type_fn;
pub mod varmem;
pub mod varpoi;
pub mod ve_convert;
pub mod vmax;
pub mod vmin;
pub mod werf;

pub use abs::RosyABS;
pub use acos::RosyACOS;
pub use asin::RosyASIN;
pub use atan::RosyATAN;
pub use cm::RosyCM;
pub use cmplx::RosyCMPLX;
pub use conj::RosyCONJ;
pub use cons::RosyCONS;
pub use cos::RosyCOS;
pub use cosh::RosyCOSH;
pub use derive::RosyDerive;
pub use erf::RosyERF;
pub use exp::RosyEXP;
pub use from_st::RosyFromST;
pub use imag_fn::RosyIMAG;
pub use int_fn::RosyINT;
pub use isrt::RosyISRT;
pub use isrt3::RosyISRT3;
pub use length::RosyLENGTH;
pub use lo::RosyLO;
pub use log::RosyLOG;
pub use ltrim::RosyLTRIM;
pub use mem_size::{RosyLCD, RosyLCM, RosyLDA, RosyLLO, RosyLRE, RosyLST, RosyLVE};
pub use nint::RosyNINT;
pub use norm::RosyNORM;
pub use position::RosyPOSITION;
pub use re_convert::RosyREConvert;
pub use real_fn::RosyREAL;
pub use sin::RosySIN;
pub use sinh::RosySINH;
pub use sqr::RosySQR;
pub use sqrt::RosySQRT;
pub use st::RosyST;
pub use tan::RosyTAN;
pub use tanh::RosyTANH;
pub use trim::RosyTRIM;
pub use type_fn::RosyTYPE;
pub use varmem::RosyVARMEM;
pub use varpoi::RosyVARPOI;
pub use ve_convert::RosyVEConvert;
pub use vmax::RosyVMAX;
pub use vmin::RosyVMIN;
pub use werf::RosyWERF;

/// Represents a parsed intrinsic type rule from the source code.
#[derive(Debug, Clone)]
pub struct IntrinsicTypeRule {
    pub input: &'static str,
    pub result: &'static str,
    pub test_val: &'static str,
}
impl IntrinsicTypeRule {
    /// Create a new intrinsic type rule.
    pub const fn new(input: &'static str, result: &'static str, test_val: &'static str) -> Self {
        Self {
            input,
            result,
            test_val,
        }
    }
}
