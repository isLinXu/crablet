import React, { useEffect, useRef, useState, useCallback } from 'react';
import * as d3 from 'd3';
import './ThoughtGraphViewer.css';

// 思维节点类型
export type ThoughtNodeType = 
  | 'reasoning'
  | 'tool_call'
  | 'observation'
  | 'decision'
  | 'reflection'
  | 'retrieval'
  | 'planning'
  | 'summary'
  | 'error'
  | 'user_input'
  | 'system_prompt';

// 节点状态
export type ThoughtNodeStatus = 
  | 'pending'
  | 'processing'
  | 'completed'
  | 'skipped'
  | 'failed'
  | 'corrected';

// 边类型
export type EdgeType = 
  | 'sequential'
  | 'branch'
  | 'merge'
  | 'backtrack'
  | 'reference'
  | 'correction';

// 思维节点接口
export interface ThoughtNode {
  id: string;
  node_type: ThoughtNodeType;
  status: ThoughtNodeStatus;
  content: string;
  parent_ids: string[];
  child_ids: string[];
  alternative_ids: string[];
  created_at: number;
  completed_at?: number;
  duration_ms?: number;
  confidence?: number;
  information_gain?: number;
  metadata: Record<string, any>;
  depth: number;
  branch_id?: string;
  x?: number;
  y?: number;
}

// 思维边接口
export interface ThoughtEdge {
  id: string;
  source: string;
  target: string;
  edge_type: EdgeType;
  label?: string;
  weight: number;
}

// 思维图谱接口
export interface ThoughtGraph {
  id: string;
  root_id: string;
  nodes: Record<string, ThoughtNode>;
  edges: ThoughtEdge[];
  active_node_id?: string;
  created_at: number;
  updated_at: number;
}

// 图谱统计
export interface ThoughtGraphStats {
  total_nodes: number;
  completed_nodes: number;
  failed_nodes: number;
  max_depth: number;
  total_duration_ms: number;
  average_confidence?: number;
  branch_count: number;
}

interface ThoughtGraphViewerProps {
  graph: ThoughtGraph;
  width?: number;
  height?: number;
  onNodeClick?: (node: ThoughtNode) => void;
  onNodeHover?: (node: ThoughtNode | null) => void;
  showMiniMap?: boolean;
  showStats?: boolean;
  layout?: 'tree' | 'force' | 'dagre';
}

// 节点类型配置
const NODE_TYPE_CONFIG: Record<ThoughtNodeType, { color: string; icon: string; label: string }> = {
  'reasoning': { color: '#3b82f6', icon: '🧠', label: '推理' },
  'tool_call': { color: '#f59e0b', icon: '🔧', label: '工具调用' },
  'observation': { color: '#10b981', icon: '👁️', label: '观察' },
  'decision': { color: '#8b5cf6', icon: '🎯', label: '决策' },
  'reflection': { color: '#ec4899', icon: '🔄', label: '反思' },
  'retrieval': { color: '#06b6d4', icon: '🔍', label: '检索' },
  'planning': { color: '#6366f1', icon: '📋', label: '规划' },
  'summary': { color: '#14b8a6', icon: '📝', label: '总结' },
  'error': { color: '#ef4444', icon: '❌', label: '错误' },
  'user_input': { color: '#84cc16', icon: '💬', label: '用户输入' },
  'system_prompt': { color: '#6b7280', icon: '⚙️', label: '系统提示' },
};

// 节点状态配置
const NODE_STATUS_CONFIG: Record<ThoughtNodeStatus, { pulse: boolean; opacity: number }> = {
  'pending': { pulse: false, opacity: 0.5 },
  'processing': { pulse: true, opacity: 1 },
  'completed': { pulse: false, opacity: 1 },
  'skipped': { pulse: false, opacity: 0.3 },
  'failed': { pulse: false, opacity: 1 },
  'corrected': { pulse: false, opacity: 0.7 },
};

