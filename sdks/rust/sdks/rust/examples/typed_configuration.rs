//! Typed workspace configuration: get and set structured config.
//!
//! Demonstrates reading and writing workspace-level configuration
//! using the typed `WorkspaceConfiguration` struct.
//!
//! Run with `cargo run --example typed_configuration`

use honcho_ai::types::workspace::{
    DreamConfiguration, ReasoningConfiguration, SummaryConfiguration, WorkspaceConfiguration,
};
use honcho_ai::Honcho;

#[tokio::main]
async fn main() -> honcho_ai::error::Result<()> {
    let honcho = Honcho::from_params(
        Honcho::builder()
            .base_url("http://localhost:8000")
            .workspace_id("config-demo")
            .build(),
    )?;

    let config = honcho.get_configuration().await?;
    println!("Current config: {config:#?}");

    if let Some(ref reasoning) = config.reasoning {
        println!("Reasoning enabled: {:?}", reasoning.enabled);
    }

    let new_config = WorkspaceConfiguration {
        reasoning: Some(ReasoningConfiguration {
            enabled: Some(true),
            custom_instructions: Some("Focus on user preferences".into()),
        }),
        summary: Some(SummaryConfiguration {
            enabled: Some(true),
            messages_per_short_summary: Some(20),
            messages_per_long_summary: Some(60),
        }),
        dream: Some(DreamConfiguration {
            enabled: Some(true),
        }),
        peer_card: None,
    };

    honcho.set_configuration(&new_config).await?;
    println!("Configuration updated");

    let updated = honcho.get_configuration().await?;
    println!(
        "Reasoning custom instructions: {:?}",
        updated.reasoning.as_ref().and_then(|r| r.custom_instructions.as_ref())
    );

    let raw = honcho.get_configuration_raw().await?;
    println!("Raw config keys: {:?}", raw.keys().collect::<Vec<_>>());

    Ok(())
}
