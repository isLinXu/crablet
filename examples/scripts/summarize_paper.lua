-- summarize_paper.lua
-- This script demonstrates how to use Crablet's API to summarize a PDF paper

local paper_path = "tests/samples/paper.pdf"

-- 1. Read PDF content (using crablet binding)
-- Note: In a real scenario, we would use a specialized PDF reading binding
-- For MVP, we simulate reading or assume text conversion
local content = crablet.read_file(paper_path)

if content:find("Error") then
    print("Failed to read paper: " .. content)
    return
end

-- 2. Construct a summary prompt
local prompt = "Please summarize the following paper content:\n\n" .. content

-- 3. Call LLM (via CLI for now, or direct binding if available)
-- In the future, we will expose `crablet.llm.chat()` directly
local cmd = "crablet run '" .. prompt .. "'"
local summary = crablet.run_command(cmd)

print("=== Paper Summary ===")
print(summary)
print("=====================")
