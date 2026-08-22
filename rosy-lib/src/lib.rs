#![cfg_attr(feature = "nightly-simd", feature(portable_simd))]

//! # Rosy Runtime Library
//!
//! The embedded runtime library that ships with every generated Rust project.
//! Contains operator implementations, intrinsic functions, type definitions,
//! MPI support, Taylor series (DA/CD), and the optimizer.
//!
//! ## Type Aliases
//!
//! | Rosy Type | Rust Type | Description |
//! |-----------|-----------|-------------|
//! | `RE` | `f64` | Real number |
//! | `ST` | `String` | String |
//! | `LO` | `bool` | Logical (boolean) |
//! | `CM` | `Complex64` | Complex number |
//! | `VE` | `Vec<f64>` | Vector of reals |
//! | `DA` | [`taylor::DA`] | Differential Algebra (Taylor series) |
//! | `CD` | [`taylor::CD`] | Complex Differential Algebra |
//!
//! ## Sub-modules
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`operators`] | Binary operator dispatch (add, sub, mult, div, etc.) |
//! | [`intrinsics`] | Built-in math functions (sin, sqr, exp, etc.) |
//! | [`core`] | Core I/O, file management, concatenation |
//! | [`taylor`] | DA/CD Taylor series implementation |
//! | `mpi` | MPI parallel context |
//! | [`optimizer`] | FIT loop optimization algorithms |

pub mod core;
pub mod value;
pub mod intrinsics;
#[cfg(feature = "mpi")]
pub mod mpi;
pub mod operators;
pub mod optimizer;
pub mod registry;
pub mod taylor;

pub use core::*;
pub use value::{RosyValue, rosy_dyn_binary};
pub use intrinsics::*;
#[cfg(feature = "mpi")]
pub use mpi::*;
pub use operators::*;
pub use registry::{
    BinaryOp, INTRINSICS, Intrinsic, IntrinsicTyping, UnaryOp, binary_return_type,
    lookup_intrinsic, unary_return_type,
};

pub use taylor::{CD, DA};

pub trait ExpectReExt {
    fn expect_re(&self) -> anyhow::Result<f64>;
}
impl ExpectReExt for f64 {
    fn expect_re(&self) -> anyhow::Result<f64> {
        Ok(*self)
    }
}

pub trait LenExt {
    fn len(&self) -> usize;
}
impl LenExt for f64 {
    fn len(&self) -> usize {
        1
    }
}

/// Anything the generated code might use as a real / index / cast source.
pub trait IntoF64 {
    fn into_f64(self) -> f64;
}
impl IntoF64 for f64 {
    fn into_f64(self) -> f64 {
        self
    }
}
impl IntoF64 for usize {
    fn into_f64(self) -> f64 {
        self as f64
    }
}
impl IntoF64 for u64 {
    fn into_f64(self) -> f64 {
        self as f64
    }
}
impl IntoF64 for u32 {
    fn into_f64(self) -> f64 {
        self as f64
    }
}
impl IntoF64 for &f64 {
    fn into_f64(self) -> f64 {
        *self
    }
}
impl IntoF64 for RosyValue {
    fn into_f64(self) -> f64 {
        self.as_f64()
    }
}
impl IntoF64 for &RosyValue {
    fn into_f64(self) -> f64 {
        self.as_f64()
    }
}
impl IntoF64 for &mut RosyValue {
    fn into_f64(self) -> f64 {
        self.as_f64()
    }
}

pub trait AsF64 {
    fn as_f64_val(&self) -> f64;
}
impl AsF64 for f64 {
    fn as_f64_val(&self) -> f64 {
        *self
    }
}
impl AsF64 for usize {
    fn as_f64_val(&self) -> f64 {
        *self as f64
    }
}
impl AsF64 for u64 {
    fn as_f64_val(&self) -> f64 {
        *self as f64
    }
}
impl AsF64 for String {
    fn as_f64_val(&self) -> f64 {
        self.trim().parse().unwrap_or(0.0)
    }
}
impl AsF64 for RosyValue {
    fn as_f64_val(&self) -> f64 {
        self.as_f64()
    }
}
impl AsF64 for DA {
    fn as_f64_val(&self) -> f64 {
        self.constant_part()
    }
}
impl<T: AsF64 + ?Sized> AsF64 for &T {
    fn as_f64_val(&self) -> f64 {
        (**self).as_f64_val()
    }
}