export const ThoughtGraphViewer: React.FC<ThoughtGraphViewerProps> = ({
  graph,
  width = 800,
  height = 600,
  onNodeClick,
  onNodeHover,
  showMiniMap = true,
  showStats = true,
  layout = 'tree',
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [selectedNode, setSelectedNode] = useState<ThoughtNode | null>(null);
  const [hoveredNode, setHoveredNode] = useState<ThoughtNode | null>(null);
  const [zoomTransform, setZoomTransform] = useState<d3.ZoomTransform>(d3.zoomIdentity);
  const [stats, setStats] = useState<ThoughtGraphStats | null>(null);

  // 计算图谱统计
  useEffect(() => {
    const nodes = Object.values(graph.nodes);
    const completedNodes = nodes.filter(n => n.status === 'completed').length;
    const failedNodes = nodes.filter(n => n.status === 'failed').length;
    const maxDepth = Math.max(...nodes.map(n => n.depth), 0);
    const totalDuration = nodes.reduce((sum, n) => sum + (n.duration_ms || 0), 0);
    const confidences = nodes.filter(n => n.confidence !== undefined).map(n => n.confidence!);
    const avgConfidence = confidences.length > 0 
      ? confidences.reduce((a, b) => a + b, 0) / confidences.length 
      : undefined;
    const branchCount = graph.edges.filter(e => e.edge_type === 'branch').length;

    setStats({
      total_nodes: nodes.length,
      completed_nodes: completedNodes,
      failed_nodes: failedNodes,
      max_depth: maxDepth,
      total_duration_ms: totalDuration,
      average_confidence: avgConfidence,
      branch_count: branchCount,
    });
  }, [graph]);

  // 计算树形布局
  const calculateTreeLayout = useCallback((graph: ThoughtGraph) => {
    const nodes = Object.values(graph.nodes);
    const root = nodes.find(n => n.id === graph.root_id);
    if (!root) return { nodes: [], links: [] };

    const nodeMap = new Map<string, ThoughtNode>();
    const levelWidth = 180;
    const nodeHeight = 80;

    // 按深度分组
    const nodesByDepth: ThoughtNode[][] = [];
    nodes.forEach(node => {
      if (!nodesByDepth[node.depth]) {
        nodesByDepth[node.depth] = [];
      }
      nodesByDepth[node.depth].push(node);
    });

    // 计算位置
    nodesByDepth.forEach((levelNodes, depth) => {
      const totalHeight = levelNodes.length * nodeHeight;
      const startY = -totalHeight / 2 + nodeHeight / 2;
      
      levelNodes.forEach((node, index) => {
        const positionedNode = {
          ...node,
          x: depth * levelWidth,
          y: startY + index * nodeHeight,
        };
        nodeMap.set(node.id, positionedNode);
      });
    });

    // 创建连接
    const links = graph.edges.map(edge => ({
      source: nodeMap.get(edge.source),
      target: nodeMap.get(edge.target),
      edge,
    })).filter(l => l.source && l.target);

    return { 
      nodes: Array.from(nodeMap.values()), 
      links 
    };
  }, []);

  // 渲染图谱
  useEffect(() => {
    if (!svgRef.current) return;

    const svg = d3.select(svgRef.current);
    svg.selectAll('*').remove();

    const { nodes, links } = calculateTreeLayout(graph);
    
    // 创建主容器
    const g = svg.append('g');

    // 添加缩放行为
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 4])
      .on('zoom', (event) => {
        g.attr('transform', event.transform.toString());
        setZoomTransform(event.transform);
      });

    svg.call(zoom);

    // 初始居中
    const initialTransform = d3.zoomIdentity
      .translate(100, height / 2)
      .scale(0.9);
    svg.call(zoom.transform, initialTransform);

    // 定义箭头标记
    const defs = svg.append('defs');
    
    // 普通箭头
    defs.append('marker')
      .attr('id', 'arrow-normal')
      .attr('viewBox', '0 -5 10 10')
      .attr('refX', 28)
      .attr('refY', 0)
      .attr('markerWidth', 6)
      .attr('markerHeight', 6)
      .attr('orient', 'auto')
      .append('path')
      .attr('d', 'M0,-5L10,0L0,5')
      .attr('fill', '#64748b');

    // 分支箭头
    defs.append('marker')
      .attr('id', 'arrow-branch')
      .attr('viewBox', '0 -5 10 10')
      .attr('refX', 28)
      .attr('refY', 0)
      .attr('markerWidth', 6)
      .attr('markerHeight', 6)
      .attr('orient', 'auto')
      .append('path')
      .attr('d', 'M0,-5L10,0L0,5')
      .attr('fill', '#8b5cf6');

    // 绘制连接线
    const linkGroup = g.append('g').attr('class', 'links');
    
    links.forEach(({ source, target, edge }) => {
      if (!source?.x || !source?.y || !target?.x || !target?.y) return;

      const isBranch = edge.edge_type === 'branch';
      const isActive = graph.active_node_id === target.id;

      const path = linkGroup.append('path')
        .attr('d', d3.linkHorizontal()({
          source: [source.x!, source.y!],
          target: [target.x!, target.y!],
        } as any))
        .attr('fill', 'none')
        .attr('stroke', isBranch ? '#8b5cf6' : '#64748b')
        .attr('stroke-width', isActive ? 3 : 2)
        .attr('stroke-dasharray', isBranch ? '5,5' : 'none')
        .attr('marker-end', `url(#arrow-${isBranch ? 'branch' : 'normal'})`)
        .attr('opacity', isActive ? 1 : 0.6);

      // 添加边标签
      if (edge.label) {
        const midX = (source.x! + target.x!) / 2;
        const midY = (source.y! + target.y!) / 2;
        
        linkGroup.append('rect')
          .attr('x', midX - 30)
          .attr('y', midY - 10)
          .attr('width', 60)
          .attr('height', 20)
          .attr('fill', '#1e293b')
          .attr('rx', 4);
        
        linkGroup.append('text')
          .attr('x', midX)
          .attr('y', midY + 4)
          .attr('text-anchor', 'middle')
          .attr('fill', '#94a3b8')
          .attr('font-size', '10px')
          .text(edge.label);
      }
    });

    // 绘制节点
    const nodeGroup = g.append('g').attr('class', 'nodes');

    nodes.forEach(node => {
      if (node.x === undefined || node.y === undefined) return;

      const config = NODE_TYPE_CONFIG[node.node_type];
      const statusConfig = NODE_STATUS_CONFIG[node.status];
      const isActive = graph.active_node_id === node.id;
      const isSelected = selectedNode?.id === node.id;

      const nodeG = nodeGroup.append('g')
        .attr('transform', `translate(${node.x},${node.y})`)
        .attr('class', `thought-node ${isActive ? 'active' : ''} ${isSelected ? 'selected' : ''}`)
        .style('cursor', 'pointer')
        .on('click', () => {
          setSelectedNode(node);
          onNodeClick?.(node);
        })
        .on('mouseenter', () => {
          setHoveredNode(node);
          onNodeHover?.(node);
        })
        .on('mouseleave', () => {
          setHoveredNode(null);
          onNodeHover?.(null);
        });

      // 脉冲动画（处理中状态）
      if (statusConfig.pulse) {
        nodeG.append('circle')
          .attr('r', 35)
          .attr('fill', config.color)
          .attr('opacity', 0.3)
          .append('animate')
          .attr('attributeName', 'r')
          .attr('values', '35;45;35')
          .attr('dur', '1.5s')
          .attr('repeatCount', 'indefinite');
      }

      // 节点外圈
      nodeG.append('circle')
        .attr('r', 32)
        .attr('fill', '#1e293b')
        .attr('stroke', isActive ? config.color : '#475569')
        .attr('stroke-width', isActive ? 3 : 2);

      // 节点内圈（类型颜色）
      nodeG.append('circle')
        .attr('r', 28)
        .attr('fill', config.color)
        .attr('opacity', statusConfig.opacity);

      // 图标
      nodeG.append('text')
        .attr('text-anchor', 'middle')
        .attr('dy', '5')
        .attr('font-size', '20px')
        .text(config.icon);

      // 节点标签
      nodeG.append('text')
        .attr('text-anchor', 'middle')
        .attr('dy', 45)
        .attr('fill', '#e2e8f0')
        .attr('font-size', '11px')
        .attr('font-weight', '500')
        .text(config.label);

      // 内容预览（截断）
      const preview = node.content.slice(0, 15) + (node.content.length > 15 ? '...' : '');
      nodeG.append('text')
        .attr('text-anchor', 'middle')
        .attr('dy', 58)
        .attr('fill', '#94a3b8')
        .attr('font-size', '9px')
        .text(preview);

      // 置信度指示器
      if (node.confidence !== undefined) {
        const confidenceColor = node.confidence > 0.8 ? '#22c55e' : 
                               node.confidence > 0.5 ? '#eab308' : '#ef4444';
        
        nodeG.append('circle')
          .attr('cx', 20)
          .attr('cy', -20)
          .attr('r', 8)
          .attr('fill', confidenceColor);
        
        nodeG.append('text')
          .attr('x', 20)
          .attr('y', -16)
          .attr('text-anchor', 'middle')
          .attr('fill', 'white')
          .attr('font-size', '8px')
          .attr('font-weight', 'bold')
          .text(`${Math.round(node.confidence * 100)}`);
      }

      // 状态指示器
      if (node.status === 'failed') {
        nodeG.append('circle')
          .attr('cx', -20)
          .attr('cy', -20)
          .attr('r', 8)
          .attr('fill', '#ef4444');
        
        nodeG.append('text')
          .attr('x', -20)
          .attr('y', -16)
          .attr('text-anchor', 'middle')
          .attr('fill', 'white')
          .attr('font-size', '10px')
          .text('!');
      }
    });

  }, [graph, selectedNode, onNodeClick, onNodeHover, calculateTreeLayout, height]);

  // 渲染迷你地图
  const renderMiniMap = () => {
    if (!showMiniMap || !stats) return null;
    
    const miniWidth = 150;
    const miniHeight = 100;
    const scale = Math.min(miniWidth / (stats.max_depth * 180 + 100), miniHeight / 400);

    return (
      <div className="thought-graph-minimap">
        <svg width={miniWidth} height={miniHeight} viewBox={`0 0 ${miniWidth} ${miniHeight}`}>
          <rect width={miniWidth} height={miniHeight} fill="#1e293b" rx="4" />
          
          {/* 简化的节点表示 */}
          {Object.values(graph.nodes).map((node, i) => (
            <circle
              key={node.id}
              cx={10 + (node.depth * 30 * scale)}
              cy={10 + (i * 8 * scale) % (miniHeight - 20)}
              r={3}
              fill={NODE_TYPE_CONFIG[node.node_type].color}
              opacity={node.id === graph.active_node_id ? 1 : 0.5}
            />
          ))}
          
          {/* 视口指示器 */}
          <rect
            x={10}
            y={10}
            width={miniWidth - 20}
            height={miniHeight - 20}
            fill="none"
            stroke="#64748b"
            strokeWidth="1"
            strokeDasharray="2,2"
          />
        </svg>
      </div>
    );
  };

  // 渲染统计面板
  const renderStats = () => {
    if (!showStats || !stats) return null;

    return (
      <div className="thought-graph-stats">
        <div className="stat-item">
          <span className="stat-value">{stats.total_nodes}</span>
          <span className="stat-label">节点</span>
        </div>
        <div className="stat-item">
          <span className="stat-value">{stats.max_depth}</span>
          <span className="stat-label">深度</span>
        </div>
        <div className="stat-item">
          <span className="stat-value">{stats.branch_count}</span>
          <span className="stat-label">分支</span>
        </div>
        {stats.average_confidence && (
          <div className="stat-item">
            <span className="stat-value">{(stats.average_confidence * 100).toFixed(0)}%</span>
            <span className="stat-label">置信度</span>
          </div>
        )}
        <div className="stat-item">
          <span className="stat-value">{(stats.total_duration_ms / 1000).toFixed(1)}s</span>
          <span className="stat-label">耗时</span>
        </div>
      </div>
    );
  };

  // 渲染节点详情面板
  const renderNodeDetail = () => {
    if (!hoveredNode && !selectedNode) return null;
    
    const node = selectedNode || hoveredNode;
    if (!node) return null;

    const config = NODE_TYPE_CONFIG[node.node_type];

    return (
      <div className="thought-node-detail">
        <div className="detail-header">
          <span className="detail-icon">{config.icon}</span>
          <span className="detail-type">{config.label}</span>
          <span 
            className="detail-status"
            style={{ 
              background: node.status === 'completed' ? '#22c55e' :
                         node.status === 'failed' ? '#ef4444' :
                         node.status === 'processing' ? '#3b82f6' : '#6b7280'
            }}
          >
            {node.status}
          </span>
        </div>
        
        <div className="detail-content">
          <p>{node.content}</p>
        </div>

        <div className="detail-meta">
          {node.confidence !== undefined && (
            <div className="meta-item">
              <span className="meta-label">置信度:</span>
              <div className="confidence-bar">
                <div 
                  className="confidence-fill"
                  style={{ width: `${node.confidence * 100}%` }}
                />
              </div>
              <span className="meta-value">{(node.confidence * 100).toFixed(1)}%</span>
            </div>
          )}
          
          {node.information_gain !== undefined && (
            <div className="meta-item">
              <span className="meta-label">信息增益:</span>
              <span className="meta-value">{node.information_gain.toFixed(3)}</span>
            </div>
          )}
          
          {node.duration_ms !== undefined && (
            <div className="meta-item">
              <span className="meta-label">耗时:</span>
              <span className="meta-value">{node.duration_ms}ms</span>
            </div>
          )}
          
          <div className="meta-item">
            <span className="meta-label">深度:</span>
            <span className="meta-value">{node.depth}</span>
          </div>
          
          {node.branch_id && (
            <div className="meta-item">
              <span className="meta-label">分支:</span>
              <span className="meta-value">{node.branch_id}</span>
            </div>
          )}
        </div>

        {Object.keys(node.metadata).length > 0 && (
          <div className="detail-metadata">
            <h5>元数据</h5>
            <pre>{JSON.stringify(node.metadata, null, 2)}</pre>
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="thought-graph-viewer" ref={containerRef}>
      <div className="graph-container">
        <svg
          ref={svgRef}
          width={width}
          height={height}
          className="graph-svg"
        />
        
        {renderMiniMap()}
        {renderStats()}
        
        <div className="graph-controls">
          <button 
            className="control-btn"
            onClick={() => {
              if (svgRef.current) {
                const svg = d3.select(svgRef.current);
                svg.transition().duration(750).call(
                  d3.zoom<SVGSVGElement, unknown>().transform,
                  d3.zoomIdentity.translate(100, height / 2).scale(0.9)
                );
              }
            }}
            title="重置视图"
          >
            ⟲
          </button>
          <button 
            className="control-btn"
            onClick={() => {
              if (svgRef.current) {
                const svg = d3.select(svgRef.current);
                const current = d3.zoomTransform(svg.node()!);
                svg.transition().duration(300).call(
                  d3.zoom<SVGSVGElement, unknown>().transform,
                  current.scale(current.k * 1.2)
                );
              }
            }}
            title="放大"
          >
            +
          </button>
          <button 
            className="control-btn"
            onClick={() => {
              if (svgRef.current) {
                const svg = d3.select(svgRef.current);
                const current = d3.zoomTransform(svg.node()!);
                svg.transition().duration(300).call(
                  d3.zoom<SVGSVGElement, unknown>().transform,
                  current.scale(current.k / 1.2)
                );
              }
            }}
            title="缩小"
          >
            −
          </button>
        </div>
      </div>
      
      {renderNodeDetail()}
    </div>
  );
};

export default ThoughtGraphViewer;
