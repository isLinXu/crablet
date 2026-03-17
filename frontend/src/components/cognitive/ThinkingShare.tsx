import React, { useState, useCallback } from 'react';
import './ThinkingShare.css';

// 分享选项
export interface ShareOptions {
  includeThinking: boolean;
  includeCode: boolean;
  includeMetrics: boolean;
  isPublic: boolean;
  allowComments: boolean;
  expiresIn?: '1h' | '1d' | '7d' | '30d' | 'never';
}

// 批注
export interface Annotation {
  id: string;
  nodeId: string;
  author: string;
  avatar?: string;
  content: string;
  timestamp: number;
  replies: Annotation[];
}

// 分享记录
export interface ShareRecord {
  id: string;
  url: string;
  createdAt: number;
  viewCount: number;
  options: ShareOptions;
  title: string;
  description?: string;
}

interface ThinkingShareProps {
  thinkingId: string;
  thinkingTitle: string;
  annotations: Annotation[];
  shareRecords: ShareRecord[];
  onShare: (options: ShareOptions) => Promise<ShareRecord>;
  onAddAnnotation: (annotation: Omit<Annotation, 'id' | 'timestamp' | 'replies'>) => void;
  onReplyAnnotation: (parentId: string, reply: Omit<Annotation, 'id' | 'timestamp' | 'replies'>) => void;
  onExport: (format: 'pdf' | 'markdown' | 'html' | 'json') => Promise<void>;
  currentUser: string;
}

