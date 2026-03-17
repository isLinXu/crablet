/**
 * 文件内容提取工具
 * 支持多种文件格式的文本内容提取
 */

// 动态导入 pdf-parse 以避免服务端渲染问题
const loadPdfParser = async () => {
  const pdfParse = await import('pdf-parse');
  // pdf-parse 的导出方式比较特殊，需要处理不同的导出格式
  return (pdfParse as any).default || pdfParse;
};

// 动态导入 tesseract.js 用于OCR
const loadTesseract = async () => {
  const tesseract = await import('tesseract.js');
  return tesseract;
};

// PDF.js 用于渲染PDF页面
const loadPdfJs = async () => {
  const pdfjs = await import('pdfjs-dist');
  // 设置worker
  pdfjs.GlobalWorkerOptions.workerSrc = `https://cdnjs.cloudflare.com/ajax/libs/pdf.js/${pdfjs.version}/pdf.worker.min.js`;
  return pdfjs;
};

export interface ExtractedContent {
  text: string;
  success: boolean;
  error?: string;
  truncated: boolean;
  isOcr?: boolean; // 标记是否使用了OCR
}

export interface ExtractionOptions {
  maxLength?: number;
  onProgress?: (progress: number) => void;
  onOcrStart?: () => void;
}

/**
 * 根据文件类型提取文本内容
 */
export async function extractFileContent(
  file: File,
  options: ExtractionOptions = {}
): Promise<ExtractedContent> {
  const { maxLength = 5 * 1024 * 1024, onProgress, onOcrStart } = options;
  const ext = (file.name.split('.').pop() || '').toLowerCase();

  try {
    let text: string;

    switch (ext) {
      case 'pdf':
        const pdfResult = await extractPdfContent(file, onProgress, onOcrStart);
        return {
          text: pdfResult.text,
          success: true,
          truncated: pdfResult.text.length > maxLength,
          isOcr: pdfResult.isOcr,
        };
      case 'txt':
      case 'md':
      case 'csv':
      case 'json':
      case 'js':
      case 'ts':
      case 'tsx':
      case 'jsx':
      case 'html':
      case 'css':
      case 'py':
      case 'java':
      case 'cpp':
      case 'c':
      case 'h':
      case 'go':
      case 'rs':
      case 'swift':
      case 'kt':
      case 'xml':
      case 'yaml':
      case 'yml':
        text = await file.text();
        break;
      case 'doc':
      case 'docx':
        // Word 文档暂不支持直接解析，提示用户
        return {
          text: `[Word 文档: ${file.name}]\n注意：Word 文档(.doc/.docx)暂不支持直接解析文本内容。建议将文档转换为 PDF 或纯文本格式后上传。`,
          success: true,
          truncated: false,
        };
      case 'xls':
      case 'xlsx':
        // Excel 文件暂不支持直接解析
        return {
          text: `[Excel 表格: ${file.name}]\n注意：Excel 文件(.xls/.xlsx)暂不支持直接解析文本内容。建议将表格导出为 CSV 格式后上传。`,
          success: true,
          truncated: false,
        };
      default:
        // 尝试作为文本读取
        text = await file.text();
    }

    // 检查是否需要截断
    const truncated = text.length > maxLength;
    const finalText = truncated
      ? text.slice(0, maxLength) + '\n... (内容已截断，文件过大)'
      : text;

    return {
      text: finalText,
      success: true,
      truncated,
    };
  } catch (error) {
    return {
      text: '',
      success: false,
      error: error instanceof Error ? error.message : '未知错误',
      truncated: false,
    };
  }
}

interface PdfExtractionResult {
  text: string;
  isOcr: boolean;
}

/**
 * 提取 PDF 文件的文本内容
 * 支持文本PDF和扫描版PDF（OCR）
 */
async function extractPdfContent(
  file: File,
  onProgress?: (progress: number) => void,
  onOcrStart?: () => void
): Promise<PdfExtractionResult> {
  const arrayBuffer = await file.arrayBuffer();
  const uint8Array = new Uint8Array(arrayBuffer);

  try {
    // 首先尝试使用 pdf-parse 提取文本
    const pdfParse = await loadPdfParser();
    const result = await pdfParse(uint8Array);
    
    // 检查提取的文本是否有效（非空且不是乱码）
    const extractedText = result.text || '';
    const isValidText = validateExtractedText(extractedText);
    
    if (isValidText && extractedText.trim().length > 100) {
      return { text: extractedText, isOcr: false };
    }
    
    // 如果文本提取不完整或无效，尝试使用 PDF.js + OCR
    console.log('PDF文本提取不完整，尝试OCR识别...');
    if (onOcrStart) onOcrStart();
    const ocrText = await extractPdfWithOCR(file, onProgress);
    return { text: ocrText, isOcr: true };
    
  } catch (error) {
    console.warn('PDF 解析失败，尝试OCR:', error);
    // 尝试OCR作为备选方案
    try {
      if (onOcrStart) onOcrStart();
      const ocrText = await extractPdfWithOCR(file, onProgress);
      return { text: ocrText, isOcr: true };
    } catch (ocrError) {
      console.error('OCR也失败了:', ocrError);
      return {
        text: `[PDF 文件: ${file.name}]\n注意：无法提取 PDF 文本内容。该文件可能是扫描版 PDF、加密 PDF 或图片格式。建议：\n1. 使用支持OCR的工具转换后再上传\n2. 或者手动复制文本内容粘贴到聊天中`,
        isOcr: false,
      };
    }
  }
}

