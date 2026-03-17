import React, { useState, useCallback } from 'react';
import { 
  ThoughtGraphViewer, 
  ThinkingStream, 
  SmartSuggestions,
  ThinkingIntervention,
  MultimodalThinking,
  ThinkingAnalytics,
  ThinkingShare,
} from './index';
import type { 
  ThoughtGraph,
  ThinkingToken,
  ThinkingMetrics,
  ThinkingPhase,
  Suggestion,
  QuickAction,
  InterventionRequest,
  ContentBlock,
  ThinkingStep,
  ThinkingSession,
  Annotation,
  ShareRecord,
  ShareOptions,
} from './index';
import './CognitiveEnhancementPanel.css';

// 面板视图模式
type PanelView = 'graph' | 'stream' | 'multimodal' | 'analytics' | 'share';

// 认知负载状态
interface CognitiveLoad {
  system1: number; // 0-100
  system2: number;
  system3: number;
}

interface CognitiveEnhancementPanelProps {
  // 思维图谱数据
  thoughtGraph?: ThoughtGraph;
  
  // 实时思考流数据
  thinkingTokens?: ThinkingToken[];
  thinkingPhase?: ThinkingPhase;
  thinkingMetrics?: ThinkingMetrics;
  isThinkingPaused?: boolean;
  
  // 建议数据
  suggestions?: Suggestion[];
  quickActions?: QuickAction[];
  
  // 干预数据
  isInterventionActive?: boolean;
  currentNodeId?: string;
  branchOptions?: Array<{ id: string; label: string; description: string; confidence: number }>;
  
  // 多模态数据
  thinkingSteps?: ThinkingStep[];
  
  // 分析数据
  sessions?: ThinkingSession[];
  
  // 分享数据
  annotations?: Annotation[];
  shareRecords?: ShareRecord[];
  
  // 回调函数
  onNodeClick?: (nodeId: string) => void;
  onSuggestionClick?: (suggestion: Suggestion) => void;
  onQuickActionClick?: (action: QuickAction) => void;
  onIntervene?: (intervention: InterventionRequest) => void;
  onToolConfirm?: (toolName: string, params: any, confirmed: boolean) => void;
  onStepClick?: (step: ThinkingStep) => void;
  onBlockAction?: (action: string, block: ContentBlock) => void;
  onSessionSelect?: (sessionId: string) => void;
  onShare?: (options: ShareOptions) => Promise<ShareRecord>;
  onAddAnnotation?: (annotation: Omit<Annotation, 'id' | 'timestamp' | 'replies'>) => void;
  onReplyAnnotation?: (parentId: string, reply: Omit<Annotation, 'id' | 'timestamp' | 'replies'>) => void;
  onExport?: (format: 'pdf' | 'markdown' | 'html' | 'json' | 'csv') => Promise<void>;
  onThinkingPause?: () => void;
  onThinkingResume?: () => void;
  onSpeedChange?: (speed: number) => void;
  
  // 用户和配置
  currentUser?: string;
  thinkingId?: string;
  thinkingTitle?: string;
  
  // 认知负载
  cognitiveLoad?: CognitiveLoad;
}

