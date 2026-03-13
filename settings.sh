#!/bin/bash
set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENV_FILE="${ROOT_DIR}/crablet/.env"
INTERACTIVE=1
VERIFY_MODEL=1

for arg in "$@"; do
    case "$arg" in
        --non-interactive)
            INTERACTIVE=0
            ;;
        --skip-verify)
            VERIFY_MODEL=0
            ;;
        *)
            echo -e "${RED}Error: Unknown option ${arg}${NC}"
            echo "Usage: ./settings.sh [--non-interactive] [--skip-verify]"
            exit 1
            ;;
    esac
done

get_env_var() {
    local key="$1"
    grep -E "^${key}=" "$ENV_FILE" | head -n 1 | cut -d '=' -f2- | tr -d '\r'
}

set_env_var() {
    local key="$1"
    local value="$2"
    if grep -q "^${key}=" "$ENV_FILE"; then
        awk -v k="$key" -v v="$value" '
            BEGIN { replaced = 0 }
            $0 ~ ("^" k "=") && replaced == 0 { print k "=" v; replaced = 1; next }
            { print }
        ' "$ENV_FILE" > "${ENV_FILE}.tmp" && mv "${ENV_FILE}.tmp" "$ENV_FILE"
    else
        echo "${key}=${value}" >> "$ENV_FILE"
    fi
}

ensure_env_var() {
    local key="$1"
    local default_value="$2"
    if ! grep -q "^${key}=" "$ENV_FILE"; then
        echo "${key}=${default_value}" >> "$ENV_FILE"
    fi
}

trim_value() {
    local v="$1"
    v="${v%\"}"
    v="${v#\"}"
    v="${v%\'}"
    v="${v#\'}"
    echo "$v"
}

default_base_for_vendor() {
    case "$1" in
        aliyun) echo "https://dashscope.aliyuncs.com/compatible-mode/v1" ;;
        openai) echo "https://api.openai.com/v1" ;;
        kimi) echo "https://api.moonshot.cn/v1" ;;
        zhipu) echo "https://open.bigmodel.cn/api/paas/v4" ;;
        ollama) echo "http://127.0.0.1:11434/v1" ;;
        custom) echo "" ;;
        *) echo "" ;;
    esac
}

default_model_for_vendor() {
    case "$1" in
        aliyun) echo "qwen-plus" ;;
        openai) echo "gpt-4o-mini" ;;
        kimi) echo "moonshot-v1-8k" ;;
        zhipu) echo "glm-4-flash" ;;
        ollama) echo "qwen2.5:14b" ;;
        custom) echo "qwen-plus" ;;
        *) echo "qwen-plus" ;;
    esac
}

is_model_name_reasonable() {
    local vendor="$1"
    local model="$2"
    case "$vendor" in
        aliyun) [[ "$model" == qwen* || "$model" == wanx* ]] ;;
        openai) [[ "$model" == gpt* || "$model" == o1* || "$model" == o3* ]] ;;
        kimi) [[ "$model" == moonshot* || "$model" == kimi* ]] ;;
        zhipu) [[ "$model" == glm* || "$model" == cog* ]] ;;
        ollama) [[ "$model" == *:* || "$model" == llama* || "$model" == qwen* || "$model" == mistral* ]] ;;
        custom) return 0 ;;
        *) return 0 ;;
    esac
}

verify_model_connectivity() {
    local base_url="$1"
    local api_key="$2"
    local model="$3"
    local vendor="$4"
    local endpoint="${base_url%/}/chat/completions"
    if [ -z "$api_key" ] && [ "$vendor" != "ollama" ]; then
        echo -e "${YELLOW}Skip verify: API key is empty.${NC}"
        return 0
    fi
    local auth_args=()
    if [ -n "$api_key" ]; then
        auth_args=(-H "Authorization: Bearer ${api_key}")
    fi
    local payload
    payload=$(printf '{"model":"%s","messages":[{"role":"user","content":"ping"}],"max_tokens":8,"temperature":0}' "$model")
    local response
    response=$(curl -sS --max-time 25 "${auth_args[@]}" -H "Content-Type: application/json" -d "$payload" "$endpoint" -w '\n__HTTP_STATUS__:%{http_code}' || true)
    local http_status
    http_status=$(echo "$response" | awk -F: '/__HTTP_STATUS__/{print $2}' | tail -n 1)
    local body
    body=$(echo "$response" | sed '/__HTTP_STATUS__:/d')
    if [ "$http_status" = "200" ] && echo "$body" | grep -q '"choices"'; then
        echo -e "${GREEN}Model verification passed (${vendor}/${model}).${NC}"
        return 0
    fi
    echo -e "${YELLOW}Model verification failed (${vendor}/${model}), HTTP ${http_status:-N/A}.${NC}"
    if echo "$body" | grep -q '"error"'; then
        echo "$body" | head -n 3
    fi
    return 1
}