#[inline]
pub fn rosy_as_usize(v: &impl AsF64) -> usize {
    v.as_f64_val() as usize
}
#[inline]
pub fn rosy_as_u32(v: &impl AsF64) -> u32 {
    v.as_f64_val() as u32
}
#[inline]
pub fn rosy_as_u64(v: &impl AsF64) -> u64 {
    v.as_f64_val() as u64
}
#[inline]
pub fn rosy_as_i64(v: &impl AsF64) -> i64 {
    v.as_f64_val() as i64
}
#[inline]
pub fn rosy_as_f64(v: &impl AsF64) -> f64 {
    v.as_f64_val()
}

/// Write a real into either `f64` or an `ANY` cell.
pub trait SetF64 {
    fn set_f64(&mut self, v: f64);
}
impl SetF64 for f64 {
    fn set_f64(&mut self, v: f64) {
        *self = v;
    }
}
impl SetF64 for RosyValue {
    fn set_f64(&mut self, v: f64) {
        *self = RosyValue::RE(v);
    }
}

pub trait RecstFmt {
    fn recst_fmt(&self) -> String;
}
impl RecstFmt for str {
    fn recst_fmt(&self) -> String {
        self.to_string()
    }
}
impl RecstFmt for String {
    fn recst_fmt(&self) -> String {
        self.clone()
    }
}
impl RecstFmt for &String {
    fn recst_fmt(&self) -> String {
        (*self).clone()
    }
}
impl RecstFmt for &str {
    fn recst_fmt(&self) -> String {
        (*self).to_string()
    }
}
impl RecstFmt for RosyValue {
    fn recst_fmt(&self) -> String {
        match self {
            RosyValue::ST(s) => s.clone(),
            other => other.rosy_display(),
        }
    }
}
impl RecstFmt for &RosyValue {
    fn recst_fmt(&self) -> String {
        (*self).recst_fmt()
    }
}

pub trait PolvalDaSrc {
    fn to_da_vec(&self) -> Vec<DA>;
}
impl PolvalDaSrc for Vec<f64> {
    fn to_da_vec(&self) -> Vec<DA> {
        Vec::new()
    }
}
impl PolvalDaSrc for Vec<DA> {
    fn to_da_vec(&self) -> Vec<DA> {
        self.clone()
    }
}
impl PolvalDaSrc for [DA] {
    fn to_da_vec(&self) -> Vec<DA> {
        self.to_vec()
    }
}
impl PolvalDaSrc for Vec<RosyValue> {
    fn to_da_vec(&self) -> Vec<DA> {
        self.iter()
            .map(|v| v.clone().expect_da().unwrap_or_else(|_| DA::zero()))
            .collect()
    }
}
impl PolvalDaSrc for [RosyValue] {
    fn to_da_vec(&self) -> Vec<DA> {
        self.iter()
            .map(|v| v.clone().expect_da().unwrap_or_else(|_| DA::zero()))
            .collect()
    }
}
impl PolvalDaSrc for RosyValue {
    fn to_da_vec(&self) -> Vec<DA> {
        match self {
            RosyValue::DA(d) => vec![d.clone()],
            RosyValue::Arr(v) => v
                .iter()
                .map(|x| x.clone().expect_da().unwrap_or_else(|_| DA::zero()))
                .collect(),
            _ => Vec::new(),
        }
    }
}

