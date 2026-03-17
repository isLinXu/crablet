import React, { useState, useMemo } from 'react';
import { Line, Radar, Bar, Doughnut } from 'react-chartjs-2';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  ArcElement,
  RadialLinearScale,
  Title,
  Tooltip,
  Legend,
  Filler,
} from 'chart.js';
import './ThinkingAnalytics.css';

// 注册 Chart.js 组件
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  ArcElement,
  RadialLinearScale,
  Title,
  Tooltip,
  Legend,
  Filler
);

// 思考会话数据
export interface ThinkingSession {
  id: string;
  timestamp: number;
  duration: number;
  tokenCount: number;
  stepCount: number;
  toolCalls: number;
  backtracks: number;
  confidence: number;
  coherence: number;
  efficiency: number;
  complexity: 'low' | 'medium' | 'high';
  status: 'success' | 'partial' | 'failure';
  tags: string[];
}

// 对比数据
export interface ComparisonData {
  sessions: ThinkingSession[];
  metrics: {
    avgDuration: number;
    avgTokens: number;
    successRate: number;
    avgConfidence: number;
  };
}

interface ThinkingAnalyticsProps {
  sessions: ThinkingSession[];
  selectedSessionIds?: string[];
  onSessionSelect?: (sessionId: string) => void;
  onExport?: (format: 'pdf' | 'json' | 'csv') => void;
  onCompare?: (sessionIds: string[]) => void;
}

type ViewMode = 'overview' | 'timeline' | 'comparison' | 'patterns';