export const ThinkingShare: React.FC<ThinkingShareProps> = ({
  thinkingId,
  thinkingTitle,
  annotations,
  shareRecords,
  onShare,
  onAddAnnotation,
  onReplyAnnotation,
  onExport,
  currentUser,
}) => {
  const [activeTab, setActiveTab] = useState<'share' | 'annotate' | 'export' | 'history'>('share');
  const [shareOptions, setShareOptions] = useState<ShareOptions>({
    includeThinking: true,
    includeCode: true,
    includeMetrics: true,
    isPublic: false,
    allowComments: true,
    expiresIn: '7d',
  });
  const [isSharing, setIsSharing] = useState(false);
  const [newShare, setNewShare] = useState<ShareRecord | null>(null);
  const [annotationText, setAnnotationText] = useState('');
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [replyingTo, setReplyingTo] = useState<string | null>(null);
  const [replyText, setReplyText] = useState('');
  const [copiedLink, setCopiedLink] = useState(false);

  // 处理分享
  const handleShare = useCallback(async () => {
    setIsSharing(true);
    try {
      const record = await onShare(shareOptions);
      setNewShare(record);
    } finally {
      setIsSharing(false);
    }
  }, [onShare, shareOptions]);

  // 复制链接
  const handleCopyLink = useCallback(async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
      setCopiedLink(true);
      setTimeout(() => setCopiedLink(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }, []);

  // 添加批注
  const handleAddAnnotation = useCallback(() => {
    if (!annotationText.trim() || !selectedNodeId) return;
    
    onAddAnnotation({
      nodeId: selectedNodeId,
      author: currentUser,
      content: annotationText,
    });
    
    setAnnotationText('');
    setSelectedNodeId(null);
  }, [annotationText, selectedNodeId, currentUser, onAddAnnotation]);

  // 回复批注
  const handleReply = useCallback((parentId: string) => {
    if (!replyText.trim()) return;
    
    onReplyAnnotation(parentId, {
      nodeId: '',
      author: currentUser,
      content: replyText,
    });
    
    setReplyText('');
    setReplyingTo(null);
  }, [replyText, currentUser, onReplyAnnotation]);

  // 格式化时间
  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleString();
  };

  // 格式化过期时间
  const formatExpiry = (expiresIn?: string) => {
    const labels: Record<string, string> = {
      '1h': '1 小时',
      '1d': '1 天',
      '7d': '7 天',
      '30d': '30 天',
      'never': '永不过期',
    };
    return labels[expiresIn || '7d'];
  };

  // 渲染分享面板
  const renderSharePanel = () => (
    <div className="share-panel">
      {newShare ? (
        <div className="share-success">
          <div className="success-icon">✅</div>
          <h4>分享链接已创建</h4>
          
          <div className="share-link-box">
            <input 
              type="text" 
              value={newShare.url}
              readOnly
              className="share-link-input"
            />
            <button 
              className="btn-copy"
              onClick={() => handleCopyLink(newShare.url)}
            >
              {copiedLink ? '已复制' : '📋 复制'}
            </button>
          </div>
          
          <div className="share-details">
            <div className="detail-item">
              <span className="detail-label">过期时间:</span>
              <span className="detail-value">{formatExpiry(newShare.options.expiresIn)}</span>
            </div>
            <div className="detail-item">
              <span className="detail-label">访问权限:</span>
              <span className="detail-value">{newShare.options.isPublic ? '公开' : '私有'}</span>
            </div>
          </div>
          
          <div className="share-actions">
            <button 
              className="btn-secondary"
              onClick={() => setNewShare(null)}
            >
              创建新链接
            </button>
            <a 
              href={newShare.url}
              target="_blank"
              rel="noopener noreferrer"
              className="btn-primary"
            >
              查看分享
            </a>
          </div>
        </div>
      ) : (
        <>
          <div className="share-options">
            <h4>分享选项</h4>
            
            <div className="option-group">
              <label className="option-label">包含内容</label>
              
              <label className="checkbox-item">
                <input
                  type="checkbox"
                  checked={shareOptions.includeThinking}
                  onChange={(e) => setShareOptions(prev => ({ 
                    ...prev, 
                    includeThinking: e.target.checked 
                  }))}
                />
                <span className="checkmark"></span>
                <span className="checkbox-text">思考过程</span>
              </label>
              
              <label className="checkbox-item">
                <input
                  type="checkbox"
                  checked={shareOptions.includeCode}
                  onChange={(e) => setShareOptions(prev => ({ 
                    ...prev, 
                    includeCode: e.target.checked 
                  }))}
                />
                <span className="checkmark"></span>
                <span className="checkbox-text">代码块</span>
              </label>
              
              <label className="checkbox-item">
                <input
                  type="checkbox"
                  checked={shareOptions.includeMetrics}
                  onChange={(e) => setShareOptions(prev => ({ 
                    ...prev, 
                    includeMetrics: e.target.checked 
                  }))}
                />
                <span className="checkmark"></span>
                <span className="checkbox-text">质量指标</span>
              </label>
            </div>
            
            <div className="option-group">
              <label className="option-label">权限设置</label>
              
              <label className="checkbox-item">
                <input
                  type="checkbox"
                  checked={shareOptions.isPublic}
                  onChange={(e) => setShareOptions(prev => ({ 
                    ...prev, 
                    isPublic: e.target.checked 
                  }))}
                />
                <span className="checkmark"></span>
                <span className="checkbox-text">公开访问（无需登录）</span>
              </label>
              
              <label className="checkbox-item">
                <input
                  type="checkbox"
                  checked={shareOptions.allowComments}
                  onChange={(e) => setShareOptions(prev => ({ 
                    ...prev, 
                    allowComments: e.target.checked 
                  }))}
                />
                <span className="checkmark"></span>
                <span className="checkbox-text">允许评论</span>
              </label>
            </div>
            
            <div className="option-group">
              <label className="option-label">过期时间</label>
              <select
                value={shareOptions.expiresIn}
                onChange={(e) => setShareOptions(prev => ({ 
                  ...prev, 
                  expiresIn: e.target.value as any 
                }))}
                className="select-input"
              >
                <option value="1h">1 小时</option>
                <option value="1d">1 天</option>
                <option value="7d">7 天</option>
                <option value="30d">30 天</option>
                <option value="never">永不过期</option>
              </select>
            </div>
          </div>
          
          <button 
            className="btn-share"
            onClick={handleShare}
            disabled={isSharing}
          >
            {isSharing ? '⏳ 创建中...' : '🔗 生成分享链接'}
          </button>
        </>
      )}
    </div>
  );

  // 渲染批注面板
  const renderAnnotatePanel = () => (
    <div className="annotate-panel">
      <div className="annotation-input">
        <h4>添加批注</h4>
        
        <select
          value={selectedNodeId || ''}
          onChange={(e) => setSelectedNodeId(e.target.value || null)}
          className="select-input"
        >
          <option value="">选择思考节点...</option>
          <option value="node-1">步骤 1: 初始化</option>
          <option value="node-2">步骤 2: 分析</option>
          <option value="node-3">步骤 3: 推理</option>
        </select>
        
        <textarea
          value={annotationText}
          onChange={(e) => setAnnotationText(e.target.value)}
          placeholder="写下你的批注..."
          rows={3}
          className="annotation-textarea"
        />
        
        <button 
          className="btn-primary"
          onClick={handleAddAnnotation}
          disabled={!annotationText.trim() || !selectedNodeId}
        >
          💬 添加批注
        </button>
      </div>
      
      <div className="annotations-list">
        <h4>批注 ({annotations.length})</h4>
        
        {annotations.map(annotation => (
          <div key={annotation.id} className="annotation-item">
            <div className="annotation-header">
              <div className="annotation-author">
                <span className="author-avatar">
                  {annotation.avatar || annotation.author[0].toUpperCase()}
                </span>
                <span className="author-name">{annotation.author}</span>
              </div>
              <span className="annotation-time">
                {formatTime(annotation.timestamp)}
              </span>
            </div>
            
            <div className="annotation-content">
              <span className="annotation-node">📍 节点: {annotation.nodeId}</span>
              <p>{annotation.content}</p>
            </div>
            
            <div className="annotation-actions">
              <button 
                className="btn-reply"
                onClick={() => setReplyingTo(annotation.id)}
              >
                回复
              </button>
            </div>
            
            {replyingTo === annotation.id && (
              <div className="reply-input">
                <textarea
                  value={replyText}
                  onChange={(e) => setReplyText(e.target.value)}
                  placeholder="写下你的回复..."
                  rows={2}
                  className="annotation-textarea"
                />
                <div className="reply-actions">
                  <button 
                    className="btn-secondary"
                    onClick={() => setReplyingTo(null)}
                  >
                    取消
                  </button>
                  <button 
                    className="btn-primary"
                    onClick={() => handleReply(annotation.id)}
                    disabled={!replyText.trim()}
                  >
                    发送
                  </button>
                </div>
              </div>
            )}
            
            {annotation.replies.length > 0 && (
              <div className="replies-list">
                {annotation.replies.map(reply => (
                  <div key={reply.id} className="reply-item">
                    <div className="reply-header">
                      <span className="author-name">{reply.author}</span>
                      <span className="reply-time">
                        {formatTime(reply.timestamp)}
                      </span>
                    </div>
                    <p>{reply.content}</p>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
        
        {annotations.length === 0 && (
          <div className="empty-annotations">
            <span>📝 暂无批注</span>
          </div>
        )}
      </div>
    </div>
  );

  // 渲染导出面板
  const renderExportPanel = () => (
    <div className="export-panel">
      <h4>导出思考过程</h4>
      
      <div className="export-options">
        <button 
          className="export-option"
          onClick={() => onExport('pdf')}
        >
          <span className="export-icon">📄</span>
          <div className="export-info">
            <span className="export-name">PDF 文档</span>
            <span className="export-desc">适合打印和分享</span>
          </div>
        </button>
        
        <button 
          className="export-option"
          onClick={() => onExport('markdown')}
        >
          <span className="export-icon">📝</span>
          <div className="export-info">
            <span className="export-name">Markdown</span>
            <span className="export-desc">适合编辑和版本控制</span>
          </div>
        </button>
        
        <button 
          className="export-option"
          onClick={() => onExport('html')}
        >
          <span className="export-icon">🌐</span>
          <div className="export-info">
            <span className="export-name">HTML 页面</span>
            <span className="export-desc">交互式网页格式</span>
          </div>
        </button>
        
        <button 
          className="export-option"
          onClick={() => onExport('json')}
        >
          <span className="export-icon">📊</span>
          <div className="export-info">
            <span className="export-name">JSON 数据</span>
            <span className="export-desc">结构化数据格式</span>
          </div>
        </button>
      </div>
    </div>
  );

  // 渲染历史面板
  const renderHistoryPanel = () => (
    <div className="history-panel">
      <h4>分享历史</h4>
      
      <div className="share-history-list">
        {shareRecords.map(record => (
          <div key={record.id} className="history-item">
            <div className="history-header">
              <span className="history-title">{record.title}</span>
              <span className="history-views">👁️ {record.viewCount} 次查看</span>
            </div>
            
            <div className="history-meta">
              <span>创建于 {formatTime(record.createdAt)}</span>
              <span className={`history-visibility ${record.options.isPublic ? 'public' : 'private'}`}>
                {record.options.isPublic ? '🌐 公开' : '🔒 私有'}
              </span>
            </div>
            
            <div className="history-link">
              <input 
                type="text" 
                value={record.url}
                readOnly
                className="history-link-input"
              />
              <button 
                className="btn-copy-sm"
                onClick={() => handleCopyLink(record.url)}
              >
                复制
              </button>
            </div>
          </div>
        ))}
        
        {shareRecords.length === 0 && (
          <div className="empty-history">
            <span>📭 暂无分享记录</span>
          </div>
        )}
      </div>
    </div>
  );

  return (
    <div className="thinking-share">
      {/* 标签页 */}
      <div className="share-tabs">
        <button 
          className={`tab ${activeTab === 'share' ? 'active' : ''}`}
          onClick={() => setActiveTab('share')}
        >
          🔗 分享
        </button>
        <button 
          className={`tab ${activeTab === 'annotate' ? 'active' : ''}`}
          onClick={() => setActiveTab('annotate')}
        >
          💬 批注 ({annotations.length})
        </button>
        <button 
          className={`tab ${activeTab === 'export' ? 'active' : ''}`}
          onClick={() => setActiveTab('export')}
        >
          📥 导出
        </button>
        <button 
          className={`tab ${activeTab === 'history' ? 'active' : ''}`}
          onClick={() => setActiveTab('history')}
        >
          📜 历史
        </button>
      </div>

      {/* 内容区 */}
      <div className="share-content">
        {activeTab === 'share' && renderSharePanel()}
        {activeTab === 'annotate' && renderAnnotatePanel()}
        {activeTab === 'export' && renderExportPanel()}
        {activeTab === 'history' && renderHistoryPanel()}
      </div>
    </div>
  );
};

export default ThinkingShare;