pub trait PolvalReSrc {
    fn to_re_vec(&self) -> Vec<f64>;
}
impl PolvalReSrc for Vec<f64> {
    fn to_re_vec(&self) -> Vec<f64> {
        self.clone()
    }
}
impl PolvalReSrc for f64 {
    fn to_re_vec(&self) -> Vec<f64> {
        vec![*self]
    }
}
impl PolvalReSrc for [f64] {
    fn to_re_vec(&self) -> Vec<f64> {
        self.to_vec()
    }
}
impl PolvalReSrc for Vec<RosyValue> {
    fn to_re_vec(&self) -> Vec<f64> {
        self.iter().map(|v| v.as_f64()).collect()
    }
}
impl PolvalReSrc for [RosyValue] {
    fn to_re_vec(&self) -> Vec<f64> {
        self.iter().map(|v| v.as_f64()).collect()
    }
}
impl PolvalReSrc for RosyValue {
    fn to_re_vec(&self) -> Vec<f64> {
        match self {
            RosyValue::VE(v) => v.clone(),
            RosyValue::Arr(v) => v.iter().map(|x| x.as_f64()).collect(),
            other => vec![other.as_f64()],
        }
    }
}

pub trait PolvalReDst {
    fn load_re_vec(&self) -> Vec<f64>;
    fn store_re_vec(&mut self, v: Vec<f64>);
}
impl PolvalReDst for Vec<f64> {
    fn load_re_vec(&self) -> Vec<f64> {
        self.clone()
    }
    fn store_re_vec(&mut self, v: Vec<f64>) {
        *self = v;
    }
}
impl PolvalReDst for Vec<RosyValue> {
    fn load_re_vec(&self) -> Vec<f64> {
        self.iter().map(|x| x.as_f64()).collect()
    }
    fn store_re_vec(&mut self, v: Vec<f64>) {
        if v.len() > self.len() {
            self.resize(v.len(), RosyValue::RE(0.0));
        }
        for (i, x) in v.into_iter().enumerate() {
            self[i] = RosyValue::RE(x);
        }
    }
}
impl PolvalReDst for &mut Vec<RosyValue> {
    fn load_re_vec(&self) -> Vec<f64> {
        self.iter().map(|x| x.as_f64()).collect()
    }
    fn store_re_vec(&mut self, v: Vec<f64>) {
        **self = v.into_iter().map(RosyValue::RE).collect();
    }
}
impl PolvalReDst for RosyValue {
    fn load_re_vec(&self) -> Vec<f64> {
        self.to_re_vec()
    }
    fn store_re_vec(&mut self, v: Vec<f64>) {
        *self = RosyValue::Arr(v.into_iter().map(RosyValue::RE).collect());
    }
}

pub trait AsDaDst {
    fn load_da_vec(&self) -> Vec<DA>;
    fn store_da_vec(&mut self, v: Vec<DA>);
}
impl AsDaDst for Vec<DA> {
    fn load_da_vec(&self) -> Vec<DA> {
        self.clone()
    }
    fn store_da_vec(&mut self, v: Vec<DA>) {
        *self = v;
    }
}
impl AsDaDst for Vec<RosyValue> {
    fn load_da_vec(&self) -> Vec<DA> {
        self.as_da_vec()
    }
    fn store_da_vec(&mut self, v: Vec<DA>) {
        if v.len() > self.len() {
            self.resize(v.len(), RosyValue::RE(0.0));
        }
        for (i, x) in v.into_iter().enumerate() {
            self[i] = RosyValue::DA(x);
        }
    }
}
impl AsDaDst for f64 {
    fn load_da_vec(&self) -> Vec<DA> {
        Vec::new()
    }
    fn store_da_vec(&mut self, _v: Vec<DA>) {}
}
impl AsDaDst for RosyValue {
    fn load_da_vec(&self) -> Vec<DA> {
        self.as_da_vec()
    }
    fn store_da_vec(&mut self, v: Vec<DA>) {
        if v.len() == 1 {
            *self = RosyValue::DA(v.into_iter().next().unwrap());
        } else {
            *self = RosyValue::Arr(v.into_iter().map(RosyValue::DA).collect());
        }
    }
}

