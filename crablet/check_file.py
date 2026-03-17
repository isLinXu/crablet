with open('src/skills/openclaw_executor.rs', 'rb') as f:
    content = f.read()

# 查找零宽字符和其他特殊字符
zw_chars = [
    (0x200B, 'ZERO WIDTH SPACE'),
    (0x200C, 'ZERO WIDTH NON-JOINER'),
    (0x200D, 'ZERO WIDTH JOINER'),
    (0xFEFF, 'ZERO WIDTH NO-BREAK SPACE (BOM)'),
    (0x2060, 'WORD JOINER'),
    (0x00AD, 'SOFT HYPHEN'),
]

for codepoint, name in zw_chars:
    if codepoint <= 0xFF:
        b = bytes([codepoint])
    else:
        b = chr(codepoint).encode('utf-8')
    
    idx = content.find(b)
    if idx >= 0:
        print(f'Found {name} at byte {idx}')
        print(f'  Context: {content[max(0,idx-10):idx+10]}')

# 检查第296行在文件中的确切位置
lines = content.split(b'\n')
line_start = 0
for i in range(295):
    line_start += len(lines[i]) + 1

print(f'\nLine 296 starts at byte {line_start}')
line296 = lines[295]
print(f'Line 296 length: {len(line296)} bytes')
print(f'Line 296 bytes: {line296.hex()}')

# 查找 LLM 的位置
llm_idx = line296.find(b'LLM')
print(f'LLM at relative position: {llm_idx}')
print(f'Absolute position: {line_start + llm_idx}')

# 检查该位置是否有特殊字符
for i in range(len(line296)):
    b = line296[i]
    if b >= 0x80:
        print(f'Non-ASCII at relative position {i}: 0x{b:02x}')
