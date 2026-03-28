/**
 * Edge Routing Hook - 连接线自动路由算法
 * 实现平滑的贝塞尔曲线自动路由
 */

import { useCallback, useMemo } from 'react';
import type { Node, Edge, Connection } from '@xyflow/react';

export interface RouteOptions {
  type?: 'smoothstep' | 'bezier' | 'straight' | 'step';
  curvature?: number;
  borderRadius?: number;
}

export interface UseEdgeRoutingReturn {
  routeEdges: (edges: Edge[], nodes: Node[], options?: RouteOptions) => Edge[];
  getEdgePath: (source: Node, target: Node, type?: string) => string;
  optimizeLayout: (nodes: Node[], edges: Edge[]) => Node[];
}

/**
 * 贝塞尔曲线路径计算
 */
function getBezierPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
  curvature: number = 0.5
): string {
  const deltaX = Math.abs(targetX - sourceX) * curvature;
  const deltaY = Math.abs(targetY - sourceY) * curvature;
  
  return `M${sourceX},${sourceY} C${sourceX + deltaX},${sourceY} ${targetX - deltaX},${targetY} ${targetX},${targetY}`;
}

/**
 * 获取直线路径
 */
function getStraightPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number
): string {
  return `M${sourceX},${sourceY} L${targetX},${targetY}`;
}

/**
 * 获取阶梯路径 (正交路由)
 */
function getStepPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
  borderRadius: number = 20
): string {
  const midX = (sourceX + targetX) / 2;
  
  // 如果水平距离大于垂直距离，水平先走
  if (Math.abs(targetX - sourceX) > Math.abs(targetY - sourceY)) {
    return `M${sourceX},${sourceY} 
            H${Math.max(sourceX + borderRadius, Math.min(midX, targetX - borderRadius))} 
            V${targetY} 
            H${targetX}`;
  } else {
    return `M${sourceX},${sourceY} 
            V${Math.max(sourceY + borderRadius, Math.min(midX, targetY - borderRadius))} 
            H${targetX} 
            V${targetY}`;
  }
}

/**
 * 获取 Smoothstep 路径 (ReactFlow 默认)
 */
function getSmoothstepPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number
): string {
  const horizontalDist = Math.abs(targetX - sourceX);
  const verticalDist = Math.abs(targetY - sourceY);
  
  // 平滑阶梯曲线
  const controlOffset = Math.min(horizontalDist / 2, 50);
  
  if (horizontalDist > verticalDist) {
    // 水平主导
    return `M${sourceX},${sourceY} 
            C${sourceX + controlOffset},${sourceY} 
              ${targetX - controlOffset},${targetY} 
              ${targetX},${targetY}`;
  } else {
    // 垂直主导
    return `M${sourceX},${sourceY} 
            C${sourceX},${sourceY + controlOffset} 
              ${targetX},${targetY - controlOffset} 
              ${targetX},${targetY}`;
  }
}

/**
 * 边路由 Hook
 */
export function useEdgeRouting(): UseEdgeRoutingReturn {
  /**
   * 路由所有边
   */
  const routeEdges = useCallback((
    edges: Edge[],
    nodes: Node[],
    options: RouteOptions = {}
  ): Edge[] => {
    const { type = 'smoothstep', curvature = 0.5, borderRadius = 20 } = options;
    
    // 创建节点位置映射
    const nodeMap = new Map(nodes.map(n => [n.id, n]));
    
    return edges.map(edge => {
      const source = nodeMap.get(edge.source);
      const target = nodeMap.get(edge.target);
      
      if (!source || !target) {
        return edge;
      }
      
      // 计算源和目标的位置
      const sourceX = source.position.x + 90; // 假设节点宽度 180 / 2
      const sourceY = source.position.y + 50;  // 假设节点高度 100 / 2
      const targetX = target.position.x + 90;
      const targetY = target.position.y + 50;
      
      // 根据类型计算路径
      let path: string;
      let animated = false;
      
      switch (type) {
        case 'bezier':
          path = getBezierPath(sourceX, sourceY, targetX, targetY, curvature);
          animated = true;
          break;
        case 'straight':
          path = getStraightPath(sourceX, sourceY, targetX, targetY);
          break;
        case 'step':
          path = getStepPath(sourceX, sourceY, targetX, targetY, borderRadius);
          break;
        case 'smoothstep':
        default:
          path = getSmoothstepPath(sourceX, sourceY, targetX, targetY);
          animated = true;
          break;
      }
      
      return {
        ...edge,
        type: 'smoothstep',
        animated,
        style: {
          ...edge.style,
          stroke: edge.style?.stroke || '#6366f1',
          strokeWidth: edge.style?.strokeWidth || 2,
        },
      };
    });
  }, []);
  
  /**
   * 获取单条边的路径
   */
  const getEdgePath = useCallback((
    source: Node,
    target: Node,
    edgeType?: string
  ): string => {
    const sourceX = source.position.x + 90;
    const sourceY = source.position.y + 50;
    const targetX = target.position.x + 90;
    const targetY = target.position.y + 50;
    
    switch (edgeType) {
      case 'bezier':
        return getBezierPath(sourceX, sourceY, targetX, targetY);
      case 'straight':
        return getStraightPath(sourceX, sourceY, targetX, targetY);
      case 'step':
        return getStepPath(sourceX, sourceY, targetX, targetY);
      default:
        return getSmoothstepPath(sourceX, sourceY, targetX, targetY);
    }
  }, []);
  
  /**
   * 优化布局 - 减少边交叉
   */
  const optimizeLayout = useCallback((nodes: Node[], edges: Edge[]): Node[] => {
    // 简单的层次布局优化
    const newNodes = nodes.map(n => ({ ...n }));
    
    // 计算每个节点的入度和出度
    const inDegree = new Map<string, number>();
    const outDegree = new Map<string, number>();
    
    edges.forEach(edge => {
      inDegree.set(edge.target, (inDegree.get(edge.target) || 0) + 1);
      outDegree.set(edge.source, (outDegree.get(edge.source) || 0) + 1);
    });
    
    // 按度数排序，优先放置中心节点
    newNodes.sort((a, b) => {
      const degreeA = (inDegree.get(a.id) || 0) + (outDegree.get(a.id) || 0);
      const degreeB = (inDegree.get(b.id) || 0) + (outDegree.get(b.id) || 0);
      return degreeB - degreeA;
    });
    
    return newNodes;
  }, []);
  
  return {
    routeEdges,
    getEdgePath,
    optimizeLayout,
  };
}

export default useEdgeRouting;