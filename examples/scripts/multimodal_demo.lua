-- Multimodal Demo Script
-- This script demonstrates how to use Crablet's multimodal capabilities via Lua

print("🦀 Crablet Multimodal Demo Script")
print("================================")

-- 1. Check for audio file
local audio_path = "test_audio.mp3"
local image_path = "test_image.png"

-- Helper to check file existence
local function file_exists(name)
   local f = io.open(name, "r")
   if f ~= nil then io.close(f) return true else return false end
end

-- 2. Audio Transcription (ASR)
if file_exists(audio_path) then
    print("\n[Audio] Transcribing " .. audio_path .. "...")
    local text, err = crablet.audio_transcribe(audio_path)
    if err then
        print("Error: " .. err)
    else
        print("Transcription: " .. text)
        
        -- 3. LLM Summarization
        print("\n[LLM] Summarizing transcription...")
        local summary_prompt = "Please summarize the following text in one sentence: " .. text
        local summary = crablet.llm_chat("gpt-4o-mini", summary_prompt)
        print("Summary: " .. summary)
        
        -- 4. Text to Speech (TTS)
        print("\n[Audio] Generating speech from summary...")
        local output_speech = "summary_speech.mp3"
        crablet.audio_speak(summary, output_speech)
        print("Speech saved to " .. output_speech)
    end
else
    print("\n[Audio] " .. audio_path .. " not found. Skipping ASR/TTS demo.")
    print("To test: provide a 'test_audio.mp3' file.")
end

-- 5. Vision (Image Description)
if file_exists(image_path) then
    print("\n[Vision] Analyzing " .. image_path .. "...")
    local description, err = crablet.vision_describe(image_path)
    if err then
        print("Error: " .. err)
    else
        print("Description: " .. description)
        
        -- 6. Knowledge Extraction from Description
        print("\n[Knowledge] Extracting entities from image description...")
        local knowledge_json = crablet.extract_knowledge(description)
        print("Extracted Knowledge (JSON): " .. knowledge_json)
    end
else
    print("\n[Vision] " .. image_path .. " not found. Skipping Vision demo.")
    print("To test: provide a 'test_image.png' file.")
end

print("\nDemo completed.")
return "Success"