export const ThinkingAnalytics: React.FC<ThinkingAnalyticsProps> = ({
  sessions,
  selectedSessionIds = [],
  onSessionSelect,
  onExport,
  onCompare,
}) => {
  const [viewMode, setViewMode] = useState<ViewMode>('overview');
  const [dateRange, setDateRange] = useState<'7d' | '30d' | '90d' | 'all'>('7d');
  const [compareMode, setCompareMode] = useState(false);

  // 过滤会话
  const filteredSessions = useMemo(() => {
    const now = Date.now();
    const ranges: Record<string, number> = {
      '7d': 7 * 24 * 60 * 60 * 1000,
      '30d': 30 * 24 * 60 * 60 * 1000,
      '90d': 90 * 24 * 60 * 60 * 1000,
    };
    
    if (dateRange === 'all') return sessions;
    
    const cutoff = now - ranges[dateRange];
    return sessions.filter(s => s.timestamp >= cutoff);
  }, [sessions, dateRange]);

  // 计算统计数据
  const stats = useMemo(() => {
    if (filteredSessions.length === 0) return null;
    
    const total = filteredSessions.length;
    const successCount = filteredSessions.filter(s => s.status === 'success').length;
    const avgDuration = filteredSessions.reduce((sum, s) => sum + s.duration, 0) / total;
    const avgTokens = filteredSessions.reduce((sum, s) => sum + s.tokenCount, 0) / total;
    const avgConfidence = filteredSessions.reduce((sum, s) => sum + s.confidence, 0) / total;
    const avgCoherence = filteredSessions.reduce((sum, s) => sum + s.coherence, 0) / total;
    const avgEfficiency = filteredSessions.reduce((sum, s) => sum + s.efficiency, 0) / total;
    
    return {
      total,
      successRate: (successCount / total) * 100,
      avgDuration,
      avgTokens,
      avgConfidence,
      avgCoherence,
      avgEfficiency,
    };
  }, [filteredSessions]);

  // 时间线图表数据
  const timelineData = useMemo(() => {
    const sorted = [...filteredSessions].sort((a, b) => a.timestamp - b.timestamp);
    
    return {
      labels: sorted.map(s => new Date(s.timestamp).toLocaleDateString()),
      datasets: [
        {
          label: '思考时长 (秒)',
          data: sorted.map(s => s.duration / 1000),
          borderColor: '#3b82f6',
          backgroundColor: 'rgba(59, 130, 246, 0.1)',
          fill: true,
          tension: 0.4,
        },
        {
          label: 'Token 数',
          data: sorted.map(s => s.tokenCount),
          borderColor: '#10b981',
          backgroundColor: 'rgba(16, 185, 129, 0.1)',
          fill: true,
          tension: 0.4,
          yAxisID: 'y1',
        },
      ],
    };
  }, [filteredSessions]);

  // 雷达图数据（能力分析）
  const radarData = useMemo(() => {
    if (!stats) return null;
    
    return {
      labels: ['置信度', '连贯性', '效率', '成功率', '复杂度管理', '工具使用'],
      datasets: [
        {
          label: '平均表现',
          data: [
            stats.avgConfidence * 100,
            stats.avgCoherence * 100,
            stats.avgEfficiency * 100,
            stats.successRate,
            75, // 复杂度管理（模拟）
            80, // 工具使用（模拟）
          ],
          backgroundColor: 'rgba(139, 92, 246, 0.2)',
          borderColor: '#8b5cf6',
          pointBackgroundColor: '#8b5cf6',
          pointBorderColor: '#fff',
          pointHoverBackgroundColor: '#fff',
          pointHoverBorderColor: '#8b5cf6',
        },
      ],
    };
  }, [stats]);

  // 复杂度分布
  const complexityData = useMemo(() => {
    const counts = {
      low: filteredSessions.filter(s => s.complexity === 'low').length,
      medium: filteredSessions.filter(s => s.complexity === 'medium').length,
      high: filteredSessions.filter(s => s.complexity === 'high').length,
    };
    
    return {
      labels: ['低复杂度', '中复杂度', '高复杂度'],
      datasets: [
        {
          data: [counts.low, counts.medium, counts.high],
          backgroundColor: ['#22c55e', '#eab308', '#ef4444'],
          borderWidth: 0,
        },
      ],
    };
  }, [filteredSessions]);

  // 状态分布
  const statusData = useMemo(() => {
    const counts = {
      success: filteredSessions.filter(s => s.status === 'success').length,
      partial: filteredSessions.filter(s => s.status === 'partial').length,
      failure: filteredSessions.filter(s => s.status === 'failure').length,
    };
    
    return {
      labels: ['成功', '部分成功', '失败'],
      datasets: [
        {
          data: [counts.success, counts.partial, counts.failure],
          backgroundColor: ['#22c55e', '#eab308', '#ef4444'],
          borderWidth: 0,
        },
      ],
    };
  }, [filteredSessions]);

  // 对比数据
  const comparisonData = useMemo(() => {
    if (selectedSessionIds.length < 2) return null;
    
    const selected = filteredSessions.filter(s => selectedSessionIds.includes(s.id));
    
    return {
      labels: selected.map((s, i) => `会话 ${i + 1}`),
      datasets: [
        {
          label: '时长 (秒)',
          data: selected.map(s => s.duration / 1000),
          backgroundColor: '#3b82f6',
        },
        {
          label: 'Token 数',
          data: selected.map(s => s.tokenCount),
          backgroundColor: '#10b981',
        },
        {
          label: '置信度',
          data: selected.map(s => s.confidence * 100),
          backgroundColor: '#8b5cf6',
        },
      ],
    };
  }, [filteredSessions, selectedSessionIds]);

  // 渲染概览视图
  const renderOverview = () => (
    <div className="analytics-overview">
      {stats && (
        <>
          <div className="stats-grid">
            <div className="stat-card">
              <span className="stat-icon">🧠</span>
              <div className="stat-content">
                <span className="stat-value">{stats.total}</span>
                <span className="stat-label">总会话</span>
              </div>
            </div>
            
            <div className="stat-card success">
              <span className="stat-icon">✅</span>
              <div className="stat-content">
                <span className="stat-value">{stats.successRate.toFixed(1)}%</span>
                <span className="stat-label">成功率</span>
              </div>
            </div>
            
            <div className="stat-card">
              <span className="stat-icon">⏱️</span>
              <div className="stat-content">
                <span className="stat-value">{(stats.avgDuration / 1000).toFixed(1)}s</span>
                <span className="stat-label">平均时长</span>
              </div>
            </div>
            
            <div className="stat-card">
              <span className="stat-icon">📝</span>
              <div className="stat-content">
                <span className="stat-value">{Math.round(stats.avgTokens)}</span>
                <span className="stat-label">平均 Token</span>
              </div>
            </div>
            
            <div className="stat-card">
              <span className="stat-icon">🎯</span>
              <div className="stat-content">
                <span className="stat-value">{(stats.avgConfidence * 100).toFixed(1)}%</span>
                <span className="stat-label">平均置信度</span>
              </div>
            </div>
            
            <div className="stat-card">
              <span className="stat-icon">🔗</span>
              <div className="stat-content">
                <span className="stat-value">{(stats.avgCoherence * 100).toFixed(1)}%</span>
                <span className="stat-label">平均连贯性</span>
              </div>
            </div>
          </div>

          <div className="charts-grid">
            <div className="chart-card">
              <h4>能力雷达图</h4>
              {radarData && (
                <Radar 
                  data={radarData}
                  options={{
                    responsive: true,
                    maintainAspectRatio: false,
                    scales: {
                      r: {
                        beginAtZero: true,
                        max: 100,
                        ticks: {
                          stepSize: 20,
                          color: '#64748b',
                        },
                        grid: {
                          color: '#334155',
                        },
                        pointLabels: {
                          color: '#94a3b8',
                        },
                      },
                    },
                    plugins: {
                      legend: {
                        display: false,
                      },
                    },
                  }}
                />
              )}
            </div>
            
            <div className="chart-card">
              <h4>复杂度分布</h4>
              <Doughnut 
                data={complexityData}
                options={{
                  responsive: true,
                  maintainAspectRatio: false,
                  plugins: {
                    legend: {
                      position: 'bottom',
                      labels: {
                        color: '#94a3b8',
                        padding: 16,
                      },
                    },
                  },
                }}
              />
            </div>
            
            <div className="chart-card">
              <h4>状态分布</h4>
              <Doughnut 
                data={statusData}
                options={{
                  responsive: true,
                  maintainAspectRatio: false,
                  plugins: {
                    legend: {
                      position: 'bottom',
                      labels: {
                        color: '#94a3b8',
                        padding: 16,
                      },
                    },
                  },
                }}
              />
            </div>
          </div>
        </>
      )}
    </div>
  );

  // 渲染时间线视图
  const renderTimeline = () => (
    <div className="analytics-timeline">
      <div className="chart-card full-width">
        <h4>思考趋势</h4>
        <Line 
          data={timelineData}
          options={{
            responsive: true,
            maintainAspectRatio: false,
            interaction: {
              mode: 'index',
              intersect: false,
            },
            scales: {
              x: {
                grid: {
                  color: '#334155',
                },
                ticks: {
                  color: '#94a3b8',
                },
              },
              y: {
                type: 'linear',
                display: true,
                position: 'left',
                grid: {
                  color: '#334155',
                },
                ticks: {
                  color: '#94a3b8',
                },
                title: {
                  display: true,
                  text: '时长 (秒)',
                  color: '#64748b',
                },
              },
              y1: {
                type: 'linear',
                display: true,
                position: 'right',
                grid: {
                  drawOnChartArea: false,
                },
                ticks: {
                  color: '#94a3b8',
                },
                title: {
                  display: true,
                  text: 'Token 数',
                  color: '#64748b',
                },
              },
            },
            plugins: {
              legend: {
                labels: {
                  color: '#94a3b8',
                },
              },
            },
          }}
        />
      </div>
      
      <div className="session-list">
        <h4>会话列表</h4>
        {filteredSessions.map(session => (
          <div 
            key={session.id}
            className={`session-item ${selectedSessionIds.includes(session.id) ? 'selected' : ''}`}
            onClick={() => {
              if (compareMode) {
                onCompare?.([...selectedSessionIds, session.id]);
              } else {
                onSessionSelect?.(session.id);
              }
            }}
          >
            <div className="session-header">
              <span className={`status-badge ${session.status}`}>
                {session.status === 'success' ? '✅' : 
                 session.status === 'partial' ? '⚠️' : '❌'}
              </span>
              <span className="session-date">
                {new Date(session.timestamp).toLocaleString()}
              </span>
              <span className={`complexity-badge ${session.complexity}`}>
                {session.complexity === 'low' ? '低' : 
                 session.complexity === 'medium' ? '中' : '高'}
              </span>
            </div>
            
            <div className="session-metrics">
              <span>⏱️ {(session.duration / 1000).toFixed(1)}s</span>
              <span>📝 {session.tokenCount}</span>
              <span>🔧 {session.toolCalls}</span>
              <span>🔄 {session.backtracks}</span>
              <span>🎯 {(session.confidence * 100).toFixed(0)}%</span>
            </div>
            
            {session.tags.length > 0 && (
              <div className="session-tags">
                {session.tags.map(tag => (
                  <span key={tag} className="tag">{tag}</span>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );

  // 渲染对比视图
  const renderComparison = () => (
    <div className="analytics-comparison">
      {comparisonData ? (
        <div className="chart-card full-width">
          <h4>会话对比</h4>
          <Bar 
            data={comparisonData}
            options={{
              responsive: true,
              maintainAspectRatio: false,
              scales: {
                x: {
                  grid: {
                    color: '#334155',
                  },
                  ticks: {
                    color: '#94a3b8',
                  },
                },
                y: {
                  grid: {
                    color: '#334155',
                  },
                  ticks: {
                    color: '#94a3b8',
                  },
                },
              },
              plugins: {
                legend: {
                  labels: {
                    color: '#94a3b8',
                  },
                },
              },
            }}
          />
        </div>
      ) : (
        <div className="comparison-placeholder">
          <span className="placeholder-icon">📊</span>
          <span className="placeholder-text">请选择至少 2 个会话进行对比</span>
          <button 
            className="btn-primary"
            onClick={() => setCompareMode(true)}
          >
            进入选择模式
          </button>
        </div>
      )}
    </div>
  );

  return (
    <div className="thinking-analytics">
      {/* 工具栏 */}
      <div className="analytics-toolbar">
        <div className="view-tabs">
          <button 
            className={`tab ${viewMode === 'overview' ? 'active' : ''}`}
            onClick={() => setViewMode('overview')}
          >
            📊 概览
          </button>
          <button 
            className={`tab ${viewMode === 'timeline' ? 'active' : ''}`}
            onClick={() => setViewMode('timeline')}
          >
            📈 时间线
          </button>
          <button 
            className={`tab ${viewMode === 'comparison' ? 'active' : ''}`}
            onClick={() => setViewMode('comparison')}
          >
            ⚖️ 对比
          </button>
        </div>
        
        <div className="toolbar-actions">
          <select 
            value={dateRange}
            onChange={(e) => setDateRange(e.target.value as any)}
            className="date-range-select"
          >
            <option value="7d">最近 7 天</option>
            <option value="30d">最近 30 天</option>
            <option value="90d">最近 90 天</option>
            <option value="all">全部</option>
          </select>
          
          <div className="export-dropdown">
            <button className="btn-export">
              📥 导出
            </button>
            <div className="dropdown-menu">
              <button onClick={() => onExport?.('pdf')}>PDF 报告</button>
              <button onClick={() => onExport?.('json')}>JSON 数据</button>
              <button onClick={() => onExport?.('csv')}>CSV 表格</button>
            </div>
          </div>
        </div>
      </div>

      {/* 内容区 */}
      <div className="analytics-content">
        {viewMode === 'overview' && renderOverview()}
        {viewMode === 'timeline' && renderTimeline()}
        {viewMode === 'comparison' && renderComparison()}
      </div>
    </div>
  );
};

export default ThinkingAnalytics;