if [ ! -f "$ENV_FILE" ]; then
    mkdir -p "$(dirname "$ENV_FILE")"
    cat > "$ENV_FILE" <<EOL
PORT=3000
RUST_LOG=info
DATABASE_URL=sqlite:crablet.db?mode=rwc
JWT_SECRET=$(openssl rand -hex 32)
DASHSCOPE_API_KEY=
OPENAI_API_KEY=
OPENAI_API_BASE=https://dashscope.aliyuncs.com/compatible-mode/v1
OPENAI_MODEL_NAME=qwen-plus
OLLAMA_MODEL=qwen2.5:14b
EOL
fi

ensure_env_var "OPENAI_API_BASE" "https://dashscope.aliyuncs.com/compatible-mode/v1"
ensure_env_var "OPENAI_MODEL_NAME" "qwen-plus"
ensure_env_var "OPENAI_API_KEY" ""
ensure_env_var "DASHSCOPE_API_KEY" ""
ensure_env_var "LLM_VENDOR" "aliyun"

vendor=$(trim_value "$(get_env_var LLM_VENDOR)")
[ -z "$vendor" ] && vendor="aliyun"

if [ "$INTERACTIVE" -eq 1 ] && [ -t 0 ]; then
    echo -e "${GREEN}🛠️  Configure LLM settings${NC}"
    echo "Select vendor preset:"
    echo "  1) Aliyun (DashScope / Qwen)"
    echo "  2) OpenAI"
    echo "  3) Kimi (Moonshot)"
    echo "  4) ZhiPu (GLM)"
    echo "  5) Ollama (Local)"
    echo "  6) Custom OpenAI-Compatible"
    read -r -p "Vendor [current: ${vendor}]: " vendor_choice
    case "$vendor_choice" in
        1) vendor="aliyun" ;;
        2) vendor="openai" ;;
        3) vendor="kimi" ;;
        4) vendor="zhipu" ;;
        5) vendor="ollama" ;;
        6) vendor="custom" ;;
        "") ;;
        *) echo -e "${YELLOW}Unknown choice, keep current vendor: ${vendor}.${NC}" ;;
    esac
    set_env_var LLM_VENDOR "$vendor"

    suggested_base=$(default_base_for_vendor "$vendor")
    current_base=$(trim_value "$(get_env_var OPENAI_API_BASE)")
    [ -z "$current_base" ] && current_base="$suggested_base"
    [ -z "$current_base" ] && current_base="https://dashscope.aliyuncs.com/compatible-mode/v1"
    read -r -p "API Base URL [${current_base}]: " input_base
    [ -n "$input_base" ] && set_env_var OPENAI_API_BASE "$input_base"

    suggested_model=$(default_model_for_vendor "$vendor")
    current_model=$(trim_value "$(get_env_var OPENAI_MODEL_NAME)")
    [ -z "$current_model" ] && current_model="$suggested_model"
    [ -z "$current_model" ] && current_model="qwen-plus"
    read -r -p "Model Name [${current_model}]: " input_model
    [ -n "$input_model" ] && set_env_var OPENAI_MODEL_NAME "$input_model"

    echo "API Key will sync to DASHSCOPE_API_KEY and OPENAI_API_KEY."
    read -r -s -p "API Key (leave blank to keep existing): " input_key
    echo
    if [ -n "$input_key" ]; then
        set_env_var DASHSCOPE_API_KEY "$input_key"
        set_env_var OPENAI_API_KEY "$input_key"
    fi
fi

dashscope_key=$(get_env_var DASHSCOPE_API_KEY)
openai_key=$(get_env_var OPENAI_API_KEY)
api_base=$(trim_value "$(get_env_var OPENAI_API_BASE)")
model_name=$(trim_value "$(get_env_var OPENAI_MODEL_NAME)")
vendor=$(trim_value "$(get_env_var LLM_VENDOR)")
[ -z "$vendor" ] && vendor="aliyun"

if [ -z "$dashscope_key" ] && [ -z "$openai_key" ]; then
    echo -e "${RED}Warning: API key is empty. Chat may return empty results until configured.${NC}"
fi

if ! is_model_name_reasonable "$vendor" "$model_name"; then
    echo -e "${YELLOW}Warning: Model name '${model_name}' may not match vendor '${vendor}'.${NC}"
fi

if [ "$VERIFY_MODEL" -eq 1 ]; then
    verify_model_connectivity "$api_base" "${dashscope_key:-$openai_key}" "$model_name" "$vendor" || true
fi

echo -e "${GREEN}LLM settings saved: ${ENV_FILE}${NC}"
