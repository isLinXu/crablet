use crate::channels::cli::args::VisionSubcommands;
use crate::cognitive::multimodal::image::ImageProcessor;
use anyhow::Result;
use tracing::info;

pub async fn handle_vision(subcmd: &VisionSubcommands) -> Result<()> {
    match subcmd {
        VisionSubcommands::Describe { path } => {
            info!("Analyzing image: {}", path);
            let processor = ImageProcessor::new()?;
            let description = processor.describe(path).await?;
            println!("Description: {}", description);
        }
    }
    Ok(())
}
