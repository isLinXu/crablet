import React from 'react';
import ReactMarkdown from 'react-markdown';
import { cn } from '../ui/Button';
import type { ExtendedMessage } from '@/store/chatStore';
import { CodeBlock } from './CodeBlock';
import { Bot, User, Copy, Check, Download, Pencil, Trash2, X } from 'lucide-react';
import { cognitiveLayerLabel } from '@/utils/cognitive';

interface MessageBubbleProps {
  message: ExtendedMessage;
  onEdit?: (id: string, newContent: string) => void;
  onDelete?: (id: string) => void;
}

export const MessageBubble: React.FC<MessageBubbleProps> = ({ message, onEdit, onDelete }) => {
  const isUser = message.role === 'user';
  const [copied, setCopied] = React.useState(false);
  const [isEditing, setIsEditing] = React.useState(false);
  const [editValue, setEditValue] = React.useState('');

  const handleCopy = () => {
    if (typeof message.content === 'string') {
        navigator.clipboard.writeText(message.content);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    }
  };

  const downloadImage = async (url: string, name: string) => {
    try {
      if (url.startsWith('data:image/')) {
        const a = document.createElement('a');
        a.href = url;
        a.download = name;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        return;
      }
      const res = await fetch(url);
      const blob = await res.blob();
      const objectUrl = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = objectUrl;
      a.download = name;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(objectUrl);
    } catch {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  };

  const downloadAllImages = async (urls: string[]) => {
    for (let i = 0; i < urls.length; i++) {
      const ts = new Date(message.timestamp || Date.now()).getTime() || Date.now();
      await downloadImage(urls[i], `crablet-image-${ts}-${i + 1}.png`);
      await new Promise((resolve) => setTimeout(resolve, 120));
    }
  };
  
  const handleSaveEdit = () => {
    if (message.id && onEdit && editValue.trim() !== '') {
        onEdit(message.id, editValue);
        setIsEditing(false);
    }
  };

  const startEditing = () => {
      if (typeof message.content === 'string') {
          setEditValue(message.content);
          setIsEditing(true);
      }
  };

  const renderContent = () => {
    if (isEditing) {
        return (
            <div className="flex flex-col gap-2 w-full">
                <textarea
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    className="w-full bg-white/10 text-inherit border border-white/20 rounded p-2 min-h-[100px] focus:outline-none focus:ring-1 focus:ring-white/30 resize-y"
                />
                <div className="flex justify-end gap-2">
                    <button onClick={() => setIsEditing(false)} className="p-1 hover:bg-white/10 rounded" title="Cancel">
                        <X className="w-4 h-4" />
                    </button>
                    <button onClick={handleSaveEdit} className="p-1 hover:bg-white/10 rounded" title="Save">
                        <Check className="w-4 h-4" />
                    </button>
                </div>
            </div>
        );
    }

    if (typeof message.content === 'string') {
      return <ReactMarkdown components={{
        code({ node, inline, className, children, ...props }: any) {
          const match = /language-(\w+)/.exec(className || '');
          return !inline && match ? (
            <CodeBlock language={match[1]} value={String(children).replace(/\n$/, '')} {...props} />
          ) : (
            <code className={cn(className, "bg-gray-200 dark:bg-gray-800 rounded px-1 py-0.5 text-sm font-mono")} {...props}>
              {children}
            </code>
          );
        }
      }}>{message.content}</ReactMarkdown>;
    }
    
    const textParts = message.content.filter((part) => part.type === 'text');
    const imageParts = message.content.filter((part) => part.type === 'image_url');
    const imageUrls = imageParts.map((part) => part.image_url.url).filter(Boolean);
    return (
      <>
        {textParts.map((part, index) => (
          <ReactMarkdown key={`text-${index}`}>{part.text}</ReactMarkdown>
        ))}
        {imageUrls.length > 0 && (
          <div className="mt-2 space-y-2">
            {imageUrls.length > 1 && (
              <div className="flex justify-end">
                <button
                  onClick={() => downloadAllImages(imageUrls)}
                  className="inline-flex items-center gap-1 text-xs px-2 py-1 rounded-md border border-zinc-300 dark:border-zinc-600 hover:bg-zinc-100 dark:hover:bg-zinc-800"
                >
                  <Download className="w-3.5 h-3.5" />
                  下载全部
                </button>
              </div>
            )}
            <div className={cn("grid gap-2", imageUrls.length === 1 ? "grid-cols-1 max-w-sm" : "grid-cols-2")}>
              {imageUrls.map((url, index) => {
                const ts = new Date(message.timestamp || Date.now()).getTime() || Date.now();
                return (
                  <div key={`img-${index}`} className="relative group/image">
                    <img src={url} alt={`Generated ${index + 1}`} className="w-full rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 object-cover" />
                    <button
                      onClick={() => downloadImage(url, `crablet-image-${ts}-${index + 1}.png`)}
                      className="absolute top-2 right-2 opacity-0 group-hover/image:opacity-100 transition-opacity inline-flex items-center gap-1 text-xs px-2 py-1 rounded-md border border-zinc-300 dark:border-zinc-600 bg-white/90 dark:bg-zinc-900/90 hover:bg-white dark:hover:bg-zinc-900"
                    >
                      <Download className="w-3.5 h-3.5" />
                      下载
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </>
    );
  };

  const knowledgeJump = (source: string, snippet?: string) =>
    `/knowledge?q=${encodeURIComponent(source)}&source=${encodeURIComponent(source)}${snippet ? `&snippet=${encodeURIComponent(snippet.slice(0, 160))}` : ''}`;

  return (
    <div className={cn("flex w-full gap-4 max-w-4xl mx-auto group", isUser ? "flex-row-reverse" : "flex-row")}>
      <div className={cn(
        "w-8 h-8 rounded-full flex items-center justify-center shrink-0 shadow-sm mt-1",
        isUser 
            ? "bg-zinc-200 dark:bg-zinc-800" 
            : "bg-gradient-to-br from-blue-600 to-indigo-600 shadow-lg shadow-blue-500/20"
      )}>
        {isUser ? (
            <User className="w-5 h-5 text-zinc-600 dark:text-zinc-300" />
        ) : (
            <Bot className="w-5 h-5 text-white" />
        )}
      </div>

      <div className={cn(
        "flex-1 min-w-0 rounded-2xl px-6 py-4 shadow-sm relative transition-all duration-200",
        isUser
          ? "bg-blue-600 text-white rounded-tr-sm"
          : "bg-zinc-100 text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100 border border-zinc-200 dark:border-zinc-700 shadow-md shadow-zinc-200/50 dark:shadow-black/20 rounded-tl-sm"
      )}>
        {/* User Actions */}
        {isUser && !isEditing && (
            <div className="absolute top-2 left-2 opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
                <button onClick={startEditing} className="p-1 hover:bg-white/20 rounded text-white/70 hover:text-white" title="Edit">
                    <Pencil className="w-3.5 h-3.5" />
                </button>
                <button onClick={() => message.id && onDelete?.(message.id)} className="p-1 hover:bg-white/20 rounded text-white/70 hover:text-white" title="Delete">
                    <Trash2 className="w-3.5 h-3.5" />
                </button>
            </div>
        )}

        {/* Assistant Actions */}
        {!isUser && (
            <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
                <button 
                    onClick={handleCopy}
                    className="p-1.5 hover:bg-zinc-200 dark:hover:bg-zinc-700 rounded-md text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 transition-colors"
                    title={copied ? "Copied" : "Copy"}
                >
                    {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
                {/* Optional: Add Delete for Assistant too */}
                <button 
                    onClick={() => message.id && onDelete?.(message.id)}
                    className="p-1.5 hover:bg-zinc-200 dark:hover:bg-zinc-700 rounded-md text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 transition-colors"
                    title="Delete"
                >
                    <Trash2 className="w-3.5 h-3.5" />
                </button>
            </div>
        )}
        {!isUser && message.cognitiveLayer && message.cognitiveLayer !== 'unknown' && (
          <div className="mb-2">
            <span className="inline-flex items-center px-2 py-0.5 rounded-md text-[10px] font-medium bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300">
              {cognitiveLayerLabel(message.cognitiveLayer)}
            </span>
          </div>
        )}
        <div className={cn(
            "prose prose-sm max-w-none break-words leading-relaxed prose-zinc dark:prose-zinc",
            "prose-p:my-2 prose-headings:my-3",
            "prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded-md prose-code:before:content-none prose-code:after:content-none",
            isUser 
                ? "prose-p:text-white prose-headings:text-white prose-strong:text-white prose-li:text-white prose-code:text-white prose-code:bg-blue-700" 
                : "prose-p:text-zinc-800 dark:prose-p:text-zinc-100 prose-headings:text-zinc-900 dark:prose-headings:text-zinc-100 prose-strong:text-zinc-900 dark:prose-strong:text-zinc-100 prose-li:text-zinc-800 dark:prose-li:text-zinc-200 prose-code:text-zinc-900 dark:prose-code:text-zinc-100 prose-code:bg-zinc-200 dark:prose-code:bg-zinc-700"
        )}>
          {renderContent()}
        </div>
        {!isUser && Array.isArray(message.citations) && message.citations.length > 0 && (
          <div className="mt-3 space-y-2">
            <div className="text-[11px] text-zinc-500 dark:text-zinc-400 font-medium">引用来源</div>
            {message.citations.map((c, idx) => (
              <div key={`${c.source}-${idx}`} className="rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white/70 dark:bg-zinc-900/60 p-2">
                <div className="text-[11px] text-zinc-500 dark:text-zinc-400">
                  {c.source} · score {Number(c.score || 0).toFixed(3)}
                </div>
                <div className="text-xs text-zinc-700 dark:text-zinc-300 mt-1">{c.snippet}</div>
                <div className="mt-2">
                  <a href={knowledgeJump(c.source, c.snippet)} className="inline-flex text-[11px] px-2 py-1 rounded border border-zinc-300 dark:border-zinc-600 text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800">
                    查看来源
                  </a>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