pub trait AsDaRef {
    fn as_da_vec(&self) -> Vec<DA>;
}
impl AsDaRef for Vec<DA> {
    fn as_da_vec(&self) -> Vec<DA> {
        self.clone()
    }
}
impl AsDaRef for DA {
    fn as_da_vec(&self) -> Vec<DA> {
        vec![self.clone()]
    }
}
impl AsDaRef for f64 {
    fn as_da_vec(&self) -> Vec<DA> {
        vec![DA::constant(*self)]
    }
}
impl AsDaRef for RosyValue {
    fn as_da_vec(&self) -> Vec<DA> {
        match self {
            RosyValue::DA(d) => vec![d.clone()],
            RosyValue::Arr(v) => v
                .iter()
                .map(|x| x.clone().expect_da().unwrap_or_else(|_| DA::zero()))
                .collect(),
            other => vec![DA::constant(other.as_f64())],
        }
    }
}
impl AsDaRef for Vec<RosyValue> {
    fn as_da_vec(&self) -> Vec<DA> {
        self.iter()
            .map(|v| v.clone().expect_da().unwrap_or_else(|_| DA::zero()))
            .collect()
    }
}
impl AsDaRef for [RosyValue] {
    fn as_da_vec(&self) -> Vec<DA> {
        self.iter()
            .map(|v| v.clone().expect_da().unwrap_or_else(|_| DA::zero()))
            .collect()
    }
}

pub trait AsCdRef {
    fn as_cd_vec(&self) -> Vec<CD>;
}
impl AsCdRef for DA {
    fn as_cd_vec(&self) -> Vec<CD> {
        vec![CD::from_da(self)]
    }
}
impl AsCdRef for CD {
    fn as_cd_vec(&self) -> Vec<CD> {
        vec![self.clone()]
    }
}
impl AsCdRef for Vec<CD> {
    fn as_cd_vec(&self) -> Vec<CD> {
        self.clone()
    }
}
impl AsCdRef for [CD] {
    fn as_cd_vec(&self) -> Vec<CD> {
        self.to_vec()
    }
}
impl AsCdRef for RosyValue {
    fn as_cd_vec(&self) -> Vec<CD> {
        match self {
            RosyValue::CD(d) => vec![d.clone()],
            RosyValue::DA(d) => vec![CD::from_da(d)],
            RosyValue::Arr(v) => v.iter().map(|x| x.as_cd()).collect(),
            other => vec![other.as_cd()],
        }
    }
}
impl AsCdRef for Vec<RosyValue> {
    fn as_cd_vec(&self) -> Vec<CD> {
        self.iter().map(|x| x.as_cd()).collect()
    }
}
pub trait AsCdDst {
    fn load_cd_vec(&self) -> Vec<CD>;
    fn store_cd_vec(&mut self, v: Vec<CD>);
}
impl AsCdDst for Vec<CD> {
    fn load_cd_vec(&self) -> Vec<CD> {
        self.clone()
    }
    fn store_cd_vec(&mut self, v: Vec<CD>) {
        *self = v;
    }
}
impl AsCdDst for CD {
    fn load_cd_vec(&self) -> Vec<CD> {
        vec![self.clone()]
    }
    fn store_cd_vec(&mut self, v: Vec<CD>) {
        *self = v.into_iter().next().unwrap_or_else(CD::zero);
    }
}
impl AsCdDst for f64 {
    fn load_cd_vec(&self) -> Vec<CD> {
        Vec::new()
    }
    fn store_cd_vec(&mut self, _v: Vec<CD>) {}
}
impl AsCdDst for Vec<f64> {
    fn load_cd_vec(&self) -> Vec<CD> {
        Vec::new()
    }
    fn store_cd_vec(&mut self, _v: Vec<CD>) {}
}

