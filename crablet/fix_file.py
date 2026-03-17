with open('src/skills/openclaw_executor.rs.bak', 'rb') as f:
    content = f.read()

# 直接替换字节
# 第296行: "Empty response from LLM"
# 查找并替换
old1 = b'ResponseType::Error("Empty response from LLM".to_string())'
new1 = b'ResponseType::Error("Empty response from LLM".to_string())'

old2 = b'ResponseType::Error("No content in LLM response".to_string())'
new2 = b'ResponseType::Error("No content in LLM response".to_string())'

old3 = b'.context("Failed to parse tool arguments")?;'
new3 = b'.context("Failed to parse tool arguments")?;'

old4 = b'Ok(format!("Tool {} executed with args: {}", name, args))'
new4 = b'Ok(format!("Tool {} executed with args: {}", name, args))'

# 检查这些字节是否存在
print(f'Found old1: {old1 in content}')
print(f'Found old2: {old2 in content}')
print(f'Found old3: {old3 in content}')
print(f'Found old4: {old4 in content}')

# 即使内容相同，也重新写入
with open('src/skills/openclaw_executor.rs', 'wb') as f:
    f.write(content)

print('File written')
