/**
 * MiniMap - 迷你地图组件
 * 提供工作流概览和快速导航
 */

import React, { useMemo, useCallback, useRef, useEffect } from 'react';
import type { Node, Edge, Viewport } from '@xyflow/react';

interface MiniMapProps {
  nodes: Node[];
  edges: Edge[];
  viewport: Viewport;
  onViewportChange: (viewport: Viewport) => void;
  width?: number;
  height?: number;
  position?: 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
  nodeColor?: (node: Node) => string;
  nodeStrokeColor?: (node: Node) => string;
}

export const MiniMap: React.FC<MiniMapProps> = ({
  nodes,
  edges,
  viewport,
  onViewportChange,
  width = 200,
  height = 150,
  position = 'bottom-right',
  nodeColor,
  nodeStrokeColor,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const isDraggingRef = useRef(false);
  const lastPosRef = useRef({ x: 0, y: 0 });

  // 计算边界和缩放
  const bounds = useMemo(() => {
    if (nodes.length === 0) {
      return { minX: 0, minY: 0, maxX: 1000, maxY: 1000 };
    }

    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    
    nodes.forEach((node) => {
      minX = Math.min(minX, node.position.x);
      minY = Math.min(minY, node.position.y);
      maxX = Math.max(maxX, node.position.x + (node.width || 200));
      maxY = Math.max(maxY, node.position.y + (node.height || 100));
    });

    // 添加边距
    const padding = 100;
    return {
      minX: minX - padding,
      minY: minY - padding,
      maxX: maxX + padding,
      maxY: maxY + padding,
    };
  }, [nodes]);

  const worldWidth = bounds.maxX - bounds.minX;
  const worldHeight = bounds.maxY - bounds.minY;
  const scaleX = width / worldWidth;
  const scaleY = height / worldHeight;
  const scale = Math.min(scaleX, scaleY, 0.2);

  // 转换坐标
  const toMiniMap = useCallback((x: number, y: number) => {
    return {
      x: (x - bounds.minX) * scale,
      y: (y - bounds.minY) * scale,
    };
  }, [bounds, scale]);

  // 获取节点颜色
  const getNodeFill = useCallback((node: Node) => {
    if (nodeColor) return nodeColor(node);
    
    const typeColors: Record<string, string> = {
      start: '#22c55e',
      end: '#ef4444',
      llm: '#3b82f6',
      agent: '#8b5cf6',
      condition: '#f59e0b',
      loop: '#ec4899',
      code: '#6b7280',
      knowledge: '#14b8a6',
      http: '#0ea5e9',
      template: '#f97316',
      variable: '#64748b',
    };
    
    return typeColors[node.type || 'default'] || '#94a3b8';
  }, [nodeColor]);

  const getNodeStroke = useCallback((node: Node) => {
    if (nodeStrokeColor) return nodeStrokeColor(node);
    return '#ffffff';
  }, [nodeStrokeColor]);

  // 计算视口矩形
  const viewportRect = useMemo(() => {
    const viewWidth = (window.innerWidth / viewport.zoom);
    const viewHeight = (window.innerHeight / viewport.zoom);
    const viewX = -viewport.x / viewport.zoom;
    const viewY = -viewport.y / viewport.zoom;
    
    const { x, y } = toMiniMap(viewX, viewY);
    const w = viewWidth * scale;
    const h = viewHeight * scale;
    
    return { x, y, width: w, height: h };
  }, [viewport, scale, toMiniMap]);

  // 处理点击/拖动
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    isDraggingRef.current = true;
    lastPosRef.current = { x: e.clientX, y: e.clientY };
    e.preventDefault();
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!isDraggingRef.current) return;
    
    const dx = e.clientX - lastPosRef.current.x;
    const dy = e.clientY - lastPosRef.current.y;
    lastPosRef.current = { x: e.clientX, y: e.clientY };
    
    const newX = viewport.x - dx / scale * 2;
    const newY = viewport.y - dy / scale * 2;
    
    onViewportChange({ ...viewport, x: newX, y: newY });
  }, [viewport, scale, onViewportChange]);

  const handleMouseUp = useCallback(() => {
    isDraggingRef.current = false;
  }, []);

  // 处理点击迷你地图跳转
  const handleClick = useCallback((e: React.MouseEvent) => {
    if (isDraggingRef.current) return;
    
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    const clickX = e.clientX - rect.left;
    const clickY = e.clientY - rect.top;
    
    // 转换到世界坐标
    const worldX = clickX / scale + bounds.minX;
    const worldY = clickY / scale + bounds.minY;
    
    // 计算新的视口中心
    const viewWidth = window.innerWidth / viewport.zoom;
    const viewHeight = window.innerHeight / viewport.zoom;
    
    const newX = -(worldX - viewWidth / 2) * viewport.zoom;
    const newY = -(worldY - viewHeight / 2) * viewport.zoom;
    
    onViewportChange({ ...viewport, x: newX, y: newY });
  }, [viewport, scale, bounds, onViewportChange]);

  const positionClasses = {
    'top-left': 'top-4 left-4',
    'top-right': 'top-4 right-4',
    'bottom-left': 'bottom-4 left-4',
    'bottom-right': 'bottom-4 right-4',
  };

  return (
    <div
      ref={containerRef}
      className={`absolute ${positionClasses[position]} bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 overflow-hidden cursor-pointer ${positionClasses[position]}`}
      style={{ width, height }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
      onClick={handleClick}
    >
      <svg width={width} height={height} className="block">
        {/* 背景网格 */}
        <defs>
          <pattern id="minimap-grid" width={20 * scale} height={20 * scale} patternUnits="userSpaceOnUse">
            <path d={`M ${20 * scale} 0 L 0 0 0 ${20 * scale}`} fill="none" stroke="rgba(0,0,0,0.05)" strokeWidth="1"/>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#minimap-grid)" />
        
        {/* 连接线 */}
        {edges.map((edge) => {
          const sourceNode = nodes.find((n) => n.id === edge.source);
          const targetNode = nodes.find((n) => n.id === edge.target);
          if (!sourceNode || !targetNode) return null;
          
          const sourcePos = toMiniMap(
            sourceNode.position.x + (sourceNode.width || 200),
            sourceNode.position.y + (sourceNode.height || 100) / 2
          );
          const targetPos = toMiniMap(
            targetNode.position.x,
            targetNode.position.y + (targetNode.height || 100) / 2
          );
          
          return (
            <path
              key={edge.id}
              d={`M${sourcePos.x},${sourcePos.y} L${targetPos.x},${targetPos.y}`}
              fill="none"
              stroke="rgba(148, 163, 184, 0.5)"
              strokeWidth="1"
            />
          );
        })}
        
        {/* 节点 */}
        {nodes.map((node) => {
          const { x, y } = toMiniMap(node.position.x, node.position.y);
          const nodeW = (node.width || 200) * scale;
          const nodeH = (node.height || 100) * scale;
          
          return (
            <rect
              key={node.id}
              x={x}
              y={y}
              width={nodeW}
              height={nodeH}
              rx={2}
              fill={getNodeFill(node)}
              stroke={getNodeStroke(node)}
              strokeWidth="1"
            />
          );
        })}
        
        {/* 视口矩形 */}
        <rect
          x={viewportRect.x}
          y={viewportRect.y}
          width={viewportRect.width}
          height={viewportRect.height}
          fill="rgba(59, 130, 246, 0.1)"
          stroke="#3b82f6"
          strokeWidth="2"
          rx="2"
        />
      </svg>
      
      {/* 节点计数 */}
      <div className="absolute bottom-1 right-1 text-[10px] text-gray-500 bg-white/80 dark:bg-gray-800/80 px-1 rounded">
        {nodes.length} 节点
      </div>
    </div>
  );
};

export default MiniMap;
