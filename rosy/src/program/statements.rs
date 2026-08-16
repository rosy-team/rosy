//! # Statements
//!
//! Everything in Rosy that *does* something — declarations, control flow, I/O,
//! and more.
//!
//! ## Looking for something?
//!
//! | I want to... | Go to |
//! |--------------|-------|
//! | Declare a variable (`VARIABLE`) | **[`core::var_decl`]** |
//! | Assign a value (`:=`) | **[`core::assign`]** |
//! | Use `IF` / `ELSEIF` / `ELSE` | **[`core::if`]** |
//! | Use `LOOP` or `WHILE` | **[`core::loop`]**, **[`core::while_loop`]** |
//! | Use `PLOOP` (MPI parallel loop) | **[`core::ploop`]** |
//! | Define a `FUNCTION` or `PROCEDURE` | **[`core::function`]**, **[`core::procedure`]** |
//! | Call a function or procedure | **[`core::function_call`]**, **[`core::procedure_call`]** |
//! | Print output (`WRITE`) | **[`io::write`]** |
//! | Read input (`READ`) | **[`io::read`]** |
//! | Open/close files | **[`io::openf`]**, **[`io::closef`]** |
//! | Use binary I/O | **[`io::writeb`]**, **[`io::readb`]** |
//! | Initialize DA (`OV`) | **[`da::da_init`]** |
//! | Print DA values | **[`da::daprv`]**, **[`da::darev`]** |
//! | Print DA by variable/order | **[`da::dapew`]** |
//! | Read DA from file | **[`da::darea`]** |
//! | Configure DA | **[`da::daeps`]**, **[`da::danot`]**, **[`da::datrn`]** |
//! | Scale / negate DA | **[`da::dascl`]**, **[`da::dasgn`]** |
//! | Differentiate / integrate DA | **[`da::dader`]**, **[`da::daint`]** |
//! | Filter DA terms | **[`da::danoro`]**, **[`da::danors`]** |
//! | Substitute variable in DA | **[`da::daplu`]** |
//! | DA division / shift | **[`da::dadiu`]**, **[`da::dadmu`]** |
//! | Extract DA coefficients | **[`da::dacliw`]**, **[`da::dacqlc`]**, **[`da::dapee`]**, **[`da::dapea`]**, **[`da::dapep`]** |
//! | Estimate DA term size | **[`da::daest`]** |
//! | DA tree evaluation | **[`da::mtree`]** |
//! | Use `FIT` (optimization) | **[`math::fit`]** |
//! | Use `BREAK` or `QUIT` | **[`core::break`]**, **[`core::quit`]** |
//! | Measure time | **[`io::cpusec`]**, **[`io::pwtime`]** |
//! | Run a shell command | **[`io::os_call`]** |
//! | Read vectors from files | **[`io::velget`]** |
//! | Extract a substring | **[`core::substr`]** |
//! | Parse string to number | **[`core::stcre`]** |
//! | Format number as string | **[`core::recst`]** |
//! | Set a vector component | **[`core::velset`]** |
//! | Get a random number | **[`core::reran`]** |
//! | Set RNG seed | **[`core::ranseed`]** |
//! | Get imaginary unit | **[`core::imunit`]** |
//! | Get process count | **[`core::pnpro`]** |
//! | Matrix operations | **[`math::linv`]**, **[`math::ldet`]**, **[`math::lev`]**, **[`math::mblock`]** |
//! | Polynomial evaluation | **[`math::polval`]** |
//! | Vector math | **[`math::vedot`]**, **[`math::veunit`]**, **[`math::vezero`]** |

pub mod core;
pub mod da;
pub mod io;
pub mod kind;
pub mod math;

pub use kind::StmtKind;

pub use core::assign::AssignStatement;
pub use core::r#break::BreakStatement;
pub use core::function::FunctionStatement;
pub use core::function_call::FunctionCallStatement;
pub use core::r#if::IfStatement;
pub use core::r#loop::LoopStatement;
pub use core::ploop::PLoopStatement;
pub use core::procedure::ProcedureStatement;
pub use core::procedure_call::ProcedureCallStatement;
pub use core::var_decl::{VarDeclStatement, VariableDeclarationData};
pub use core::while_loop::WhileStatement;

