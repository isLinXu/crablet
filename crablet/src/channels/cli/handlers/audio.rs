use crate::channels::cli::args::AudioSubcommands;
use crate::cognitive::multimodal::audio::AudioTool;
use anyhow::Result;
use tracing::info;

pub async fn handle_audio(subcmd: &AudioSubcommands) -> Result<()> {
    match subcmd {
        AudioSubcommands::Transcribe { path } => {
            info!("Transcribing audio: {}", path);
            let processor = AudioTool::new()?;
            let text = processor.transcribe(path).await?;
            println!("Transcription:\n{}", text);
        }
        AudioSubcommands::Speak { text, output } => {
            info!("Generating speech...");
            let processor = AudioTool::new()?;
            processor.speak(text, output).await?;
            println!("Speech saved to {}", output);
        }
    }
    Ok(())
}
