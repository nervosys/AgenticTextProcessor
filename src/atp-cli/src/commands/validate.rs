//! Validate command — check patterns, expressions, or pipelines without executing.

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use atp_core::output::Format;

#[derive(Args)]
pub struct ValidateArgs {
    /// Type of input to validate: pattern, expression, pipeline
    #[arg(long, short = 't', default_value = "pattern")]
    pub input_type: String,

    /// The input string to validate
    pub input: String,
}

#[derive(Debug, Serialize)]
struct ValidationResult {
    valid: bool,
    input_type: String,
    input: String,
    error: Option<String>,
    normalized: Option<String>,
}

pub fn execute(args: ValidateArgs, format: Format) -> Result<()> {
    let result = match args.input_type.as_str() {
        "pattern" | "regex" => validate_pattern(&args.input),
        "expression" | "expr" => validate_expression(&args.input),
        "pipeline" | "pipe" => validate_pipeline(&args.input),
        "aql" | "query" => validate_aql(&args.input),
        other => ValidationResult {
            valid: false,
            input_type: other.to_string(),
            input: args.input.clone(),
            error: Some(format!(
                "Unknown input type: '{other}'. Use: pattern, expression, pipeline, aql"
            )),
            normalized: None,
        },
    };

    match format {
        Format::Human => {
            if result.valid {
                println!("✓ Valid {} : {}", result.input_type, result.input);
                if let Some(ref norm) = result.normalized {
                    println!("  Normalized: {norm}");
                }
            } else {
                println!("✗ Invalid {} : {}", result.input_type, result.input);
                if let Some(ref err) = result.error {
                    println!("  Error: {err}");
                }
            }
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    if !result.valid {
        std::process::exit(1);
    }

    Ok(())
}

fn validate_pattern(pattern: &str) -> ValidationResult {
    match regex::Regex::new(pattern) {
        Ok(re) => ValidationResult {
            valid: true,
            input_type: "pattern".to_string(),
            input: pattern.to_string(),
            error: None,
            normalized: Some(re.as_str().to_string()),
        },
        Err(e) => ValidationResult {
            valid: false,
            input_type: "pattern".to_string(),
            input: pattern.to_string(),
            error: Some(e.to_string()),
            normalized: None,
        },
    }
}

fn validate_expression(expr: &str) -> ValidationResult {
    match atp_core::engine::sed::parse_substitution(expr) {
        Ok(_) => ValidationResult {
            valid: true,
            input_type: "expression".to_string(),
            input: expr.to_string(),
            error: None,
            normalized: Some(expr.to_string()),
        },
        Err(e) => ValidationResult {
            valid: false,
            input_type: "expression".to_string(),
            input: expr.to_string(),
            error: Some(e.to_string()),
            normalized: None,
        },
    }
}

fn validate_pipeline(dsl: &str) -> ValidationResult {
    match atp_core::engine::pipeline::Pipeline::from_dsl(dsl) {
        Ok(pipe) => {
            let explain = pipe.explain();
            ValidationResult {
                valid: true,
                input_type: "pipeline".to_string(),
                input: dsl.to_string(),
                error: None,
                normalized: Some(explain.join(" → ")),
            }
        }
        Err(e) => ValidationResult {
            valid: false,
            input_type: "pipeline".to_string(),
            input: dsl.to_string(),
            error: Some(e.to_string()),
            normalized: None,
        },
    }
}

fn validate_aql(query: &str) -> ValidationResult {
    match atp_core::engine::aql::parse(query) {
        Ok(pipeline) => {
            let explain = pipeline.explain();
            ValidationResult {
                valid: true,
                input_type: "aql".to_string(),
                input: query.to_string(),
                error: None,
                normalized: Some(explain.join(" | ")),
            }
        }
        Err(e) => ValidationResult {
            valid: false,
            input_type: "aql".to_string(),
            input: query.to_string(),
            error: Some(e.to_string()),
            normalized: None,
        },
    }
}