pub use da::da_init::DAInitStatement;
pub use da::daprv::DaprvStatement;
pub use da::darev::DarevStatement;

pub use io::backf::BackfStatement;
pub use io::closef::ClosefStatement;
pub use io::openf::OpenfStatement;
pub use io::openfb::OpenfbStatement;
pub use io::read::ReadStatement;
pub use io::readb::ReadbStatement;
pub use io::readm::ReadmStatement;
pub use io::reads::ReadsStatement;
pub use io::rewf::RewfStatement;
pub use io::save::SaveStatement;
pub use io::write::WriteStatement;
pub use io::writeb::WritebStatement;
pub use io::writem::WritemStatement;

pub use math::cpolval::CpolvalStatement;
pub use math::fit::FitStatement;
pub use math::intpol::IntpolStatement;
pub use math::ldet::LdetStatement;
pub use math::lev::LevStatement;
pub use math::linv::LinvStatement;
pub use math::lsline::LslineStatement;
pub use math::mblock::MblockStatement;
pub use math::polval::PolvalStatement;
pub use math::rkco::RkcoStatement;

pub use core::quit::QuitStatement;
pub use core::scrlen::ScrlenStatement;
pub use core::substr::SubstrStatement;
pub use core::velset::VelsetStatement;

pub use io::cpusec::CpusecStatement;
pub use io::os_call::OsCallStatement;
pub use io::pwtime::PwtimeStatement;
pub use io::velget::VelgetStatement;

pub use math::vedot::VedotStatement;
pub use math::veunit::VeunitStatement;
pub use math::vezero::VezeroStatement;

pub use core::argget::ArggetStatement;
pub use core::imunit::ImunitStatement;
pub use core::lfalse::LfalseStatement;
pub use core::ltrue::LtrueStatement;
pub use core::memall::MemallStatement;
pub use core::memdpv::MemdpvStatement;
pub use core::memfre::MemfreStatement;
pub use core::memwrt::MemwrtStatement;
pub use core::pnpro::PnproStatement;
pub use core::ranseed::RanseedStatement;
pub use core::recst::RecstStatement;
pub use core::reran::ReranStatement;
pub use core::sleepm::SleepmStatement;
pub use core::stcre::StcreStatement;

pub use da::cdf2::Cdf2Statement;
pub use da::cdflo::CdfloStatement;
pub use da::cdnf::CdnfStatement;
pub use da::cdnfda::CdnfdaStatement;
pub use da::cdnfds::CdnfdsStatement;
pub use da::dacliw::DacliwStatement;
pub use da::dacode::DacodeStatement;
pub use da::dacqlc::DacqlcStatement;
pub use da::dader::DaderStatement;
pub use da::dadiu::DadiuStatement;
pub use da::dadmu::DadmuStatement;
pub use da::daeps::DaepsStatement;
pub use da::daepsm::DaepsmStatement;
pub use da::daest::DaestStatement;
pub use da::dafilt::DafiltStatement;
pub use da::daflo::DafloStatement;
pub use da::dafset::DafsetStatement;
pub use da::dagmd::DagmdStatement;
pub use da::daint::DaintStatement;
pub use da::danoro::DanoroStatement;
pub use da::danors::DanorsStatement;
pub use da::danot::DanotStatement;
pub use da::danotw::DanotwStatement;
pub use da::danow::DanowStatement;
pub use da::dapea::DapeaStatement;
pub use da::dapee::DapeeStatement;
pub use da::dapep::DapepStatement;
pub use da::dapew::DapewStatement;
pub use da::daplu::DapluStatement;
pub use da::daran::DaranStatement;
pub use da::darea::DareaStatement;
pub use da::dascl::DasclStatement;
pub use da::dasgn::DasgnStatement;
pub use da::datrn::DatrnStatement;
pub use da::epsmin::EpsminStatement;
pub use da::mtree::MtreeStatement;

use crate::{
    ast::{FromRule, Rule},
    errors::WithLocation,
    resolve::*,
    transpile::*,
};
use anyhow::{Context, Error, Result, bail};

