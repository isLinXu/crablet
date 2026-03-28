/**
 * SelectionBox Component - 框选组件
 * 用于在画布上通过拖拽框选多个节点
 */

import React, { useCallback, useState, useRef, useEffect } from 'react';

export interface SelectionBoxProps {
  onSelectionChange: (selectedIds: string[]) => void;
  getNodesInRect: (rect: SelectionRect) => string[];
}

export interface SelectionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export const SelectionBox: React.FC<SelectionBoxProps> = ({
  onSelectionChange,
  getNodesInRect,
}) => {
  const [isSelecting, setIsSelecting] = useState(false);
  const [startPoint, setStartPoint] = useState<{ x: number; y: number } | null>(null);
  const [currentPoint, setCurrentPoint] = useState<{ x: number; y: number } | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  
  const containerRef = useRef<HTMLDivElement>(null);
  
  // 计算选框矩形
  const getSelectionRect = useCallback((): SelectionRect | null => {
    if (!startPoint || !currentPoint) return null;
    
    return {
      x: Math.min(startPoint.x, currentPoint.x),
      y: Math.min(startPoint.y, currentPoint.y),
      width: Math.abs(currentPoint.x - startPoint.x),
      height: Math.abs(currentPoint.y - startPoint.y),
    };
  }, [startPoint, currentPoint]);
  
  // 开始框选
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // 只在画布空白处触发
    if ((e.target as HTMLElement).closest('.react-flow__node')) return;
    
    e.preventDefault();
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    setIsSelecting(true);
    setStartPoint({
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    });
    setCurrentPoint({
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    });
    setSelectedIds([]);
  }, []);
  
  // 更新框选范围
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!isSelecting) return;
    
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    setCurrentPoint({
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    });
  }, [isSelecting]);
  
  // 完成框选
  const handleMouseUp = useCallback(() => {
    if (!isSelecting) return;
    
    const selectionRect = getSelectionRect();
    if (selectionRect && selectionRect.width > 5 && selectionRect.height > 5) {
      const ids = getNodesInRect(selectionRect);
      setSelectedIds(ids);
      onSelectionChange(ids);
    }
    
    setIsSelecting(false);
    setStartPoint(null);
    setCurrentPoint(null);
  }, [isSelecting, getSelectionRect, getNodesInRect, onSelectionChange]);
  
  // 渲染选框
  const selectionRect = getSelectionRect();
  
  if (!isSelecting || !selectionRect) return null;
  
  return (
    <div
      ref={containerRef}
      className="selection-box-container"
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
      style={{
        position: 'absolute',
        inset: 0,
        zIndex: 1000,
        cursor: 'crosshair',
      }}
    >
      <svg
        style={{
          position: 'absolute',
          inset: 0,
          width: '100%',
          height: '100%',
          pointerEvents: 'none',
        }}
      >
        <rect
          x={selectionRect.x}
          y={selectionRect.y}
          width={selectionRect.width}
          height={selectionRect.height}
          fill="rgba(59, 130, 246, 0.1)"
          stroke="#3b82f6"
          strokeWidth={1}
          strokeDasharray="4"
        />
      </svg>
    </div>
  );
};

export default SelectionBox;