import React, { useState, useEffect, useRef } from 'react';
import { sanitizeHtml } from '@/utils/security';
import './MultimodalThinking.css';

// 内容块类型
export type ContentBlockType = 
  | 'text'
  | 'code'
  | 'mermaid'
  | 'markdown'
  | 'table'
  | 'image'
  | 'diff'
  | 'json'
  | 'shell'
  | 'formula';

// 内容块
export interface ContentBlock {
  id: string;
  type: ContentBlockType;
  content: string;
  language?: string;
  title?: string;
  metadata?: {
    lineCount?: number;
    filePath?: string;
    timestamp?: number;
    author?: string;
  };
}

// 思考步骤
export interface ThinkingStep {
  id: string;
  title: string;
  description: string;
  blocks: ContentBlock[];
  timestamp: number;
  duration?: number;
  status: 'pending' | 'processing' | 'completed' | 'error';
}

interface MultimodalThinkingProps {
  steps: ThinkingStep[];
  activeStepId?: string;
  onStepClick?: (step: ThinkingStep) => void;
  onBlockAction?: (action: string, block: ContentBlock) => void;
  showStepNavigator?: boolean;
  enableCodeExecution?: boolean;
  enableMermaidRender?: boolean;
}

export const MultimodalThinking: React.FC<MultimodalThinkingProps> = ({
  steps,
  activeStepId,
  onStepClick,
  onBlockAction,
  showStepNavigator = true,
  enableCodeExecution = true,
  enableMermaidRender = true,
}) => {
  const [expandedBlocks, setExpandedBlocks] = useState<Set<string>>(new Set());
  const [copiedBlock, setCopiedBlock] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<Record<string, 'preview' | 'code'>>({});
  const mermaidRef = useRef<HTMLDivElement>(null);

  // 自动展开最新步骤
  useEffect(() => {
    const completedSteps = steps.filter(s => s.status === 'completed');
    if (completedSteps.length > 0) {
      const latest = completedSteps[completedSteps.length - 1];
      setExpandedBlocks(prev => new Set([...prev, latest.id]));
    }
  }, [steps]);

  // 复制到剪贴板
  const handleCopy = async (blockId: string, content: string) => {
    try {
      await navigator.clipboard.writeText(content);
      setCopiedBlock(blockId);
      setTimeout(() => setCopiedBlock(null), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  // 切换块展开状态
  const toggleBlock = (blockId: string) => {
    setExpandedBlocks(prev => {
      const newSet = new Set(prev);
      if (newSet.has(blockId)) {
        newSet.delete(blockId);
      } else {
        newSet.add(blockId);
      }
      return newSet;
    });
  };

  // 渲染代码块
  const renderCodeBlock = (block: ContentBlock) => {
    const isExpanded = expandedBlocks.has(block.id);
    const lines = block.content.split('\n');
    const displayLines = isExpanded ? lines : lines.slice(0, 10);
    const hasMore = lines.length > 10;

    return (
      <div className="code-block">
        <div className="code-header">
          <div className="code-meta">
            {block.language && (
              <span className="code-language">{block.language}</span>
            )}
            {block.metadata?.filePath && (
              <span className="code-file">{block.metadata.filePath}</span>
            )}
          </div>
          <div className="code-actions">
            {enableCodeExecution && block.language === 'python' && (
              <button 
                className="action-btn run"
                onClick={() => onBlockAction?.('execute', block)}
                title="运行代码"
              >
                ▶️ 运行
              </button>
            )}
            <button 
              className="action-btn copy"
              onClick={() => handleCopy(block.id, block.content)}
              title="复制"
            >
              {copiedBlock === block.id ? '✅' : '📋'}
            </button>
          </div>
        </div>
        
        <pre className="code-content">
          <code>
            {displayLines.map((line, i) => (
              <div key={i} className="code-line">
                <span className="line-number">{i + 1}</span>
                <span className="line-content">{line}</span>
              </div>
            ))}
          </code>
        </pre>
        
        {hasMore && (
          <button 
            className="expand-btn"
            onClick={() => toggleBlock(block.id)}
          >
            {isExpanded ? '收起' : `展开 ${lines.length - 10} 行更多`}
          </button>
        )}
      </div>
    );
  };

  // 渲染 Diff 块
  const renderDiffBlock = (block: ContentBlock) => {
    const lines = block.content.split('\n');
    
    return (
      <div className="diff-block">
        <div className="diff-header">
          <span className="diff-title">{block.title || '代码变更'}</span>
          <button 
            className="action-btn copy"
            onClick={() => handleCopy(block.id, block.content)}
          >
            {copiedBlock === block.id ? '✅' : '📋'}
          </button>
        </div>
        
        <div className="diff-content">
          {lines.map((line, i) => {
            const type = line.startsWith('+') ? 'add' : 
                        line.startsWith('-') ? 'remove' : 
                        line.startsWith('@') ? 'info' : 'context';
            
            return (
              <div key={i} className={`diff-line ${type}`}>
                <span className="diff-marker">{line[0] || ' '}</span>
                <span className="diff-text">{line.slice(1)}</span>
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  // 渲染 Mermaid 图表
  const renderMermaidBlock = (block: ContentBlock) => {
    const currentTab = activeTab[block.id] || 'preview';
    
    return (
      <div className="mermaid-block">
        <div className="mermaid-header">
          <span className="mermaid-title">📊 流程图</span>
          <div className="mermaid-tabs">
            <button 
              className={`tab ${currentTab === 'preview' ? 'active' : ''}`}
              onClick={() => setActiveTab(prev => ({ ...prev, [block.id]: 'preview' }))}
            >
              预览
            </button>
            <button 
              className={`tab ${currentTab === 'code' ? 'active' : ''}`}
              onClick={() => setActiveTab(prev => ({ ...prev, [block.id]: 'code' }))}
            >
              源码
            </button>
          </div>
        </div>
        
        {currentTab === 'preview' ? (
          <div 
            ref={mermaidRef}
            className="mermaid-preview"
          >
            <div className="mermaid-placeholder">
              <span>🔄 Mermaid 图表渲染中...</span>
              <pre>{block.content}</pre>
            </div>
          </div>
        ) : (
          <pre className="mermaid-code">
            <code>{block.content}</code>
          </pre>
        )}
      </div>
    );
  };

  // 渲染表格
  const renderTableBlock = (block: ContentBlock) => {
    try {
      const data = JSON.parse(block.content);
      const columns = Object.keys(data[0] || {});
      
      return (
        <div className="table-block">
          <div className="table-header">
            <span className="table-title">{block.title || '数据表格'}</span>
            <button 
              className="action-btn export"
              onClick={() => onBlockAction?.('export_csv', block)}
            >
              📥 导出 CSV
            </button>
          </div>
          
          <div className="table-wrapper">
            <table>
              <thead>
                <tr>
                  {columns.map(col => (
                    <th key={col}>{col}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {data.map((row: any, i: number) => (
                  <tr key={i}>
                    {columns.map(col => (
                      <td key={col}>{row[col]}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      );
    } catch {
      return (
        <div className="table-block error">
          <span>❌ 无效的表格数据</span>
          <pre>{block.content}</pre>
        </div>
      );
    }
  };

  // 渲染 Markdown
  const renderMarkdownBlock = (block: ContentBlock) => {
    // 简单的 Markdown 渲染（实际项目中可使用 react-markdown）
    const rawHtml = block.content
      .replace(/^### (.*$)/gim, '<h3>$1</h3>')
      .replace(/^## (.*$)/gim, '<h2>$1</h2>')
      .replace(/^# (.*$)/gim, '<h1>$1</h1>')
      .replace(/\*\*(.*)\*\*/gim, '<strong>$1</strong>')
      .replace(/\*(.*)\*/gim, '<em>$1</em>')
      .replace(/`([^`]+)`/gim, '<code>$1</code>')
      .replace(/\n/gim, '<br />');

    // Sanitize to prevent XSS — strip dangerous tags while preserving safe formatting
    const html = sanitizeHtml(rawHtml);
    
    return (
      <div 
        className="markdown-block"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    );
  };

  // 根据类型渲染内容块
  const renderBlock = (block: ContentBlock) => {
    switch (block.type) {
      case 'code':
        return renderCodeBlock(block);
      case 'diff':
        return renderDiffBlock(block);
      case 'mermaid':
        return renderMermaidBlock(block);
      case 'table':
        return renderTableBlock(block);
      case 'markdown':
        return renderMarkdownBlock(block);
      case 'json':
        return (
          <pre className="json-block">
            <code>{JSON.stringify(JSON.parse(block.content), null, 2)}</code>
          </pre>
        );
      case 'shell':
        return (
          <div className="shell-block">
            <div className="shell-header">
              <span>💻 Terminal</span>
            </div>
            <pre className="shell-content">
              <code>{block.content}</code>
            </pre>
          </div>
        );
      default:
        return (
          <div className="text-block">
            <p>{block.content}</p>
          </div>
        );
    }
  };

  // 获取步骤状态图标
  const getStatusIcon = (status: ThinkingStep['status']) => {
    switch (status) {
      case 'pending': return '⏳';
      case 'processing': return '⚡';
      case 'completed': return '✅';
      case 'error': return '❌';
      default: return '⏳';
    }
  };

  return (
    <div className="multimodal-thinking">
      {/* 步骤导航器 */}
      {showStepNavigator && (
        <div className="step-navigator">
          <div className="navigator-header">
            <span className="navigator-title">📋 思考步骤</span>
            <span className="step-count">{steps.length} 步</span>
          </div>
          
          <div className="step-list">
            {steps.map((step, index) => (
              <button
                key={step.id}
                className={`step-item ${step.id === activeStepId ? 'active' : ''} ${step.status}`}
                onClick={() => onStepClick?.(step)}
              >
                <span className="step-number">{index + 1}</span>
                <div className="step-info">
                  <span className="step-title">{step.title}</span>
                  <span className="step-meta">
                    {getStatusIcon(step.status)} 
                    {step.duration && ` · ${step.duration}ms`}
                    {step.blocks.length > 0 && ` · ${step.blocks.length} 块内容`}
                  </span>
                </div>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* 内容展示区 */}
      <div className="thinking-content">
        {steps.map(step => (
          <div 
            key={step.id}
            className={`thinking-step ${step.status} ${step.id === activeStepId ? 'active' : ''}`}
          >
            <div className="step-header">
              <div className="step-title-wrapper">
                <span className="step-status-icon">{getStatusIcon(step.status)}</span>
                <h4 className="step-title">{step.title}</h4>
              </div>
              <span className="step-description">{step.description}</span>
            </div>
            
            <div className="step-blocks">
              {step.blocks.map(block => (
                <div key={block.id} className={`content-block ${block.type}`}>
                  {renderBlock(block)}
                </div>
              ))}
            </div>
          </div>
        ))}
        
        {steps.length === 0 && (
          <div className="empty-state">
            <span className="empty-icon">📝</span>
            <span className="empty-text">暂无思考内容</span>
          </div>
        )}
      </div>
    </div>
  );
};

export default MultimodalThinking;
