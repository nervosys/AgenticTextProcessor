//! CLI command: `atp ai` — AI-assisted text processing.

use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct AiArgs {
    /// Natural language query to convert to AQL
    #[arg(long, short = 'n')]
    pub nl: Option<String>,

    /// Explain an AQL query in natural language
    #[arg(long, short = 'e')]
    pub explain: Option<String>,

    /// Suggest an AQL query for a goal (reads stdin for sample data)
    #[arg(long, short = 's')]
    pub suggest: Option<String>,

    /// Semantic search — describe what to find in natural language
    #[arg(long)]
    pub semantic: Option<String>,

    /// File or path to process
    pub path: Option<String>,

    /// AI backend: ollama or openai
    #[arg(long, default_value = "ollama")]
    pub backend: String,

    /// Model name
    #[arg(long)]
    pub model: Option<String>,

    /// Execute the generated AQL query instead of just printing it
    #[arg(long, short = 'x')]
    pub execute: bool,
}

pub fn execute(args: AiArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { execute_async(args).await })
}

async fn execute_async(args: AiArgs) -> Result<()> {
    use atp_core::ai::{AiBackend, AiConfig, AiEngine};

    let config = AiConfig {
        backend: match args.backend.as_str() {
            "openai" => AiBackend::OpenAi,
            _ => AiBackend::Ollama,
        },
        model: args
            .model
            .clone()
            .unwrap_or_else(|| AiConfig::default().model),
        ..AiConfig::default()
    };

    let engine = AiEngine::with_config(config);

    if let Some(nl) = &args.nl {
        let aql = engine.nl_to_aql(nl).await?;
        println!("{aql}");

        if args.execute {
            if let Some(path) = &args.path {
                let source = atp_core::async_io::DataSource::File { path: path.clone() };
                let results = atp_core::async_io::AsyncPipeline::execute(&aql, &source).await?;
                println!("{}", serde_json::to_string_pretty(&results.final_output)?);
            }
        }
    } else if let Some(query) = &args.explain {
        let explanation = engine.explain_aql(query).await?;
        println!("{explanation}");
    } else if let Some(goal) = &args.suggest {
        let mut sample = String::new();
        if let Some(path) = &args.path {
            sample = std::fs::read_to_string(path)?;
            // Truncate to first 50 lines for context
            sample = sample.lines().take(50).collect::<Vec<_>>().join("\n");
        } else {
            use std::io::Read;
            std::io::stdin().read_to_string(&mut sample)?;
            sample = sample.lines().take(50).collect::<Vec<_>>().join("\n");
        }
        let query = engine.suggest_query(&sample, goal).await?;
        println!("{query}");
    } else if let Some(desc) = &args.semantic {
        let content = if let Some(path) = &args.path {
            std::fs::read_to_string(path)?
        } else {
            let mut buf = String::new();
            use std::io::Read;
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        };
        let matches = engine.semantic_search(desc, &content).await?;
        for line in matches {
            println!("{line}");
        }
    } else {
        anyhow::bail!("Specify one of: --nl, --explain, --suggest, --semantic");
    }

    Ok(())
}
