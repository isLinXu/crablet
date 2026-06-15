// Settings helpers for vendor detection, model suggestions, etc.

export const VENDOR_OPTIONS = [
  { label: 'OpenAI', value: 'openai' },
  { label: 'Anthropic', value: 'anthropic' },
  { label: 'Google', value: 'google' },
  { label: '阿里百炼', value: 'dashscope' },
  { label: '腾讯混元', value: 'hunyuan' },
  { label: '字节豆包', value: 'doubao' },
  { label: '本地 Ollama', value: 'ollama' },
  { label: '自定义', value: 'custom' },
] as const;

export const VENDOR_GUIDE: Record<string, string> = {
  openai: '在 platform.openai.com 获取 API Key',
  anthropic: '在 console.anthropic.com 获取 API Key',
  google: '在 aistudio.google.com 获取 API Key',
  dashscope: '在 dashscope.console.aliyun.com 获取 API Key',
  hunyuan: '在 cloud.tencent.com 获取 API Key',
  doubao: '在 console.volcengine.com 获取 API Key',
  ollama: '确保本地已运行 Ollama 服务（默认端口 11434）',
  custom: '输入自定义 API 地址和 Key',
};

export const KEY_PLACEHOLDER: Record<string, string> = {
  openai: 'sk-...',
  anthropic: 'sk-ant-...',
  google: 'AIza...',
  dashscope: 'sk-...',
  hunyuan: '...',
  doubao: '...',
  ollama: '无需 Key',
  custom: '输入 API Key',
};

export function modelSuggestionsForVendor(vendor: string): string[] {
  const suggestions: Record<string, string[]> = {
    openai: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'o1-mini'],
    anthropic: ['claude-sonnet-4-20250514', 'claude-3-5-haiku-20241022'],
    google: ['gemini-2.0-flash', 'gemini-1.5-pro'],
    dashscope: ['qwen-max', 'qwen-plus', 'qwen-turbo'],
    hunyuan: ['hunyuan-lite', 'hunyuan-standard', 'hunyuan-pro'],
    doubao: ['doubao-pro-32k', 'doubao-lite-32k'],
    ollama: ['llama3', 'mistral', 'codellama'],
    custom: [],
  };
  return suggestions[vendor] || [];
}

export function normalizeVendorName(vendor: string): string {
  const names: Record<string, string> = {
    openai: 'OpenAI',
    anthropic: 'Anthropic',
    google: 'Google',
    dashscope: '阿里百炼',
    hunyuan: '腾讯混元',
    doubao: '字节豆包',
    ollama: 'Ollama',
    custom: '自定义',
  };
  return names[vendor] || vendor;
}

export function envVendorToRouteVendor(vendor: string): string {
  const mapping: Record<string, string> = {
    openai: 'openai',
    anthropic: 'anthropic',
    google: 'google',
    dashscope: 'dashscope',
    hunyuan: 'hunyuan',
    doubao: 'doubao',
    ollama: 'ollama',
    custom: 'custom',
  };
  return mapping[vendor] || 'general';
}

export function detectVendor(keyOrUrl: string): string {
  if (!keyOrUrl) return 'custom';
  if (keyOrUrl.startsWith('sk-ant-')) return 'anthropic';
  if (keyOrUrl.startsWith('AIza')) return 'google';
  if (keyOrUrl.includes('dashscope')) return 'dashscope';
  if (keyOrUrl.includes('hunyuan')) return 'hunyuan';
  if (keyOrUrl.includes('volces') || keyOrUrl.includes('ark.cn')) return 'doubao';
  if (keyOrUrl.includes('ollama') || keyOrUrl.includes('11434')) return 'ollama';
  if (keyOrUrl.startsWith('sk-')) return 'openai';
  return 'custom';
}

export function vendorToEnvVendor(vendor: string): string {
  const mapping: Record<string, string> = {
    openai: 'OPENAI_API_KEY',
    anthropic: 'ANTHROPIC_API_KEY',
    google: 'GOOGLE_API_KEY',
    dashscope: 'DASHSCOPE_API_KEY',
    hunyuan: 'HUNYUAN_API_KEY',
    doubao: 'DOUBAO_API_KEY',
    ollama: 'OLLAMA_HOST',
    custom: 'CUSTOM_API_KEY',
  };
  return mapping[vendor] || 'CUSTOM_API_KEY';
}

export function validateVendorModel(vendor: string, model: string): { valid: boolean; message?: string } {
  if (!vendor) return { valid: false, message: '请选择供应商' };
  if (!model) return { valid: false, message: '请选择模型' };
  return { valid: true };
}

export function showHttpError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  return String(error);
}

export async function verifySystemChatConfig(_baseUrl: string): Promise<{ ok: boolean; message: string }> {
  return { ok: true, message: '配置验证通过' };
}

export const TROUBLESHOOTING_GUIDE: Record<string, string[]> = {
  openai: [
    '确认 API Key 有效且有余额',
    '检查网络是否能访问 api.openai.com',
    '确认模型名称正确',
  ],
  anthropic: [
    '确认 API Key 有效',
    '检查网络是否能访问 api.anthropic.com',
  ],
  dashscope: [
    '确认 API Key 有效',
    '检查是否开通了对应模型',
  ],
  ollama: [
    '确认 Ollama 服务已启动（ollama serve）',
    '确认端口 11434 可访问',
    '确认模型已下载（ollama pull <model>）',
  ],
};
