/**
 * Canvas Performance Optimization - 性能优化工具
 * 支持虚拟化、懒加载、性能监控
 */

import { useCallback, useMemo, useRef, useState, useEffect } from 'react';
import type { Node, Edge, Viewport } from '@xyflow/react';

export interface PerformanceMetrics {
  nodeCount: number;
  edgeCount: number;
  visibleNodeCount: number;
  renderTime: number;
  fps: number;
  memoryUsage?: number;
}

export interface UseCanvasPerformanceOptions {
  virtualizationThreshold?: number;
  debounceMs?: number;
  enablePerformanceMonitor?: boolean;
}

export interface UseCanvasPerformanceReturn {
  visibleNodes: Node[];
  visibleEdges: Edge[];
  metrics: PerformanceMetrics;
  isLoading: boolean;
  fps: number;
  setViewport: (viewport: Viewport) => void;
  forceUpdate: () => void;
}

// 计算节点是否在视口内
function isNodeInViewport(
  node: Node,
  viewport: Viewport,
  canvasWidth: number,
  canvasHeight: number,
  padding: number = 100
): boolean {
  const x = (node.position.x * viewport.zoom) + viewport.x;
  const y = (node.position.y * viewport.zoom) + viewport.y;
  
  return (
    x + (node.width || 200) * viewport.zoom >= -padding &&
    x <= canvasWidth + padding &&
    y + (node.height || 100) * viewport.zoom >= -padding &&
    y <= canvasHeight + padding
  );
}

// 计算两点之间的直线
function calculateStraightPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number
): string {
  return `M${sourceX},${sourceY} L${targetX},${targetY}`;
}

// 计算贝塞尔曲线路径
function calculateBezierPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
  curvature: number = 0.5
): string {
  const midX = (sourceX + targetX) / 2;
  const controlPointOffset = Math.abs(targetX - sourceX) * curvature;
  
  return `M${sourceX},${sourceY} C${sourceX + controlPointOffset},${sourceY} ${targetX - controlPointOffset},${targetY} ${targetX},${targetY}`;
}

// 自动路由算法 - 智能路径规划
export function calculateAutoRoute(
  sourceNode: Node,
  targetNode: Node,
  allNodes: Node[],
  options: {
    type?: 'straight' | 'bezier' | 'step' | 'smart';
    curvature?: number;
    margin?: number;
  } = {}
): string {
  const {
    type = 'smart',
    curvature = 0.5,
    margin = 20,
  } = options;

  const sourcePos = sourceNode.position;
  const targetPos = targetNode.position;
  
  const sourceX = sourcePos.x + (sourceNode.width || 200);
  const sourceY = sourcePos.y + (sourceNode.height || 100) / 2;
  const targetX = targetPos.x;
  const targetY = targetPos.y + (targetNode.height || 100) / 2;

  switch (type) {
    case 'straight':
      return calculateStraightPath(sourceX, sourceY, targetX, targetY);
      
    case 'bezier':
      return calculateBezierPath(sourceX, sourceY, targetX, targetY, curvature);
      
    case 'step':
      const midX = (sourceX + targetX) / 2;
      return `M${sourceX},${sourceY} L${midX},${sourceY} L${midX},${targetY} L${targetX},${targetY}`;
      
    case 'smart':
    default:
      // 智能路由：检测是否有垂直冲突，决定是否绕过
      const hasVerticalConflict = allNodes.some((node) => {
        if (node.id === sourceNode.id || node.id === targetNode.id) return false;
        const nodeY = node.position.y;
        const nodeHeight = node.height || 100;
        return (
          nodeY < Math.max(sourceY, targetY) + margin &&
          nodeY + nodeHeight > Math.min(sourceY, targetY) - margin
        );
      });

      if (hasVerticalConflict) {
        // 水平距离足够，使用贝塞尔曲线
        return calculateBezierPath(sourceX, sourceY, targetX, targetY, curvature);
      } else {
        // 简单情况使用直线
        return calculateStraightPath(sourceX, sourceY, targetX, targetY);
      }
  }
}

/**
 * Canvas 性能优化 Hook
 */
