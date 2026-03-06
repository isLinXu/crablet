import React, { useState } from 'react';
import { Check, Copy } from 'lucide-react';

interface CodeBlockProps {
  language: string;
  value: string;
}

export const CodeBlock: React.FC<CodeBlockProps> = ({ language, value }) => {
  const [copied, setCopied] = useState(false);

  const copyToClipboard = () => {
    navigator.clipboard.writeText(value);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative my-4 rounded-lg bg-gray-900 text-gray-100 overflow-hidden group">
      <div className="flex items-center justify-between px-4 py-2 bg-gray-800 text-xs text-gray-400">
        <span>{language}</span>
        <button
          onClick={copyToClipboard}
          className="flex items-center space-x-1 hover:text-white"
        >
          {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
          <span>{copied ? 'Copied!' : 'Copy'}</span>
        </button>
      </div>
      <div className="p-4 overflow-x-auto">
        <pre className="font-mono text-sm">
          <code>{value}</code>
        </pre>
      </div>
    </div>
  );
};
