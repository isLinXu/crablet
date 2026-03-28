import { describe, expect, it } from 'vitest';
import {
  computeFileHash,
  detectCategoryByExt,
  extractTagsByName,
  heuristicSecurityScan,
  validateFile,
} from '../filePipeline';

const toBytes = (parts: BlobPart[]) => {
  const encoder = new TextEncoder();
  const chunks = parts.map((part) => {
    if (typeof part === 'string') return encoder.encode(part);
    if (part instanceof Uint8Array) return part;
    if (ArrayBuffer.isView(part)) return new Uint8Array(part.buffer.slice(part.byteOffset, part.byteOffset + part.byteLength));
    if (part instanceof ArrayBuffer) return new Uint8Array(part);
    return new Uint8Array(0);
  });
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  return merged;
};

const toArrayBuffer = (bytes: Uint8Array) =>
  bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;

const makeSlice = (bytes: Uint8Array) => ({
  arrayBuffer: async () => toArrayBuffer(bytes),
});

const makeFile = (parts: BlobPart[], name: string, type = 'application/octet-stream') => {
  const bytes = toBytes(parts);
  return {
    name,
    size: bytes.byteLength,
    type,
    slice: (start?: number, end?: number) => makeSlice(bytes.slice(start, end)),
    arrayBuffer: async () => toArrayBuffer(bytes),
  } as File;
};

describe('filePipeline', () => {
  it('detects supported categories by extension', () => {
    expect(detectCategoryByExt('report.PDF')).toEqual({ category: 'document', ext: 'pdf' });
    expect(detectCategoryByExt('photo.jpeg')).toEqual({ category: 'image', ext: 'jpeg' });
    expect(detectCategoryByExt('clip.mp4')).toEqual({ category: 'video', ext: 'mp4' });
    expect(detectCategoryByExt('archive.bin')).toEqual({ category: 'unknown', ext: 'bin' });
  });

  it('validates supported files and blocks risky inputs', () => {
    const okFile = makeFile(['hello'], 'notes.md', 'text/markdown');
    expect(validateFile(okFile)).toEqual({ ok: true, category: 'document', ext: 'md' });

    const blockedExecutable = makeFile(['MZ'], 'payload.exe');
    expect(validateFile(blockedExecutable)).toMatchObject({
      ok: false,
      ext: 'exe',
      reason: '可执行文件被安全策略拦截',
    });

    const unsupported = makeFile(['???'], 'payload.xyz');
    expect(validateFile(unsupported)).toMatchObject({
      ok: false,
      category: 'unknown',
      reason: '不支持的文件格式',
    });

    const hugeVideo = makeFile([new Uint8Array(301 * 1024 * 1024)], 'movie.mp4');
    expect(validateFile(hugeVideo)).toMatchObject({
      ok: false,
      category: 'video',
      ext: 'mp4',
    });
  });

  it('detects suspicious file signatures and scripts', async () => {
    const mzFile = makeFile([Uint8Array.from([0x4d, 0x5a, 0x90, 0x00])], 'sample.bin');
    await expect(heuristicSecurityScan(mzFile)).resolves.toMatchObject({
      safe: false,
      reason: '检测到可执行文件签名(MZ)',
    });

    const elfFile = makeFile([Uint8Array.from([0x7f, 0x45, 0x4c, 0x46])], 'sample.bin');
    await expect(heuristicSecurityScan(elfFile)).resolves.toMatchObject({
      safe: false,
      reason: '检测到ELF可执行文件签名',
    });

    const scriptFile = makeFile(['<?php echo "boom";'], 'sample.txt', 'text/plain');
    await expect(heuristicSecurityScan(scriptFile)).resolves.toMatchObject({
      safe: false,
      reason: '检测到潜在恶意脚本特征',
    });

    const safeFile = makeFile(['plain text'], 'sample.txt', 'text/plain');
    await expect(heuristicSecurityScan(safeFile)).resolves.toEqual({ safe: true });
  });

  it('hashes file content and extracts normalized tags', async () => {
    const file = makeFile(['Crablet'], 'Agent_Router-v2.final.md', 'text/markdown');
    const hash = await computeFileHash(file);

    expect(hash).toMatch(/^[0-9a-f]{64}$/);
    expect(extractTagsByName(file.name)).toEqual(['agent', 'router', 'v2', 'final']);
  });
});