export function useCanvasPerformance(
  nodes: Node[],
  edges: Edge[],
  options: UseCanvasPerformanceOptions = {}
): UseCanvasPerformanceReturn {
  const {
    virtualizationThreshold = 100,
    debounceMs = 150,
    enablePerformanceMonitor = true,
  } = options;

  const [viewport, setViewportState] = useState<Viewport>({ x: 0, y: 0, zoom: 1 });
  const [canvasSize, setCanvasSize] = useState({ width: 1920, height: 1080 });
  const [metrics, setMetrics] = useState<PerformanceMetrics>({
    nodeCount: 0,
    edgeCount: 0,
    visibleNodeCount: 0,
    renderTime: 0,
    fps: 60,
  });
  const [isLoading, setIsLoading] = useState(false);
  const [fps, setFps] = useState<number>(60);

  const frameCountRef = useRef(0);
  const lastTimeRef = useRef(performance.now());
  const rafRef = useRef<number | undefined>(undefined);

  // FPS 计算
  useEffect(() => {
    if (!enablePerformanceMonitor) return;

    const updateFps = () => {
      frameCountRef.current++;
      const now = performance.now();
      const delta = now - lastTimeRef.current;

      if (delta >= 1000) {
        setFps(Math.round((frameCountRef.current * 1000) / delta));
        frameCountRef.current = 0;
        lastTimeRef.current = now;
      }

      rafRef.current = requestAnimationFrame(updateFps);
    };

    rafRef.current = requestAnimationFrame(updateFps);

    return () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
      }
    };
  }, [enablePerformanceMonitor]);

  // 防抖设置视口
  const setViewport = useCallback(
    (newViewport: Viewport) => {
      setIsLoading(true);
      setTimeout(() => {
        setViewportState(newViewport);
        setIsLoading(false);
      }, debounceMs);
    },
    [debounceMs]
  );

  // 计算可见节点
  const visibleData = useMemo(() => {
    const startTime = performance.now();

    if (nodes.length < virtualizationThreshold) {
      // 节点数量少，不需要虚拟化
      const renderTime = performance.now() - startTime;
      setMetrics((prev) => ({
        ...prev,
        nodeCount: nodes.length,
        edgeCount: edges.length,
        visibleNodeCount: nodes.length,
        renderTime,
        fps,
      }));

      return {
        visibleNodes: nodes,
        visibleEdges: edges,
      };
    }

    // 虚拟化：只渲染视口内的节点
    const visibleNodes = nodes.filter((node) =>
      isNodeInViewport(node, viewport, canvasSize.width, canvasSize.height)
    );

    const visibleNodeIds = new Set(visibleNodes.map((n) => n.id));
    const visibleEdges = edges.filter(
      (edge) =>
        visibleNodeIds.has(edge.source) || visibleNodeIds.has(edge.target)
    );

    const renderTime = performance.now() - startTime;

    setMetrics((prev) => ({
      ...prev,
      nodeCount: nodes.length,
      edgeCount: edges.length,
      visibleNodeCount: visibleNodes.length,
      renderTime,
      fps,
    }));

    return {
      visibleNodes,
      visibleEdges,
    };
  }, [nodes, edges, viewport, canvasSize, virtualizationThreshold, fps]);

  // 强制更新
  const forceUpdate = useCallback(() => {
    setViewportState({ ...viewport });
  }, [viewport]);

  return {
    ...visibleData,
    metrics,
    isLoading,
    fps,
    setViewport,
    forceUpdate,
  };
}

/**
 * 批量操作优化 - 使用事务性更新减少重渲染
 */
export function useBatchUpdates() {
  const pendingUpdatesRef = useRef<(() => void)[]>([]);
  const rafRef = useRef<number | undefined>(undefined);

  const scheduleUpdate = useCallback((update: () => void) => {
    pendingUpdatesRef.current.push(update);
    
    if (!rafRef.current) {
      rafRef.current = requestAnimationFrame(() => {
        const updates = pendingUpdatesRef.current;
        pendingUpdatesRef.current = [];
        
        updates.forEach((update) => update());
        rafRef.current = undefined;
      });
    }
  }, []);

  useEffect(() => {
    return () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
      }
    };
  }, []);

  return { scheduleUpdate };
}

export default useCanvasPerformance;
