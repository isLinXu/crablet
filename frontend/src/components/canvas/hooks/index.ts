/**
 * Canvas Hooks Index
 * 导出所有 Canvas 相关的 Hooks
 */

export { useHistory, createSnapshot, type CanvasSnapshot } from './useHistory';
export { useClipboard, type ClipboardData } from './useClipboard';
export { useSelection } from './useSelection';
export { useEdgeRouting, type RouteOptions } from './useEdgeRouting';
export { useCanvasState, type CanvasStateOptions } from './useCanvasState';
export { useVersionHistory, createVersion, computeDiff, type WorkflowVersion, type VersionDiff } from './useVersionHistory';
export { useCanvasPerformance, useBatchUpdates, calculateAutoRoute, type PerformanceMetrics } from './useCanvasPerformance';