pub fn rosy_velget(v: &impl PolvalReSrc, idx: impl IntoF64) -> anyhow::Result<f64> {
    let src = v.to_re_vec();
    let i = rosy_as_usize(&idx.into_f64());
    if i < 1 || i > src.len() {
        return Ok(0.0);
    }
    Ok(src[i - 1])
}

pub fn rosy_vezero(v: &mut impl PolvalReDst, n: impl IntoF64, thresh: impl AsF64) {
    let n = rosy_as_usize(&n.into_f64());
    let thresh = thresh.as_f64_val().abs();
    let mut src = v.load_re_vec();
    for x in src.iter_mut().take(n) {
        if x.abs() > thresh {
            *x = 0.0;
        }
    }
    v.store_re_vec(src);
}

pub fn rosy_veunit(v: &impl PolvalReSrc) -> Vec<f64> {
    let src = v.to_re_vec();
    let norm = src.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm == 0.0 {
        src
    } else {
        src.iter().map(|x| x / norm).collect()
    }
}

pub trait AsReMat {
    fn to_re_mat(&self) -> Vec<Vec<f64>>;
}
impl AsReMat for Vec<Vec<f64>> {
    fn to_re_mat(&self) -> Vec<Vec<f64>> {
        self.clone()
    }
}
impl AsReMat for Vec<Vec<RosyValue>> {
    fn to_re_mat(&self) -> Vec<Vec<f64>> {
        self.iter()
            .map(|row| row.iter().map(|x| x.as_f64()).collect())
            .collect()
    }
}
impl AsReMat for RosyValue {
    fn to_re_mat(&self) -> Vec<Vec<f64>> {
        match self {
            RosyValue::Arr(rows) => rows
                .iter()
                .map(|row| match row {
                    RosyValue::Arr(v) => v.iter().map(|x| x.as_f64()).collect(),
                    other => vec![other.as_f64()],
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}
impl AsCdDst for Vec<RosyValue> {
    fn load_cd_vec(&self) -> Vec<CD> {
        self.as_cd_vec()
    }
    fn store_cd_vec(&mut self, v: Vec<CD>) {
        *self = v.into_iter().map(RosyValue::CD).collect();
    }
}
impl AsCdRef for Vec<f64> {
    fn as_cd_vec(&self) -> Vec<CD> {
        Vec::new()
    }
}
impl AsCdDst for RosyValue {
    fn load_cd_vec(&self) -> Vec<CD> {
        self.as_cd_vec()
    }
    fn store_cd_vec(&mut self, v: Vec<CD>) {
        if v.len() == 1 {
            *self = RosyValue::CD(v.into_iter().next().unwrap());
        } else {
            *self = RosyValue::Arr(v.into_iter().map(RosyValue::CD).collect());
        }
    }
}

pub trait RosyIndexable {
    type Out: Clone;
    fn rosy_index(&self, idx: usize, name: &str) -> Self::Out;
}

impl<T: Clone + Default> RosyIndexable for Vec<T> {
    type Out = T;
    fn rosy_index(&self, idx: usize, _name: &str) -> T {
        self.get(idx.wrapping_sub(1)).cloned().unwrap_or_default()
    }
}

impl<T: Clone + Default> RosyIndexable for [T] {
    type Out = T;
    fn rosy_index(&self, idx: usize, _name: &str) -> T {
        self.get(idx.wrapping_sub(1)).cloned().unwrap_or_default()
    }
}

impl RosyIndexable for f64 {
    type Out = f64;
    fn rosy_index(&self, idx: usize, _name: &str) -> f64 {
        if idx == 1 {
            *self
        } else {
            0.0
        }
    }
}

impl<T: RosyIndexable + ?Sized> RosyIndexable for &T {
    type Out = T::Out;
    fn rosy_index(&self, idx: usize, name: &str) -> T::Out {
        (**self).rosy_index(idx, name)
    }
}

impl<T: RosyIndexable + ?Sized> RosyIndexable for &mut T {
    type Out = T::Out;
    fn rosy_index(&self, idx: usize, name: &str) -> T::Out {
        (**self).rosy_index(idx, name)
    }
}

impl RosyIndexable for RosyValue {
    type Out = RosyValue;
    fn rosy_index(&self, idx: usize, name: &str) -> RosyValue {
        match self {
            RosyValue::Arr(v) => v.rosy_index(idx, name),
            RosyValue::VE(v) => RosyValue::RE(v.rosy_index(idx, name)),
            other if idx == 1 => other.clone(),
            _ => RosyValue::RE(0.0),
        }
    }
}

#[inline(always)]
pub fn rosy_get<C: RosyIndexable + ?Sized>(
    container: &C,
    one_based: impl IntoF64,
    var_name: &str,
) -> C::Out {
    container.rosy_index(one_based.into_f64().round() as usize, var_name)
}

pub trait RosyMutIndexable {
    type Out: Default;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut Self::Out;
}

impl<T: Default> RosyMutIndexable for Vec<T> {
    type Out = T;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut T {
        if idx == 0 {
            panic!("Index 0 into '{name}' is out of bounds — Rosy uses 1-based indexing");
        }
        if idx > self.len() {
            self.resize_with(idx, T::default);
        }
        &mut self[idx - 1]
    }
}
impl<T: Default> RosyMutIndexable for &mut Vec<T> {
    type Out = T;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut T {
        (**self).rosy_index_mut(idx, name)
    }
}
impl RosyMutIndexable for &mut RosyValue {
    type Out = RosyValue;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut RosyValue {
        (**self).rosy_index_mut(idx, name)
    }
}

impl RosyMutIndexable for f64 {
    type Out = f64;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut f64 {
        if idx != 1 {
            panic!("Index {idx} into scalar '{name}'");
        }
        self
    }
}

impl RosyMutIndexable for RosyValue {
    type Out = RosyValue;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut RosyValue {
        if idx == 0 {
            panic!("Index 0 into '{name}' is out of bounds — Rosy uses 1-based indexing");
        }
        if let RosyValue::VE(v) = self {
            let arr = std::mem::take(v)
                .into_iter()
                .map(RosyValue::RE)
                .collect();
            *self = RosyValue::Arr(arr);
        }
        if !matches!(self, RosyValue::Arr(_)) && idx != 1 {
            let old = std::mem::replace(self, RosyValue::RE(0.0));
            *self = RosyValue::Arr(vec![old]);
        }
        match self {
            RosyValue::Arr(v) => {
                if idx > v.len() {
                    v.resize(idx, RosyValue::RE(0.0));
                }
                &mut v[idx - 1]
            }
            other if idx == 1 => other,
            other => panic!(
                "cannot mutably index {} '{}' at {}",
                other.kind_name(),
                name,
                idx
            ),
        }
    }
}

#[inline(always)]
pub fn rosy_get_mut<'a, C: RosyMutIndexable + ?Sized>(
    container: &'a mut C,
    one_based: impl IntoF64,
    var_name: &str,
) -> &'a mut C::Out {
    container.rosy_index_mut(one_based.into_f64().round() as usize, var_name)
}

pub type RE = f64;
pub type ST = String;
pub type LO = bool;
pub type CM = num_complex::Complex64;
pub type VE = Vec<f64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RosyType {
    pub base_type: RosyBaseType,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RosyBaseType {
    RE,
    ST,
    LO,
    CM,
    VE,
    DA,
    CD,
    ANY,
}
impl std::fmt::Display for RosyBaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RosyBaseType::RE => write!(f, "RE"),
            RosyBaseType::ST => write!(f, "ST"),
            RosyBaseType::LO => write!(f, "LO"),
            RosyBaseType::CM => write!(f, "CM"),
            RosyBaseType::VE => write!(f, "VE"),
            RosyBaseType::DA => write!(f, "DA"),
            RosyBaseType::CD => write!(f, "CD"),
            RosyBaseType::ANY => write!(f, "ANY"),
        }
    }
}
impl std::fmt::Display for RosyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.dimensions == 0 {
            write!(f, "({})", self.base_type)
        } else {
            write!(f, "({} {}D)", self.base_type, self.dimensions)
        }
    }
}
impl RosyType {
    pub const fn new(base_type: RosyBaseType, dimensions: usize) -> Self {
        RosyType { base_type, dimensions }
    }