/// Source location captured from the pest parse span.
/// Used in error messages for diagnostics.
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub line: usize,
    pub col: usize,
    /// A short snippet of the source text (first line, truncated).
    pub snippet: String,
    /// The file this location came from (`None` for the root/main file).
    pub file: Option<std::path::PathBuf>,
}
impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref path) = self.file {
            write!(
                f,
                "{}:{}:{}: {}",
                path.display(),
                self.line,
                self.col,
                self.snippet
            )
        } else {
            write!(f, "line {}, col {}: {}", self.line, self.col, self.snippet)
        }
    }
}
impl SourceLocation {
    /// Build from a pest pair's span, before the pair is consumed.
    pub fn from_pair(pair: &pest::iterators::Pair<Rule>) -> Self {
        let span = pair.as_span();
        let (line, col) = span.start_pos().line_col();
        let text = span.as_str();
        // Take first line, truncate to 60 chars
        let first_line = text.lines().next().unwrap_or("");
        let snippet = if first_line.len() > 60 {
            format!("{}...", &first_line[..57])
        } else {
            first_line.to_string()
        };
        SourceLocation {
            line,
            col,
            snippet,
            file: None,
        }
    }

    /// Set the originating file path on this location.
    pub fn with_file(mut self, path: std::path::PathBuf) -> Self {
        self.file = Some(path);
        self
    }
}

