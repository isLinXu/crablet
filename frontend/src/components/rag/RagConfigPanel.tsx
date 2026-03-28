import { useState } from 'react';
import { Settings, Search, Database, Network, Filter, BarChart3 } from 'lucide-react';
import { Button } from '../ui/Button';
import clsx from 'clsx';

export interface RagConfig {
  // 检索策略开关
  enableVectorSearch: boolean;
  enableKeywordSearch: boolean;
  enableGraphSearch: boolean;
  enableQueryExpansion: boolean;
  
  // 权重配置
  vectorWeight: number;
  keywordWeight: number;
  graphWeight: number;
  diversityWeight: number;
  recencyWeight: number;
  
  // 结果配置
  maxResults: number;
  minScore: number;
  enableDeduplication: boolean;
  enableMMR: boolean;
  mmrLambda: number;
  
  // 高级配置
  queryRewrite: boolean;
  subQueryDecomposition: boolean;
  intentClassification: boolean;
}

const defaultConfig: RagConfig = {
  enableVectorSearch: true,
  enableKeywordSearch: true,
  enableGraphSearch: true,
  enableQueryExpansion: true,
  
  vectorWeight: 0.4,
  keywordWeight: 0.2,
  graphWeight: 0.2,
  diversityWeight: 0.1,
  recencyWeight: 0.1,
  
  maxResults: 10,
  minScore: 0.1,
  enableDeduplication: true,
  enableMMR: true,
  mmrLambda: 0.5,
  
  queryRewrite: true,
  subQueryDecomposition: true,
  intentClassification: true,
};

interface RagConfigPanelProps {
  isOpen: boolean;
  onClose: () => void;
  onConfigChange?: (config: RagConfig) => void;
}

const ragTabs = [
  { id: 'strategies', label: '检索策略', icon: Search },
  { id: 'weights', label: '权重配置', icon: BarChart3 },
  { id: 'advanced', label: '高级选项', icon: Filter },
] as const;

