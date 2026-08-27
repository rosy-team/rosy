//! # Parser & AST Infrastructure
//!
//! PEG grammar integration via [pest](https://pest.rs) and the Pratt parser
//! for expression precedence.
//!
//! ## Grammar
//!
//! The PEG grammar lives at `rosy-compiler/assets/rosy.pest` and defines all Rosy
//! language constructs. The generated [`CosyParser`] provides parsing entry
//! points.
//!
//! ## Operator Precedence
//!
//! The [`PRATT_PARSER`] defines expression operator precedence (lowest → highest):
//!
//! | Priority | Operators |
//! |----------|-----------|
//! | 0 | `OR` |
//! | 1 | `AND` |
//! | 2 | `&` `=` `#` `<` `>` `<=` `>=` |
//! | 3 | `+` `-` |
//! | 4 | `*` `/` |
//! | 5 | `^` (right-assoc) |
//! | 6 | `\|` `%` |
//!
//! ## FromRule Trait
//!
//! All AST nodes implement [`FromRule`] to construct themselves from a pest
//! parse pair.

use crate::program::expressions::Expr;
use crate::program::syntax_config;
use anyhow::{Context, Result, ensure};
use pest::iterators::Pairs;
use pest::pratt_parser::PrattParser;
use pest::Parser as _;
use pest_derive::Parser;
use rosy_lib::{RosyBaseType, RosyType};

#[derive(Parser)]
#[grammar = "../assets/rosy.pest"]
pub struct CosyParser;

/// Rosy sources must be `BEGIN…END`. COSY mains and INCLUDE bodies are fragments.
pub fn parse_source(source: &str) -> Result<Pairs<'_, Rule>, pest::error::Error<Rule>> {
    let rule = if syntax_config::is_cosy_syntax() {
        Rule::fragment
    } else {
        Rule::program
    };
    CosyParser::parse(rule, source)
}

pub fn parse_include(source: &str) -> Result<Pairs<'_, Rule>, pest::error::Error<Rule>> {
    CosyParser::parse(Rule::fragment, source)
}

// Create a static PrattParser for expressions
pub static PRATT_PARSER: std::sync::LazyLock<PrattParser<Rule>> = std::sync::LazyLock::new(|| {
    use Rule::*;
    use pest::pratt_parser::{Assoc::*, Op};

    // Precedence is defined from lowest to highest priority
    // Following COSY INFINITY priorities:
    // - Priority 2: Concatenation (&), Equality (=), Not-Equals (#), Less/Greater, comparison
    // - Priority 3: Addition (+), Subtraction (-)
    // - Priority 4: Multiplication (*), Division (/)
    // - Priority 5: Exponentiation (^) - right-associative
    // - Priority 6: Extraction (|), Derivation (%)
    PrattParser::new()
        // Lowest precedence (Priority 0): logical OR
        .op(Op::infix(or_op, Left))
        // Priority 1: logical AND (binds tighter than OR)
        .op(Op::infix(and_op, Left))
        // Priority 2: concatenation, equality, not-equals, comparisons
        .op(Op::infix(concat, Left)
            | Op::infix(eq, Left)
            | Op::infix(neq, Left)
            | Op::infix(lt, Left)
            | Op::infix(gt, Left)
            | Op::infix(lte, Left)
            | Op::infix(gte, Left))
        // Priority 3: Addition and Subtraction
        .op(Op::infix(add, Left) | Op::infix(sub, Left))
        // Priority 4: Multiplication and Division
        .op(Op::infix(mult, Left) | Op::infix(div, Left))
        // Priority 5: Exponentiation (right-associative, like math convention)
        .op(Op::infix(pow, Right))
        // Priority 6: Extraction (|) and Derivation (%)
        .op(Op::infix(extract, Left) | Op::infix(derive, Left))
});

pub trait FromRule: Sized {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>>;
}
// helper to build RosyType from type rule
pub fn build_type(pair: pest::iterators::Pair<Rule>) -> Result<(RosyType, Vec<Expr>)> {
    ensure!(
        pair.as_rule() == Rule::r#type,
        "Expected `type` rule when building type, found: {:?}",
        pair.as_rule()
    );

    let mut inner_pair = pair.into_inner();
    let type_str = inner_pair
        .next()
        .context("Missing type string when building var decl!")?
        .as_str()
        .to_string();
    let mut dimensions: Vec<Expr> = Vec::new();
    for dim_pair in inner_pair {
        let expr = Expr::from_rule(dim_pair)
            .context("Failed to build dimension expression in variable declaration!")?
            .ok_or_else(|| anyhow::anyhow!("Expected expression in variable declaration"))?;
        dimensions.push(expr);
    }

    let base_type: RosyBaseType = type_str
        .as_str()
        .try_into()
        .with_context(|| format!("Unknown type: {type_str}"))?;
    let r#type = RosyType::new(base_type, dimensions.len());

    Ok((r#type, dimensions))
}

#[cfg(test)]
mod intrinsic_name_sync {
    fn pest_intrinsic_names() -> Vec<String> {
        let pest = include_str!("../../assets/rosy.pest");
        let mut names = Vec::new();
        for line in pest.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("intrinsic_name") {
                continue;
            }
            for part in trimmed.split('|') {
                if let Some(start) = part.find('"') {
                    let rest = &part[start + 1..];
                    if let Some(end) = rest.find('"') {
                        let kw = rest[..end].to_ascii_uppercase();
                        if kw.chars().all(|c| c.is_ascii_alphanumeric()) {
                            names.push(kw);
                        }
                    }
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn pest_intrinsic_names_match_registry_plus_constructors() {
        let mut expected: Vec<String> = rosy_lib::INTRINSICS
            .iter()
            .map(|i| i.name.to_string())
            .collect();
        expected.extend(["DA".into(), "CD".into()]);
        expected.sort();
        expected.dedup();

        let pest = pest_intrinsic_names();
        assert_eq!(
            pest, expected,
            "build.rs should rewrite `intrinsic_name` from rosy_lib::INTRINSICS"
        );
    }
}