/**
 * 验证提取的文本是否有效
 */
function validateExtractedText(text: string): boolean {
  if (!text || text.trim().length === 0) {
    return false;
  }
  
  // 检查是否包含大量乱码特征（如连续的非打印字符）
  const nonPrintableRatio = (text.match(/[\x00-\x08\x0B\x0C\x0E-\x1F]/g) || []).length / text.length;
  if (nonPrintableRatio > 0.1) {
    return false;
  }
  
  // 检查是否包含合理的文本比例（字母、数字、中文、标点）
  const validCharRatio = (text.match(/[\p{L}\p{N}\p{P}\s]/gu) || []).length / text.length;
  return validCharRatio > 0.7;
}

/**
 * 使用 PDF.js + Tesseract OCR 提取PDF内容
 */
async function extractPdfWithOCR(
  file: File,
  onProgress?: (progress: number) => void
): Promise<string> {
  const pdfjs = await loadPdfJs();
  const tesseract = await loadTesseract();

  const arrayBuffer = await file.arrayBuffer();
  const pdf = await pdfjs.getDocument({ data: arrayBuffer }).promise;

  let fullText = `[PDF 文件: ${file.name} - OCR识别结果]\n\n`;
  const maxPages = Math.min(pdf.numPages, 10); // 最多处理10页，避免耗时过长

  // 创建Tesseract worker
  const worker = await tesseract.createWorker('eng+chi_sim');

  try {
    for (let pageNum = 1; pageNum <= maxPages; pageNum++) {
      try {
        // 报告进度
        if (onProgress) {
          onProgress(Math.round((pageNum - 1) / maxPages * 100));
        }

        const page = await pdf.getPage(pageNum);
        const viewport = page.getViewport({ scale: 2.0 }); // 提高分辨率以获得更好的OCR效果

        // 创建canvas
        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d');
        if (!context) continue;

        canvas.width = viewport.width;
        canvas.height = viewport.height;

        // 渲染PDF页面到canvas
        const renderTask = page.render({
          canvasContext: context,
          viewport: viewport,
          background: 'white',
        } as any);
        await renderTask.promise;

        // 将canvas转换为blob
        const blob = await new Promise<Blob | null>((resolve) => {
          canvas.toBlob((b) => resolve(b), 'image/png');
        });

        if (!blob) continue;

        // OCR识别
        const imageUrl = URL.createObjectURL(blob);
        const { data: { text } } = await worker.recognize(imageUrl);
        URL.revokeObjectURL(imageUrl);

        if (text.trim()) {
          fullText += `--- 第 ${pageNum} 页 ---\n${text}\n\n`;
        }

        // 清理
        page.cleanup();

      } catch (pageError) {
        console.warn(`第 ${pageNum} 页OCR失败:`, pageError);
        fullText += `--- 第 ${pageNum} 页 ---\n[OCR识别失败]\n\n`;
      }
    }

    // 完成进度
    if (onProgress) {
      onProgress(100);
    }

    await worker.terminate();

    if (fullText.trim().length > file.name.length + 50) {
      return fullText;
    } else {
      throw new Error('OCR未识别到有效文本');
    }

  } catch (error) {
    await worker.terminate();
    throw error;
  }
}

/**
 * 检测文件是否为二进制文件
 */
export function isBinaryFile(file: File): Promise<boolean> {
  return new Promise((resolve) => {
    const reader = new FileReader();
    reader.onload = (e) => {
      const content = e.target?.result as string;
      if (!content) {
        resolve(true);
        return;
      }
      // 检查是否包含大量空字符或其他二进制特征
      const nullCount = (content.match(/\0/g) || []).length;
      const totalLength = content.length;
      // 如果空字符占比超过 1%，认为是二进制文件
      resolve(nullCount / totalLength > 0.01);
    };
    reader.onerror = () => resolve(true);
    // 读取前 8KB 进行判断
    reader.readAsBinaryString(file.slice(0, 8192));
  });
}

/**
 * 获取文件类型的友好描述
 */
export function getFileTypeDescription(fileName: string): string {
  const ext = (fileName.split('.').pop() || '').toLowerCase();
  const descriptions: Record<string, string> = {
    pdf: 'PDF 文档',
    doc: 'Word 文档',
    docx: 'Word 文档',
    txt: '纯文本',
    md: 'Markdown 文档',
    csv: 'CSV 表格',
    xls: 'Excel 表格',
    xlsx: 'Excel 表格',
    json: 'JSON 文件',
    html: 'HTML 文件',
    xml: 'XML 文件',
    js: 'JavaScript 文件',
    ts: 'TypeScript 文件',
    py: 'Python 文件',
    java: 'Java 文件',
    cpp: 'C++ 文件',
    c: 'C 文件',
    go: 'Go 文件',
    rs: 'Rust 文件',
  };
  return descriptions[ext] || '文件';
}