export const CognitiveEnhancementPanel: React.FC<CognitiveEnhancementPanelProps> = ({
  thoughtGraph,
  thinkingTokens = [],
  thinkingPhase = 'initializing',
  thinkingMetrics,
  isThinkingPaused = false,
  suggestions = [],
  quickActions = [],
  isInterventionActive = false,
  currentNodeId,
  branchOptions,
  thinkingSteps = [],
  sessions = [],
  annotations = [],
  shareRecords = [],
  onNodeClick,
  onSuggestionClick,
  onQuickActionClick,
  onIntervene,
  onToolConfirm,
  onStepClick,
  onBlockAction,
  onSessionSelect,
  onShare,
  onAddAnnotation,
  onReplyAnnotation,
  onExport,
  onThinkingPause,
  onThinkingResume,
  onSpeedChange,
  currentUser = 'User',
  thinkingId = '',
  thinkingTitle = '',
  cognitiveLoad = { system1: 0, system2: 0, system3: 0 },
}) => {
  const [activeView, setActiveView] = useState<PanelView>('graph');
  const [showSuggestions, setShowSuggestions] = useState(true);
  const [showIntervention, setShowIntervention] = useState(true);

  // 处理节点点击
  const handleNodeClick = useCallback((node: any) => {
    onNodeClick?.(node.id);
  }, [onNodeClick]);

  // 渲染认知负载指示器
  const renderCognitiveLoad = () => (
    <div className="cognitive-load-indicator">
      <div className="load-bar">
        <span className="load-label">S1</span>
        <div className="load-track">
          <div 
            className="load-fill system1"
            style={{ width: `${cognitiveLoad.system1}%` }}
          />
        </div>
        <span className="load-value">{cognitiveLoad.system1}%</span>
      </div>
      <div className="load-bar">
        <span className="load-label">S2</span>
        <div className="load-track">
          <div 
            className="load-fill system2"
            style={{ width: `${cognitiveLoad.system2}%` }}
          />
        </div>
        <span className="load-value">{cognitiveLoad.system2}%</span>
      </div>
      <div className="load-bar">
        <span className="load-label">S3</span>
        <div className="load-track">
          <div 
            className="load-fill system3"
            style={{ width: `${cognitiveLoad.system3}%` }}
          />
        </div>
        <span className="load-value">{cognitiveLoad.system3}%</span>
      </div>
    </div>
  );

  // 渲染主内容区
  const renderMainContent = () => {
    switch (activeView) {
      case 'graph':
        return thoughtGraph ? (
          <ThoughtGraphViewer 
            graph={thoughtGraph}
            onNodeClick={handleNodeClick}
            showMiniMap={true}
            showStats={true}
          />
        ) : (
          <div className="empty-view">
            <span>🧠 暂无思维图谱数据</span>
          </div>
        );
        
      case 'stream':
        return (
          <ThinkingStream
            tokens={thinkingTokens}
            phase={thinkingPhase}
            metrics={thinkingMetrics}
            isPaused={isThinkingPaused}
            onPause={onThinkingPause}
            onResume={onThinkingResume}
            onSpeedChange={onSpeedChange}
            showMetrics={true}
          />
        );
        
      case 'multimodal':
        return (
          <MultimodalThinking
            steps={thinkingSteps}
            onStepClick={onStepClick}
            onBlockAction={onBlockAction}
            showStepNavigator={true}
            enableCodeExecution={true}
            enableMermaidRender={true}
          />
        );
        
      case 'analytics':
        return (
          <ThinkingAnalytics
            sessions={sessions}
            onSessionSelect={onSessionSelect}
            onExport={onExport}
          />
        );
        
      case 'share':
        return (
          <ThinkingShare
            thinkingId={thinkingId}
            thinkingTitle={thinkingTitle}
            annotations={annotations}
            shareRecords={shareRecords}
            onShare={onShare!}
            onAddAnnotation={onAddAnnotation!}
            onReplyAnnotation={onReplyAnnotation!}
            onExport={onExport!}
            currentUser={currentUser}
          />
        );
        
      default:
        return null;
    }
  };

  return (
    <div className="cognitive-enhancement-panel">
      {/* 顶部工具栏 */}
      <div className="panel-header">
        <div className="view-tabs">
          <button 
            className={`tab ${activeView === 'graph' ? 'active' : ''}`}
            onClick={() => setActiveView('graph')}
            title="思维图谱"
          >
            🕸️ 图谱
          </button>
          <button 
            className={`tab ${activeView === 'stream' ? 'active' : ''}`}
            onClick={() => setActiveView('stream')}
            title="实时思考流"
          >
            🌊 流式
          </button>
          <button 
            className={`tab ${activeView === 'multimodal' ? 'active' : ''}`}
            onClick={() => setActiveView('multimodal')}
            title="多模态展示"
          >
            🎨 多模态
          </button>
          <button 
            className={`tab ${activeView === 'analytics' ? 'active' : ''}`}
            onClick={() => setActiveView('analytics')}
            title="分析统计"
          >
            📊 分析
          </button>
          <button 
            className={`tab ${activeView === 'share' ? 'active' : ''}`}
            onClick={() => setActiveView('share')}
            title="分享协作"
          >
            🔗 分享
          </button>
        </div>
        
        {renderCognitiveLoad()}
      </div>

      {/* 主内容区 */}
      <div className="panel-main">
        {renderMainContent()}
      </div>

      {/* 底部建议区 */}
      {showSuggestions && suggestions.length > 0 && activeView !== 'share' && (
        <div className="panel-suggestions">
          <SmartSuggestions
            context=""
            conversationHistory={[]}
            onSuggestionClick={onSuggestionClick || (() => {})}
            onQuickActionClick={onQuickActionClick || (() => {})}
            maxSuggestions={3}
          />
        </div>
      )}

      {/* 干预控制区 */}
      {showIntervention && isInterventionActive && activeView !== 'share' && (
        <div className="panel-intervention">
          <ThinkingIntervention
            isActive={isInterventionActive}
            currentNodeId={currentNodeId}
            onIntervene={onIntervene || (() => {})}
            onConfirmTool={onToolConfirm || (() => {})}
            branchOptions={branchOptions}
          />
        </div>
      )}
    </div>
  );
};

export default CognitiveEnhancementPanel;