    #[allow(non_snake_case)]
    pub const fn RE() -> Self { Self::new(RosyBaseType::RE, 0) }
    #[allow(non_snake_case)]
    pub const fn ST() -> Self { Self::new(RosyBaseType::ST, 0) }
    #[allow(non_snake_case)]
    pub const fn LO() -> Self { Self::new(RosyBaseType::LO, 0) }
    #[allow(non_snake_case)]
    pub const fn CM() -> Self { Self::new(RosyBaseType::CM, 0) }
    #[allow(non_snake_case)]
    pub const fn VE() -> Self { Self::new(RosyBaseType::VE, 0) }
    #[allow(non_snake_case)]
    pub const fn DA() -> Self { Self::new(RosyBaseType::DA, 0) }
    #[allow(non_snake_case)]
    pub const fn CD() -> Self { Self::new(RosyBaseType::CD, 0) }
    #[allow(non_snake_case)]
    pub const fn ANY() -> Self { Self::new(RosyBaseType::ANY, 0) }

    pub fn is_any(&self) -> bool {
        self.base_type == RosyBaseType::ANY
    }

    /// Returns true if this type implements Copy in Rust (cheap to duplicate).
    /// RE (f64), LO (bool), CM (Complex64) are Copy at dimension 0.
    /// All array types (dimensions > 0) are non-Copy (Vec<...>).
    pub fn is_copy(&self) -> bool {
        if self.dimensions > 0 {
            return false; // arrays are Vec<...>, not Copy
        }
        matches!(
            self.base_type,
            RosyBaseType::RE | RosyBaseType::LO | RosyBaseType::CM
        )
    }

