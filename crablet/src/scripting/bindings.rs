use mlua::{Lua, Result, ExternalError};
use crate::tools::bash::BashTool;
use crate::tools::file::FileTool;
use crate::tools::search::WebSearchTool;
use crate::cognitive::llm::{LlmClient, OpenAiClient};
use crate::types::Message;
use crate::cognitive::multimodal::image::ImageProcessor;
#[cfg(feature = "audio")]
use crate::cognitive::multimodal::audio::AudioTool;
#[cfg(feature = "knowledge")]
use crate::knowledge::extractor::KnowledgeExtractor;

pub fn register_bindings(lua: &Lua) -> Result<()> {
    let globals = lua.globals();
    
    // Create 'crablet' namespace
    let crablet = lua.create_table()?;
    
    // Bind 'run_command'
    crablet.set("run_command", lua.create_function(|_, cmd: String| {
        match BashTool::execute(&cmd) {
            Ok(output) => Ok(output),
            Err(e) => Ok(format!("Error: {}", e)),
        }
    })?)?;

    // Bind 'read_file'
    crablet.set("read_file", lua.create_function(|_, path: String| {
        match FileTool::read(&path) {
            Ok(content) => Ok(content),
            Err(e) => Ok(format!("Error: {}", e)),
        }
    })?)?;

    // --- Async Bindings ---

    // Bind 'llm_chat'
    crablet.set("llm_chat", lua.create_async_function(|_, (model, prompt): (String, String)| async move {
        let client = OpenAiClient::new(&model).map_err(|e| e.into_lua_err())?;
        let message = Message::new("user", &prompt);
        let response = client.chat_complete(&[message]).await.map_err(|e| e.into_lua_err())?;
        Ok(response)
    })?)?;

    // Bind 'vision_describe'
    crablet.set("vision_describe", lua.create_async_function(|_, path: String| async move {
        let processor = ImageProcessor::new().map_err(|e| e.into_lua_err())?;
        let description = processor.describe(&path).await.map_err(|e| e.into_lua_err())?;
        Ok(description)
    })?)?;

    // Bind 'audio_transcribe'
    #[cfg(feature = "audio")]
    crablet.set("audio_transcribe", lua.create_async_function(|_, path: String| async move {
        let processor = AudioTool::new().map_err(|e| e.into_lua_err())?;
        let text = processor.transcribe(&path).await.map_err(|e| e.into_lua_err())?;
        Ok(text)
    })?)?;

    // Bind 'audio_speak'
    #[cfg(feature = "audio")]
    crablet.set("audio_speak", lua.create_async_function(|_, (text, output_path): (String, String)| async move {
        let processor = AudioTool::new().map_err(|e| e.into_lua_err())?;
        processor.speak(&text, &output_path).await.map_err(|e| e.into_lua_err())?;
        Ok(())
    })?)?;

    // Bind 'extract_knowledge'
    #[cfg(feature = "knowledge")]
    crablet.set("extract_knowledge", lua.create_async_function(|_, text: String| async move {
        let extractor = KnowledgeExtractor::new().map_err(|e| e.into_lua_err())?;
        let result = extractor.extract_from_text(&text).await.map_err(|e| e.into_lua_err())?;
        // Return as JSON string for Lua to parse if needed (or we could map to Lua Table, but JSON is easier for now)
        let json = serde_json::to_string(&result).map_err(|e| e.into_lua_err())?;
        Ok(json)
    })?)?;

    // Bind 'search'
    crablet.set("search", lua.create_async_function(|_, query: String| async move {
        let tool = WebSearchTool::new();
        match tool.search(&query).await {
            Ok(results) => {
                // Convert to simple string for now, or JSON
                let summary = results.iter().map(|r| format!("- {} ({})", r.title, r.link)).collect::<Vec<_>>().join("\n");
                Ok(summary)
            },
            Err(e) => Ok(format!("Error: {}", e)),
        }
    })?)?;

    // Bind 'http_read'
    crablet.set("http_read", lua.create_async_function(|_, url: String| async move {
        match crate::tools::http::HttpTool::read_url(&url).await {
            Ok(content) => Ok(content),
            Err(e) => Ok(format!("Error: {}", e)),
        }
    })?)?;

    globals.set("crablet", crablet)?;
    
    Ok(())
}