export const RagConfigPanel = ({ isOpen, onClose, onConfigChange }: RagConfigPanelProps) => {
  const [config, setConfig] = useState<RagConfig>(() => {
    const saved = localStorage.getItem('crablet-rag-config');
    if (!saved) return defaultConfig;
    try {
      const parsed = JSON.parse(saved) as Partial<RagConfig>;
      return { ...defaultConfig, ...parsed };
    } catch {
      return defaultConfig;
    }
  });
  const [activeTab, setActiveTab] = useState<'strategies' | 'weights' | 'advanced'>('strategies');

  const handleSave = () => {
    localStorage.setItem('crablet-rag-config', JSON.stringify(config));
    onConfigChange?.(config);
    onClose();
  };

  const handleReset = () => {
    setConfig(defaultConfig);
    localStorage.removeItem('crablet-rag-config');
  };

  const updateConfig = (updates: Partial<RagConfig>) => {
    setConfig(prev => ({ ...prev, ...updates }));
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div className="w-full max-w-2xl max-h-[90vh] overflow-hidden bg-white dark:bg-zinc-900 rounded-2xl shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-zinc-200 dark:border-zinc-800">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-blue-100 dark:bg-blue-900/30 rounded-lg">
              <Settings className="w-5 h-5 text-blue-600 dark:text-blue-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-zinc-900 dark:text-zinc-100">RAG 检索配置</h2>
              <p className="text-sm text-zinc-500">自定义检索增强生成策略</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-lg transition-colors"
          >
            <span className="sr-only">关闭</span>
            <svg className="w-5 h-5 text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-zinc-200 dark:border-zinc-800">
          {ragTabs.map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={clsx(
                'flex items-center gap-2 px-6 py-3 text-sm font-medium transition-colors border-b-2',
                activeTab === tab.id
                  ? 'border-blue-500 text-blue-600 dark:text-blue-400'
                  : 'border-transparent text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200'
              )}
            >
              <tab.icon className="w-4 h-4" />
              {tab.label}
            </button>
          ))}
        </div>

        {/* Content */}
        <div className="p-6 overflow-y-auto max-h-[60vh]">
          {activeTab === 'strategies' && (
            <div className="space-y-4">
              <p className="text-sm text-zinc-500 mb-4">选择启用的检索策略，多策略组合可提升召回率</p>
              
              <StrategyToggle
                icon={Database}
                title="向量语义检索"
                description="基于嵌入向量相似度的语义搜索"
                enabled={config.enableVectorSearch}
                onChange={(v) => updateConfig({ enableVectorSearch: v })}
              />
              
              <StrategyToggle
                icon={Search}
                title="关键词精确匹配"
                description="基于关键词的 BM25/TF-IDF 检索"
                enabled={config.enableKeywordSearch}
                onChange={(v) => updateConfig({ enableKeywordSearch: v })}
              />
              
              <StrategyToggle
                icon={Network}
                title="知识图谱遍历"
                description="基于实体关系的图检索"
                enabled={config.enableGraphSearch}
                onChange={(v) => updateConfig({ enableGraphSearch: v })}
              />
              
              <StrategyToggle
                icon={Filter}
                title="查询扩展"
                description="使用 LLM 生成相关查询词扩展检索"
                enabled={config.enableQueryExpansion}
                onChange={(v) => updateConfig({ enableQueryExpansion: v })}
              />
            </div>
          )}

          {activeTab === 'weights' && (
            <div className="space-y-6">
              <p className="text-sm text-zinc-500 mb-4">调整不同检索策略的权重和结果配置</p>
              
              <WeightSlider
                label="向量检索权重"
                value={config.vectorWeight}
                onChange={(v) => updateConfig({ vectorWeight: v })}
                disabled={!config.enableVectorSearch}
              />
              
              <WeightSlider
                label="关键词检索权重"
                value={config.keywordWeight}
                onChange={(v) => updateConfig({ keywordWeight: v })}
                disabled={!config.enableKeywordSearch}
              />
              
              <WeightSlider
                label="图检索权重"
                value={config.graphWeight}
                onChange={(v) => updateConfig({ graphWeight: v })}
                disabled={!config.enableGraphSearch}
              />
              
              <WeightSlider
                label="多样性权重"
                value={config.diversityWeight}
                onChange={(v) => updateConfig({ diversityWeight: v })}
              />
              
              <WeightSlider
                label="时效性权重"
                value={config.recencyWeight}
                onChange={(v) => updateConfig({ recencyWeight: v })}
              />
              
              <div className="pt-4 border-t border-zinc-200 dark:border-zinc-800">
                <label className="flex items-center justify-between">
                  <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">最大结果数</span>
                  <input
                    type="number"
                    min={1}
                    max={50}
                    value={config.maxResults}
                    onChange={(e) => updateConfig({ maxResults: parseInt(e.target.value) || 10 })}
                    className="w-20 px-3 py-1.5 text-sm bg-zinc-100 dark:bg-zinc-800 border border-zinc-300 dark:border-zinc-700 rounded-lg"
                  />
                </label>
              </div>
            </div>
          )}

          {activeTab === 'advanced' && (
            <div className="space-y-4">
              <p className="text-sm text-zinc-500 mb-4">高级检索优化选项</p>
              
              <StrategyToggle
                icon={Filter}
                title="查询重写"
                description="自动扩展同义词和改写查询"
                enabled={config.queryRewrite}
                onChange={(v) => updateConfig({ queryRewrite: v })}
              />
              
              <StrategyToggle
                icon={Network}
                title="子查询分解"
                description="将复杂查询分解为多个子查询"
                enabled={config.subQueryDecomposition}
                onChange={(v) => updateConfig({ subQueryDecomposition: v })}
              />
              
              <StrategyToggle
                icon={BarChart3}
                title="意图分类"
                description="识别查询意图并调整检索策略"
                enabled={config.intentClassification}
                onChange={(v) => updateConfig({ intentClassification: v })}
              />
              
              <StrategyToggle
                icon={Filter}
                title="结果去重"
                description="去除相似或重复的内容"
                enabled={config.enableDeduplication}
                onChange={(v) => updateConfig({ enableDeduplication: v })}
              />
              
              <StrategyToggle
                icon={Network}
                title="MMR 多样性排序"
                description="最大化边际相关性，平衡相关性和多样性"
                enabled={config.enableMMR}
                onChange={(v) => updateConfig({ enableMMR: v })}
              />
              
              {config.enableMMR && (
                <div className="ml-8">
                  <WeightSlider
                    label="MMR Lambda (相关性 vs 多样性)"
                    value={config.mmrLambda}
                    onChange={(v) => updateConfig({ mmrLambda: v })}
                    min={0}
                    max={1}
                    step={0.1}
                  />
                </div>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900/50">
          <Button variant="ghost" size="sm" onClick={handleReset}>
            恢复默认
          </Button>
          <div className="flex gap-2">
            <Button variant="secondary" size="sm" onClick={onClose}>
              取消
            </Button>
            <Button variant="primary" size="sm" onClick={handleSave}>
              保存配置
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};

// 策略开关组件
interface StrategyToggleProps {
  icon: React.ElementType;
  title: string;
  description: string;
  enabled: boolean;
  onChange: (enabled: boolean) => void;
}

const StrategyToggle = ({ icon: Icon, title, description, enabled, onChange }: StrategyToggleProps) => (
  <label className={clsx(
    'flex items-start gap-4 p-4 rounded-xl border-2 cursor-pointer transition-all',
    enabled
      ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
      : 'border-zinc-200 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700'
  )}>
    <input
      type="checkbox"
      checked={enabled}
      onChange={(e) => onChange(e.target.checked)}
      className="sr-only"
    />
    <div className={clsx(
      'p-2 rounded-lg',
      enabled ? 'bg-blue-100 dark:bg-blue-800' : 'bg-zinc-100 dark:bg-zinc-800'
    )}>
      <Icon className={clsx('w-5 h-5', enabled ? 'text-blue-600 dark:text-blue-400' : 'text-zinc-500')} />
    </div>
    <div className="flex-1">
      <h3 className={clsx('font-medium', enabled ? 'text-blue-900 dark:text-blue-100' : 'text-zinc-900 dark:text-zinc-100')}>
        {title}
      </h3>
      <p className="text-sm text-zinc-500 mt-0.5">{description}</p>
    </div>
    <div className={clsx(
      'w-11 h-6 rounded-full transition-colors relative',
      enabled ? 'bg-blue-500' : 'bg-zinc-300 dark:bg-zinc-700'
    )}>
      <div className={clsx(
        'absolute top-1 w-4 h-4 rounded-full bg-white transition-transform',
        enabled ? 'left-6' : 'left-1'
      )} />
    </div>
  </label>
);

// 权重滑块组件
interface WeightSliderProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
  disabled?: boolean;
  min?: number;
  max?: number;
  step?: number;
}

const WeightSlider = ({ label, value, onChange, disabled, min = 0, max = 1, step = 0.05 }: WeightSliderProps) => (
  <div className={clsx('space-y-2', disabled && 'opacity-50 pointer-events-none')}>
    <div className="flex items-center justify-between">
      <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">{label}</span>
      <span className="text-sm text-zinc-500 font-mono">{(value * 100).toFixed(0)}%</span>
    </div>
    <input
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      onChange={(e) => onChange(parseFloat(e.target.value))}
      disabled={disabled}
      className="w-full h-2 bg-zinc-200 dark:bg-zinc-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
    />
  </div>
);

export default RagConfigPanel;