    pub fn as_rust_type(&self) -> String {
        let base = match self.base_type {
            RosyBaseType::RE => "f64",
            RosyBaseType::ST => "String",
            RosyBaseType::LO => "bool",
            RosyBaseType::CM => "num_complex::Complex64",
            RosyBaseType::VE => "Vec<f64>",
            RosyBaseType::DA => "DA",
            RosyBaseType::CD => "CD",
            RosyBaseType::ANY => "RosyValue",
        }
        .to_string();

        if self.dimensions == 0 {
            base
        } else {
            let mut result = base;
            for _ in 0..self.dimensions {
                result = format!("Vec<{}>", result);
            }
            result
        }
    }
}
impl TryFrom<&str> for RosyBaseType {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<RosyBaseType, Self::Error> {
        match value {
            "RE" => Ok(RosyBaseType::RE),
            "ST" => Ok(RosyBaseType::ST),
            "LO" => Ok(RosyBaseType::LO),
            "CM" => Ok(RosyBaseType::CM),
            "VE" => Ok(RosyBaseType::VE),
            "DA" => Ok(RosyBaseType::DA),
            "CD" => Ok(RosyBaseType::CD),
            _ => Err(anyhow::anyhow!("Can't convert {} to a Rosy type", value)),
        }
    }
}

#[cfg(test)]
mod cd_coerce_tests {
    use super::*;

    #[serial_test::serial]
    #[test]
    fn da_cell_is_visible_to_cdnfda() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(3, 4)?;
        let da = DA::variable(1)?;
        let cell = RosyValue::DA(da.clone());
        let cds = cell.as_cd_vec();
        assert_eq!(cds.len(), 1);
        assert!(cds[0].constant_part().norm() == 0.0);

        let mut dst = RosyValue::RE(0.0);
        dst.store_cd_vec(vec![CD::from_da(&da)]);
        assert!(matches!(dst, RosyValue::CD(_)));

        crate::taylor::cleanup_taylor();
        Ok(())
    }
}
