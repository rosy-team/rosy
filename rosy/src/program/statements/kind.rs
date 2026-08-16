//! Closed statement AST. Replaces `Box<dyn TranspileableStatement>`.

use super::*;
use crate::transpile::{
    InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult,
};
use crate::resolve::{ScopeContext, TypeResolver};
use crate::program::statements::SourceLocation;
use anyhow::{Error, Result};

macro_rules! stmt_kind {
    ($($var:ident($ty:ty)),+ $(,)?) => {
        #[derive(Debug)]
        pub enum StmtKind {
            $($var($ty)),+
        }
        $(impl From<$ty> for StmtKind {
            fn from(s: $ty) -> Self { Self::$var(s) }
        })+
        impl Transpile for StmtKind {
            fn transpile(
                &self,
                context: &mut TranspilationInputContext,
            ) -> Result<TranspilationOutput, Vec<Error>> {
                match self { $(Self::$var(s) => s.transpile(context),)+ }
            }
        }
        impl TranspileableStatement for StmtKind {
            fn register_typeslot_declaration(
                &self,
                resolver: &mut TypeResolver,
                ctx: &mut ScopeContext,
                source_location: SourceLocation,
            ) -> TypeslotDeclarationResult {
                match self {
                    $(Self::$var(s) => s.register_typeslot_declaration(resolver, ctx, source_location),)+
                }
            }
            fn wire_inference_edges(
                &self,
                resolver: &mut TypeResolver,
                ctx: &mut ScopeContext,
                source_location: SourceLocation,
            ) -> InferenceEdgeResult {
                match self {
                    $(Self::$var(s) => s.wire_inference_edges(resolver, ctx, source_location),)+
                }
            }
            fn hydrate_resolved_types(
                &mut self,
                resolver: &TypeResolver,
                current_scope: &[String],
            ) -> TypeHydrationResult {
                match self {
                    $(Self::$var(s) => s.hydrate_resolved_types(resolver, current_scope),)+
                }
            }
        }
    };
}

stmt_kind! {
    Argget(ArggetStatement),
    Assign(AssignStatement),
    Backf(BackfStatement),
    Break(BreakStatement),
    Cdf2(Cdf2Statement),
    Cdflo(CdfloStatement),
    Cdnf(CdnfStatement),
    Cdnfda(CdnfdaStatement),
    Cdnfds(CdnfdsStatement),
    Closef(ClosefStatement),
    Cpolval(CpolvalStatement),
    Cpusec(CpusecStatement),
    DAInit(DAInitStatement),
    Dacliw(DacliwStatement),
    Dacode(DacodeStatement),
    Dacqlc(DacqlcStatement),
    Dader(DaderStatement),
    Dadiu(DadiuStatement),
    Dadmu(DadmuStatement),
    Daeps(DaepsStatement),
    Daepsm(DaepsmStatement),
    Daest(DaestStatement),
    Dafilt(DafiltStatement),
    Daflo(DafloStatement),
    Dafset(DafsetStatement),
    Dagmd(DagmdStatement),
    Daint(DaintStatement),
    Danoro(DanoroStatement),
    Danors(DanorsStatement),
    Danot(DanotStatement),
    Danotw(DanotwStatement),
    Danow(DanowStatement),
    Dapea(DapeaStatement),
    Dapee(DapeeStatement),
    Dapep(DapepStatement),
    Dapew(DapewStatement),
    Daplu(DapluStatement),
    Daprv(DaprvStatement),
    Daran(DaranStatement),
    Darea(DareaStatement),
    Darev(DarevStatement),
    Dascl(DasclStatement),
    Dasgn(DasgnStatement),
    Datrn(DatrnStatement),
    Epsmin(EpsminStatement),
    Fit(FitStatement),
    FunctionCall(FunctionCallStatement),
    Function(FunctionStatement),
    If(IfStatement),
    Imunit(ImunitStatement),
    Intpol(IntpolStatement),
    Ldet(LdetStatement),
    Lev(LevStatement),
    Lfalse(LfalseStatement),
    Linv(LinvStatement),
    Loop(LoopStatement),
    Lsline(LslineStatement),
    Ltrue(LtrueStatement),
    Mblock(MblockStatement),
    Memall(MemallStatement),
    Memdpv(MemdpvStatement),
    Memfre(MemfreStatement),
    Memwrt(MemwrtStatement),
    Mtree(MtreeStatement),
    Openf(OpenfStatement),
    Openfb(OpenfbStatement),
    OsCall(OsCallStatement),
    PLoop(PLoopStatement),
    Pnpro(PnproStatement),
    Polval(PolvalStatement),
    ProcedureCall(ProcedureCallStatement),
    Procedure(ProcedureStatement),
    Pwtime(PwtimeStatement),
    Quit(QuitStatement),
    Ranseed(RanseedStatement),
    Read(ReadStatement),
    Readb(ReadbStatement),
    Readm(ReadmStatement),
    Reads(ReadsStatement),
    Recst(RecstStatement),
    Reran(ReranStatement),
    Rewf(RewfStatement),
    Rkco(RkcoStatement),
    Save(SaveStatement),
    Scrlen(ScrlenStatement),
    Sleepm(SleepmStatement),
    Stcre(StcreStatement),
    Substr(SubstrStatement),
    VarDecl(VarDeclStatement),
    Vedot(VedotStatement),
    Velget(VelgetStatement),
    Velset(VelsetStatement),
    Veunit(VeunitStatement),
    Vezero(VezeroStatement),
    While(WhileStatement),
    Write(WriteStatement),
    Writeb(WritebStatement),
    Writem(WritemStatement),
}
