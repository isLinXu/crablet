/**
 * Security utilities for HTML sanitization and XSS prevention
 */
import DOMPurify from 'dompurify';

/**
 * DOMPurify configuration for Markdown content
 * Allows safe HTML tags for formatting while blocking dangerous elements
 */
const SANITIZE_CONFIG = {
  ALLOWED_TAGS: [
    'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
    'p', 'br', 'hr',
    'strong', 'em', 'u', 's', 'blockquote',
    'ul', 'ol', 'li',
    'code', 'pre',
    'a',
    'table', 'thead', 'tbody', 'tr', 'th', 'td',
  ],
  ALLOWED_ATTR: [
    'href',
    'class',
    'src',
    'alt',
    'title',
  ],
  ALLOW_DATA_ATTR: false,
  FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'button'],
  FORBID_ATTR: ['onclick', 'onerror', 'onload', 'onmouseover', 'onmouseout', 'onfocus', 'onblur'],
  SANITIZE_DOM: true,
  KEEP_CONTENT: true,
};

/**
 * Sanitize HTML content to prevent XSS attacks
 * 
 * @param html - Raw HTML string to sanitize
 * @returns Sanitized HTML string
 */
export const sanitizeHtml = (html: string): string => {
  if (!html || typeof html !== 'string') {
    return '';
  }

  return DOMPurify.sanitize(html, SANITIZE_CONFIG);
};

/**
 * Sanitize markdown content by first sanitizing the HTML
 * 
 * @param markdown - Markdown string to sanitize
 * @returns Sanitized markdown string
 */
export const sanitizeMarkdown = (markdown: string): string => {
  if (!markdown || typeof markdown !== 'string') {
    return '';
  }

  // Note: This is a basic sanitizer. For full markdown security,
  // we should use a markdown parser that supports sanitization
  // or sanitize after markdown to HTML conversion
  return sanitizeHtml(markdown);
};

/**
 * Escape HTML special characters
 * Use this when displaying raw text that should not be interpreted as HTML
 * 
 * @param text - Text to escape
 * @returns Escaped text
 */
export const escapeHtml = (text: string): string => {
  if (!text || typeof text !== 'string') {
    return '';
  }

  const map: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#039;',
  };

  return text.replace(/[&<>"']/g, (char) => map[char]);
};

/**
 * Validate URL to prevent XSS via javascript: or data: URLs
 * 
 * @param url - URL to validate
 * @returns true if URL is safe, false otherwise
 */
export const isSafeUrl = (url: string): boolean => {
  if (!url || typeof url !== 'string') {
    return false;
  }

  try {
    const parsed = new URL(url);
    
    // Block dangerous protocols
    const dangerousProtocols = ['javascript:', 'data:', 'vbscript:', 'file:'];
    if (dangerousProtocols.some(proto => url.toLowerCase().startsWith(proto))) {
      return false;
    }

    // Only allow http and https
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
};

/**
 * Sanitize URL for safe display
 * 
 * @param url - URL to sanitize
 * @returns Sanitized URL or empty string if unsafe
 */
export const sanitizeUrl = (url: string): string => {
  if (isSafeUrl(url)) {
    return url;
  }
  return '';
};

/**
 * Security utilities object for easy import
 */
export const securityUtils = {
  sanitizeHtml,
  sanitizeMarkdown,
  escapeHtml,
  isSafeUrl,
  sanitizeUrl,
};

export default securityUtils;