#[derive(Debug)]
pub struct Statement {
    pub inner: StmtKind,
    pub source_location: SourceLocation,
}
impl TranspileableStatement for Statement {
    fn register_typeslot_declaration(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> TypeslotDeclarationResult {
        TypeslotDeclarationResult::NotAVarFuncOrProcedureDecl
    }
    fn wire_inference_edges(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        InferenceEdgeResult::NoEdges
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> TypeHydrationResult {
        TypeHydrationResult::NothingToHydrate
    }
}
impl FromRule for Statement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Statement>> {
        // Capture source location before the pair is consumed
        let loc = SourceLocation::from_pair(&pair);
        match pair.as_rule() {
            Rule::daini => DAInitStatement::from_rule(pair)
                .context("...while building DA initialization statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daprv => DaprvStatement::from_rule(pair)
                .context("...while building DAPRV statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::darev => DarevStatement::from_rule(pair)
                .context("...while building DAREV statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::var_decl => VarDeclStatement::from_rule(pair)
                .context("...while building variable declaration!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::write => WriteStatement::from_rule(pair)
                .context("...while building write statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::reads => ReadsStatement::from_rule(pair)
                .context("...while building READS statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::read => ReadStatement::from_rule(pair)
                .context("...while building read statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::writeb => WritebStatement::from_rule(pair)
                .context("...while building WRITEB statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::readb => ReadbStatement::from_rule(pair)
                .context("...while building READB statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::openf => OpenfStatement::from_rule(pair)
                .context("...while building OPENF statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::openfb => OpenfbStatement::from_rule(pair)
                .context("...while building OPENFB statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::closef => ClosefStatement::from_rule(pair)
                .context("...while building CLOSEF statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::rewf => RewfStatement::from_rule(pair)
                .context("...while building REWF statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::backf => BackfStatement::from_rule(pair)
                .context("...while building BACKF statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::assignment => AssignStatement::from_rule(pair)
                .context("...while building assignment statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::r#loop => LoopStatement::from_rule(pair)
                .context("...while building loop statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::while_loop => WhileStatement::from_rule(pair)
                .context("...while building while statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::ploop => PLoopStatement::from_rule(pair)
                .context("...while building ploop statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::procedure => ProcedureStatement::from_rule(pair)
                .context("...while building procedure declaration!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::procedure_call => ProcedureCallStatement::from_rule(pair)
                .context("...while building procedure call!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::function => FunctionStatement::from_rule(pair)
                .context("...while building function declaration!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::function_call => FunctionCallStatement::from_rule(pair)
                .context("...while building function call!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::if_statement => IfStatement::from_rule(pair)
                .context("...while building if statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::break_statement => BreakStatement::from_rule(pair)
                .context("...while building break statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::fit_statement => FitStatement::from_rule(pair)
                .context("...while building FIT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),

            Rule::scrlen => ScrlenStatement::from_rule(pair)
                .context("...while building SCRLEN statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::cpusec => CpusecStatement::from_rule(pair)
                .context("...while building CPUSEC statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::quit => QuitStatement::from_rule(pair)
                .context("...while building QUIT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::os_call => OsCallStatement::from_rule(pair)
                .context("...while building OS statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::danot => DanotStatement::from_rule(pair)
                .context("...while building DANOT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daeps => DaepsStatement::from_rule(pair)
                .context("...while building DAEPS statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::datrn => DatrnStatement::from_rule(pair)
                .context("...while building DATRN statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::linv => LinvStatement::from_rule(pair)
                .context("...while building LINV statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::ldet => LdetStatement::from_rule(pair)
                .context("...while building LDET statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::substr => SubstrStatement::from_rule(pair)
                .context("...while building SUBSTR statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::velset => VelsetStatement::from_rule(pair)
                .context("...while building VELSET statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::velget => VelgetStatement::from_rule(pair)
                .context("...while building VELGET statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::save_stmt => SaveStatement::from_rule(pair)
                .context("...while building SAVE statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::intpol => IntpolStatement::from_rule(pair)
                .context("...while building INTPOL statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::polval => PolvalStatement::from_rule(pair)
                .context("...while building POLVAL statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::cpolval => CpolvalStatement::from_rule(pair)
                .context("...while building CPOLVAL statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::vedot => VedotStatement::from_rule(pair)
                .context("...while building VEDOT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::veunit => VeunitStatement::from_rule(pair)
                .context("...while building VEUNIT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::vezero => VezeroStatement::from_rule(pair)
                .context("...while building VEZERO statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::stcre => StcreStatement::from_rule(pair)
                .context("...while building STCRE statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::recst => RecstStatement::from_rule(pair)
                .context("...while building RECST statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::reran => ReranStatement::from_rule(pair)
                .context("...while building RERAN statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::ranseed => RanseedStatement::from_rule(pair)
                .context("...while building RANSEED statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::pwtime => PwtimeStatement::from_rule(pair)
                .context("...while building PWTIME statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::pnpro => PnproStatement::from_rule(pair)
                .context("...while building PNPRO statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::imunit => ImunitStatement::from_rule(pair)
                .context("...while building IMUNIT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::lev => LevStatement::from_rule(pair)
                .context("...while building LEV statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::mblock => MblockStatement::from_rule(pair)
                .context("...while building MBLOCK statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::mtree => MtreeStatement::from_rule(pair)
                .context("...while building MTREE statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::lsline => LslineStatement::from_rule(pair)
                .context("...while building LSLINE statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::rkco => RkcoStatement::from_rule(pair)
                .context("...while building RKCO statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dascl => DasclStatement::from_rule(pair)
                .context("...while building DASCL statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dasgn => DasgnStatement::from_rule(pair)
                .context("...while building DASGN statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dader => DaderStatement::from_rule(pair)
                .context("...while building DADER statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daint => DaintStatement::from_rule(pair)
                .context("...while building DAINT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::danoro => DanoroStatement::from_rule(pair)
                .context("...while building DANORO statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::danors => DanorsStatement::from_rule(pair)
                .context("...while building DANORS statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daplu => DapluStatement::from_rule(pair)
                .context("...while building DAPLU statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dadiu => DadiuStatement::from_rule(pair)
                .context("...while building DADIU statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dadmu => DadmuStatement::from_rule(pair)
                .context("...while building DADMU statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dacliw => DacliwStatement::from_rule(pair)
                .context("...while building DACLIW statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dacqlc => DacqlcStatement::from_rule(pair)
                .context("...while building DACQLC statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::darea => DareaStatement::from_rule(pair)
                .context("...while building DAREA statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dapew => DapewStatement::from_rule(pair)
                .context("...while building DAPEW statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dapee => DapeeStatement::from_rule(pair)
                .context("...while building DAPEE statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dapea => DapeaStatement::from_rule(pair)
                .context("...while building DAPEA statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dapep => DapepStatement::from_rule(pair)
                .context("...while building DAPEP statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daest => DaestStatement::from_rule(pair)
                .context("...while building DAEST statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daepsm => DaepsmStatement::from_rule(pair)
                .context("...while building DAEPSM statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::epsmin => EpsminStatement::from_rule(pair)
                .context("...while building EPSMIN statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dafset => DafsetStatement::from_rule(pair)
                .context("...while building DAFSET statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dafilt => DafiltStatement::from_rule(pair)
                .context("...while building DAFILT statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::danotw => DanotwStatement::from_rule(pair)
                .context("...while building DANOTW statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daflo => DafloStatement::from_rule(pair)
                .context("...while building DAFLO statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::cdflo => CdfloStatement::from_rule(pair)
                .context("...while building CDFLO statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dagmd => DagmdStatement::from_rule(pair)
                .context("...while building DAGMD statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::danow => DanowStatement::from_rule(pair)
                .context("...while building DANOW statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::cdf2 => Cdf2Statement::from_rule(pair)
                .context("...while building CDF2 statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::cdnf => CdnfStatement::from_rule(pair)
                .context("...while building CDNF statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::cdnfda => CdnfdaStatement::from_rule(pair)
                .context("...while building CDNFDA statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::cdnfds => CdnfdsStatement::from_rule(pair)
                .context("...while building CDNFDS statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::daran => DaranStatement::from_rule(pair)
                .context("...while building DARAN statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::dacode => DacodeStatement::from_rule(pair)
                .context("...while building DACODE statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),

            Rule::sleepm => SleepmStatement::from_rule(pair)
                .context("...while building SLEEPM statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::argget => ArggetStatement::from_rule(pair)
                .context("...while building ARGGET statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::memdpv => MemdpvStatement::from_rule(pair)
                .context("...while building MEMDPV statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::memfre => MemfreStatement::from_rule(pair)
                .context("...while building MEMFRE statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::memall => MemallStatement::from_rule(pair)
                .context("...while building MEMALL statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::memwrt => MemwrtStatement::from_rule(pair)
                .context("...while building MEMWRT statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::ltrue => LtrueStatement::from_rule(pair)
                .context("...while building LTRUE statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::lfalse => LfalseStatement::from_rule(pair)
                .context("...while building LFALSE statement!")
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),

            Rule::readm => ReadmStatement::from_rule(pair)
                .context("...while building READM statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),
            Rule::writem => WritemStatement::from_rule(pair)
                .context("...while building WRITEM statement!")
                .with_location(&loc)
                .map(|opt| {
                    opt.map(|stmt| Statement {
                        inner: stmt.into(),
                        source_location: loc.clone(),
                    })
                }),

            // Ignored
            Rule::begin
            | Rule::end
            | Rule::EOI
            | Rule::end_procedure
            | Rule::end_function
            | Rule::end_loop
            | Rule::end_while
            | Rule::endif
            | Rule::end_fit => Ok(None),
            other => bail!("Unexpected statement: {:?}", other),
        }
    }
}
impl Transpile for Statement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        self.inner.transpile(context).map_err(|err_vec| {
            let loc = self.source_location.clone();
            err_vec
                .into_iter()
                .map(|err| {
                    // If the error already has a RosyError with a location
                    // (from an inner statement), preserve it and just add context.
                    // Otherwise, wrap the error with this statement's location.
                    let has_location = err.chain().any(|cause| {
                        cause
                            .downcast_ref::<crate::errors::RosyError>()
                            .is_some_and(|r| r.location.is_some())
                    });
                    if has_location {
                        err
                    } else {
                        let message = format!("{}", err.root_cause());
                        anyhow::Error::from(crate::errors::RosyError {
                            message,
                            location: Some(loc.clone()),
                            severity: crate::errors::RosyErrorSeverity::Error,
                        })
                    }
                })
                .collect()
        })
    }
}
