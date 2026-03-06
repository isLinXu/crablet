export type SupportedCategory = 'document' | 'image' | 'audio' | 'video' | 'unknown';

export interface FileValidationResult {
  ok: boolean;
  category: SupportedCategory;
  ext: string;
  reason?: string;
}

export const SUPPORTED_EXTENSIONS: Record<SupportedCategory, string[]> = {
  document: ['pdf', 'doc', 'docx', 'txt', 'md', 'csv', 'xls', 'xlsx'],
  image: ['jpg', 'jpeg', 'png', 'gif', 'svg', 'webp'],
  audio: ['mp3', 'wav', 'm4a', 'flac'],
  video: ['mp4', 'avi', 'mov', 'mkv'],
  unknown: [],
};

const LIMIT_MB: Record<SupportedCategory, number> = {
  document: 30,
  image: 20,
  audio: 80,
  video: 300,
  unknown: 0,
};

const suspiciousExt = new Set(['exe', 'dll', 'bat', 'cmd', 'sh', 'js', 'jar', 'msi', 'com', 'vbs', 'ps1']);

export const detectCategoryByExt = (fileName: string): { category: SupportedCategory; ext: string } => {
  const ext = (fileName.split('.').pop() || '').toLowerCase();
  if (!ext) return { category: 'unknown', ext: '' };
  if (SUPPORTED_EXTENSIONS.document.includes(ext)) return { category: 'document', ext };
  if (SUPPORTED_EXTENSIONS.image.includes(ext)) return { category: 'image', ext };
  if (SUPPORTED_EXTENSIONS.audio.includes(ext)) return { category: 'audio', ext };
  if (SUPPORTED_EXTENSIONS.video.includes(ext)) return { category: 'video', ext };
  return { category: 'unknown', ext };
};

export const validateFile = (file: File): FileValidationResult => {
  const { category, ext } = detectCategoryByExt(file.name);
  if (suspiciousExt.has(ext)) return { ok: false, category, ext, reason: '可执行文件被安全策略拦截' };
  if (category === 'unknown') return { ok: false, category, ext, reason: '不支持的文件格式' };
  const maxSize = LIMIT_MB[category] * 1024 * 1024;
  if (file.size > maxSize) return { ok: false, category, ext, reason: `文件超过大小限制(${LIMIT_MB[category]}MB)` };
  return { ok: true, category, ext };
};

export const heuristicSecurityScan = async (file: File): Promise<{ safe: boolean; reason?: string }> => {
  const buf = await file.slice(0, 64).arrayBuffer();
  const bytes = new Uint8Array(buf);
  if (bytes.length >= 2 && bytes[0] === 0x4d && bytes[1] === 0x5a) return { safe: false, reason: '检测到可执行文件签名(MZ)' };
  if (bytes.length >= 4 && bytes[0] === 0x7f && bytes[1] === 0x45 && bytes[2] === 0x4c && bytes[3] === 0x46) return { safe: false, reason: '检测到ELF可执行文件签名' };
  const textHead = new TextDecoder().decode(bytes).toLowerCase();
  if (textHead.includes('<?php') || textHead.includes('powershell')) return { safe: false, reason: '检测到潜在恶意脚本特征' };
  return { safe: true };
};

export const computeFileHash = async (file: File): Promise<string> => {
  const data = await file.arrayBuffer();
  const digest = await crypto.subtle.digest('SHA-256', data);
  const arr = Array.from(new Uint8Array(digest));
  return arr.map((b) => b.toString(16).padStart(2, '0')).join('');
};

export const extractTagsByName = (fileName: string): string[] => {
  const base = fileName.replace(/\.[^.]+$/, '').toLowerCase();
  const chunks = base.split(/[_\-\s.]+/).filter(Boolean);
  return [...new Set(chunks)].slice(0, 8);
};
