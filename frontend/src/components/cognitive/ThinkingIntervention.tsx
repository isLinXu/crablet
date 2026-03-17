import React, { useState, useCallback } from 'react';
import './ThinkingIntervention.css';

// 干预类型
export type InterventionType = 'correct' | 'guide' | 'skip' | 'branch' | 'pause' | 'abort';

// 干预请求
export interface InterventionRequest {
  id: string;
  type: InterventionType;
  nodeId?: string;
  message: string;
  timestamp: number;
  userInput?: string;
}

// 分支选项
export interface BranchOption {
  id: string;
  label: string;
  description: string;
  confidence: number;
  preview?: string;
}

interface ThinkingInterventionProps {
  isActive: boolean;
  currentNodeId?: string;
  currentThought?: string;
  branchOptions?: BranchOption[];
  onIntervene: (intervention: InterventionRequest) => void;
  onConfirmTool: (toolName: string, params: any, confirmed: boolean) => void;
  pendingTool?: {
    name: string;
    params: any;
    description: string;
  };
  canBacktrack?: boolean;
  backtrackHistory?: string[];
}

export const ThinkingIntervention: React.FC<ThinkingInterventionProps> = ({
  isActive,
  currentNodeId,
  currentThought,
  branchOptions,
  onIntervene,
  onConfirmTool,
  pendingTool,
  canBacktrack = false,
  backtrackHistory = [],
}) => {
  const [activeTab, setActiveTab] = useState<InterventionType | null>(null);
  const [correctionText, setCorrectionText] = useState('');
  const [guidanceText, setGuidanceText] = useState('');
  const [selectedBranch, setSelectedBranch] = useState<string | null>(null);
  const [showHistory, setShowHistory] = useState(false);

  const handleIntervene = useCallback((type: InterventionType, input?: string) => {
    const intervention: InterventionRequest = {
      id: `intervention-${Date.now()}`,
      type,
      nodeId: currentNodeId,
      message: getInterventionMessage(type),
      timestamp: Date.now(),
      userInput: input,
    };
    
    onIntervene(intervention);
    
    // 重置状态
    setActiveTab(null);
    setCorrectionText('');
    setGuidanceText('');
    setSelectedBranch(null);
  }, [currentNodeId, onIntervene]);

  const getInterventionMessage = (type: InterventionType): string => {
    const messages: Record<InterventionType, string> = {
      'correct': '用户提供了纠正信息',
      'guide': '用户提供了引导',
      'skip': '用户要求跳过当前步骤',
      'branch': '用户选择了分支路径',
      'pause': '用户暂停了思考过程',
      'abort': '用户中止了思考过程',
    };
    return messages[type];
  };

  if (!isActive) return null;

  return (
    <div className="thinking-intervention">
      {/* 工具调用确认弹窗 */}
      {pendingTool && (
        <div className="tool-confirmation-overlay">
          <div className="tool-confirmation-modal">
            <div className="modal-header">
              <span className="modal-icon">🔧</span>
              <h4>工具调用确认</h4>
            </div>
            
            <div className="modal-content">
              <p className="tool-description">{pendingTool.description}</p>
              
              <div className="tool-details">
                <div className="tool-name">
                  <span className="label">工具:</span>
                  <span className="value">{pendingTool.name}</span>
                </div>
                
                <div className="tool-params">
                  <span className="label">参数:</span>
                  <pre>{JSON.stringify(pendingTool.params, null, 2)}</pre>
                </div>
              </div>
            </div>
            
            <div className="modal-actions">
              <button 
                className="btn-secondary"
                onClick={() => onConfirmTool(pendingTool.name, pendingTool.params, false)}
              >
                拒绝
              </button>
              <button 
                className="btn-primary"
                onClick={() => onConfirmTool(pendingTool.name, pendingTool.params, true)}
              >
                确认执行
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 分支选择面板 */}
      {branchOptions && branchOptions.length > 0 && !activeTab && (
        <div className="branch-selection-panel">
          <div className="panel-header">
            <span className="panel-icon">🎯</span>
            <h4>选择推理方向</h4>
            <span className="panel-subtitle">系统识别到多个可能的解决路径</span>
          </div>
          
          <div className="branch-options">
            {branchOptions.map(option => (
              <button
                key={option.id}
                className={`branch-option ${selectedBranch === option.id ? 'selected' : ''}`}
                onClick={() => setSelectedBranch(option.id)}
              >
                <div className="branch-header">
                  <span className="branch-label">{option.label}</span>
                  <div className="branch-confidence">
                    <div 
                      className="confidence-bar"
                      style={{ width: `${option.confidence * 100}%` }}
                    />
                    <span>{(option.confidence * 100).toFixed(0)}%</span>
                  </div>
                </div>
                
                <p className="branch-description">{option.description}</p>
                
                {option.preview && (
                  <div className="branch-preview">
                    <span className="preview-label">预览:</span>
                    <span className="preview-text">{option.preview}</span>
                  </div>
                )}
              </button>
            ))}
          </div>
          
          <div className="branch-actions">
            <button 
              className="btn-secondary"
              onClick={() => handleIntervene('skip')}
            >
              让系统自动选择
            </button>
            <button 
              className="btn-primary"
              disabled={!selectedBranch}
              onClick={() => handleIntervene('branch', selectedBranch!)}
            >
              确认选择
            </button>
          </div>
        </div>
      )}

      {/* 干预工具栏 */}
      <div className="intervention-toolbar">
        <div className="toolbar-header">
          <span className="toolbar-icon">⚡</span>
          <span className="toolbar-title">干预控制</span>
          {currentThought && (
            <span className="current-thought-preview">
              当前: {currentThought.slice(0, 40)}...
            </span>
          )}
        </div>
        
        <div className="toolbar-actions">
          <button
            className={`intervention-btn ${activeTab === 'correct' ? 'active' : ''}`}
            onClick={() => setActiveTab(activeTab === 'correct' ? null : 'correct')}
            title="纠正"
          >
            <span className="btn-icon">✏️</span>
            <span className="btn-label">纠正</span>
          </button>
          
          <button
            className={`intervention-btn ${activeTab === 'guide' ? 'active' : ''}`}
            onClick={() => setActiveTab(activeTab === 'guide' ? null : 'guide')}
            title="引导"
          >
            <span className="btn-icon">🧭</span>
            <span className="btn-label">引导</span>
          </button>
          
          {canBacktrack && (
            <button
              className="intervention-btn"
              onClick={() => setShowHistory(!showHistory)}
              title="回溯"
            >
              <span className="btn-icon">⏪</span>
              <span className="btn-label">回溯</span>
            </button>
          )}
          
          <button
            className="intervention-btn"
            onClick={() => handleIntervene('skip')}
            title="跳过"
          >
            <span className="btn-icon">⏭️</span>
            <span className="btn-label">跳过</span>
          </button>
          
          <button
            className="intervention-btn danger"
            onClick={() => handleIntervene('abort')}
            title="中止"
          >
            <span className="btn-icon">⏹️</span>
            <span className="btn-label">中止</span>
          </button>
        </div>
      </div>

      {/* 纠正输入面板 */}
      {activeTab === 'correct' && (
        <div className="intervention-panel">
          <div className="panel-header">
            <span className="panel-icon">✏️</span>
            <h4>提供纠正信息</h4>
          </div>
          
          <textarea
            className="intervention-input"
            placeholder="指出错误并提供正确的信息..."
            value={correctionText}
            onChange={(e) => setCorrectionText(e.target.value)}
            rows={3}
          />
          
          <div className="panel-actions">
            <button 
              className="btn-secondary"
              onClick={() => setActiveTab(null)}
            >
              取消
            </button>
            <button 
              className="btn-primary"
              disabled={!correctionText.trim()}
              onClick={() => handleIntervene('correct', correctionText)}
            >
              提交纠正
            </button>
          </div>
        </div>
      )}

      {/* 引导输入面板 */}
      {activeTab === 'guide' && (
        <div className="intervention-panel">
          <div className="panel-header">
            <span className="panel-icon">🧭</span>
            <h4>提供引导方向</h4>
          </div>
          
          <div className="guidance-presets">
            <button 
              className="preset-btn"
              onClick={() => setGuidanceText('请专注于核心问题，避免过度发散')}
            >
              聚焦核心
            </button>
            <button 
              className="preset-btn"
              onClick={() => setGuidanceText('请考虑更多边界情况和异常处理')}
            >
              考虑边界
            </button>
            <button 
              className="preset-btn"
              onClick={() => setGuidanceText('请简化解释，使用更通俗的语言')}
            >
              简化说明
            </button>
            <button 
              className="preset-btn"
              onClick={() => setGuidanceText('请深入技术细节')}
            >
              深入技术
            </button>
          </div>
          
          <textarea
            className="intervention-input"
            placeholder="或者输入自定义引导..."
            value={guidanceText}
            onChange={(e) => setGuidanceText(e.target.value)}
            rows={3}
          />
          
          <div className="panel-actions">
            <button 
              className="btn-secondary"
              onClick={() => setActiveTab(null)}
            >
              取消
            </button>
            <button 
              className="btn-primary"
              disabled={!guidanceText.trim()}
              onClick={() => handleIntervene('guide', guidanceText)}
            >
              应用引导
            </button>
          </div>
        </div>
      )}

      {/* 回溯历史面板 */}
      {showHistory && canBacktrack && (
        <div className="intervention-panel">
          <div className="panel-header">
            <span className="panel-icon">⏪</span>
            <h4>回溯到之前的步骤</h4>
          </div>
          
          <div className="backtrack-list">
            {backtrackHistory.map((step, index) => (
              <button
                key={index}
                className="backtrack-item"
                onClick={() => {
                  handleIntervene('guide', `backtrack_to:${index}`);
                  setShowHistory(false);
                }}
              >
                <span className="backtrack-index">{index + 1}</span>
                <span className="backtrack-text">{step}</span>
              </button>
            ))}
          </div>
          
          <div className="panel-actions">
            <button 
              className="btn-secondary"
              onClick={() => setShowHistory(false)}
            >
              关闭
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

export default ThinkingIntervention;
