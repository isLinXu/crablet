/**
 * Canvas Layout Algorithms
 * Smart auto-layout for workflow nodes
 */

import type { Node, Edge } from '@xyflow/react';

export interface LayoutOptions {
  direction?: 'TB' | 'LR' | 'RL' | 'BT'; // Top-Bottom, Left-Right, etc.
  nodeWidth?: number;
  nodeHeight?: number;
  horizontalSpacing?: number;
  verticalSpacing?: number;
  align?: 'center' | 'left' | 'right';
}

/**
 * Calculate node levels based on edges (topological sort)
 */
function calculateNodeLevels(nodes: Node[], edges: Edge[]): Map<string, number> {
  const levels = new Map<string, number>();
  const inDegree = new Map<string, number>();
  
  // Initialize
  nodes.forEach(node => {
    levels.set(node.id, 0);
    inDegree.set(node.id, 0);
  });
  
  // Calculate in-degrees
  edges.forEach(edge => {
    inDegree.set(edge.target, (inDegree.get(edge.target) || 0) + 1);
  });
  
  // Topological sort using Kahn's algorithm
  const queue: string[] = [];
  nodes.forEach(node => {
    if (inDegree.get(node.id) === 0) {
      queue.push(node.id);
    }
  });
  
  while (queue.length > 0) {
    const nodeId = queue.shift()!;
    const currentLevel = levels.get(nodeId) || 0;
    
    // Find all outgoing edges
    edges.filter(e => e.source === nodeId).forEach(edge => {
      const targetLevel = levels.get(edge.target) || 0;
      levels.set(edge.target, Math.max(targetLevel, currentLevel + 1));
      
      const newInDegree = (inDegree.get(edge.target) || 0) - 1;
      inDegree.set(edge.target, newInDegree);
      
      if (newInDegree === 0) {
        queue.push(edge.target);
      }
    });
  }
  
  return levels;
}

/**
 * Group nodes by their levels
 */
function groupNodesByLevel(nodes: Node[], levels: Map<string, number>): Map<number, Node[]> {
  const groups = new Map<number, Node[]>();
  
  nodes.forEach(node => {
    const level = levels.get(node.id) || 0;
    if (!groups.has(level)) {
      groups.set(level, []);
    }
    groups.get(level)!.push(node);
  });
  
  return groups;
}

/**
 * Apply hierarchical layout (Top-Bottom)
 */
export function applyHierarchicalLayout(
  nodes: Node[],
  edges: Edge[],
  options: LayoutOptions = {}
): Node[] {
  const {
    nodeWidth = 200,
    nodeHeight = 100,
    horizontalSpacing = 250,
    verticalSpacing = 150,
    align = 'center',
  } = options;
  
  const levels = calculateNodeLevels(nodes, edges);
  const groups = groupNodesByLevel(nodes, levels);
  
  const newNodes: Node[] = [];
  const levelKeys = Array.from(groups.keys()).sort((a, b) => a - b);
  
  levelKeys.forEach((level, levelIndex) => {
    const levelNodes = groups.get(level)!;
    const levelWidth = levelNodes.length * nodeWidth + (levelNodes.length - 1) * horizontalSpacing;
    
    let startX: number;
    switch (align) {
      case 'left':
        startX = 0;
        break;
      case 'right':
        startX = -levelWidth;
        break;
      case 'center':
      default:
        startX = -levelWidth / 2;
        break;
    }
    
    levelNodes.forEach((node, index) => {
      newNodes.push({
        ...node,
        position: {
          x: startX + index * (nodeWidth + horizontalSpacing) + nodeWidth / 2,
          y: levelIndex * verticalSpacing + 100,
        },
      });
    });
  });
  
  return newNodes;
}

/**
 * Apply tree layout
 */
export function applyTreeLayout(
  nodes: Node[],
  edges: Edge[],
  options: LayoutOptions = {}
): Node[] {
  const {
    nodeWidth = 200,
    horizontalSpacing = 250,
    verticalSpacing = 150,
  } = options;
  
  // Build adjacency list
  const children = new Map<string, string[]>();
  edges.forEach(edge => {
    if (!children.has(edge.source)) {
      children.set(edge.source, []);
    }
    children.get(edge.source)!.push(edge.target);
  });
  
  // Find root (node with no incoming edges)
  const hasIncoming = new Set(edges.map(e => e.target));
  const root = nodes.find(n => !hasIncoming.has(n.id));
  
  if (!root) {
    return applyHierarchicalLayout(nodes, edges, options);
  }
  
  const newNodes: Node[] = [];
  const positions = new Map<string, { x: number; y: number }>();
  
  // Calculate subtree widths
  function calculateSubtreeWidth(nodeId: string): number {
    const nodeChildren = children.get(nodeId) || [];
    if (nodeChildren.length === 0) {
      return nodeWidth;
    }
    
    let totalWidth = 0;
    nodeChildren.forEach(childId => {
      totalWidth += calculateSubtreeWidth(childId);
    });
    
    return totalWidth + (nodeChildren.length - 1) * horizontalSpacing;
  }
  
  // Position nodes recursively
  function positionNode(nodeId: string, x: number, y: number, availableWidth: number) {
    positions.set(nodeId, { x, y });
    
    const nodeChildren = children.get(nodeId) || [];
    if (nodeChildren.length === 0) return;
    
    const subtreeWidth = calculateSubtreeWidth(nodeId);
    let currentX = x - subtreeWidth / 2;
    
    nodeChildren.forEach(childId => {
      const childWidth = calculateSubtreeWidth(childId);
      positionNode(
        childId,
        currentX + childWidth / 2,
        y + verticalSpacing,
        childWidth
      );
      currentX += childWidth + horizontalSpacing;
    });
  }
  
  positionNode(root.id, 0, 100, calculateSubtreeWidth(root.id));
  
  nodes.forEach(node => {
    const pos = positions.get(node.id);
    if (pos) {
      newNodes.push({
        ...node,
        position: pos,
      });
    } else {
      newNodes.push(node);
    }
  });
  
  return newNodes;
}

/**
 * Apply force-directed layout
 */
export function applyForceLayout(
  nodes: Node[],
  edges: Edge[],
  iterations: number = 100
): Node[] {
  const newNodes = nodes.map(n => ({ ...n }));
  const positions = new Map<string, { x: number; y: number }>();
  
  // Initialize random positions
  newNodes.forEach(node => {
    positions.set(node.id, {
      x: node.position.x + (Math.random() - 0.5) * 100,
      y: node.position.y + (Math.random() - 0.5) * 100,
    });
  });
  
  const repulsionForce = 1000;
  const attractionForce = 0.01;
  const idealDistance = 200;
  
  for (let i = 0; i < iterations; i++) {
    // Calculate repulsion
    newNodes.forEach(nodeA => {
      let fx = 0;
      let fy = 0;
      
      newNodes.forEach(nodeB => {
        if (nodeA.id === nodeB.id) return;
        
        const posA = positions.get(nodeA.id)!;
        const posB = positions.get(nodeB.id)!;
        
        const dx = posA.x - posB.x;
        const dy = posA.y - posB.y;
        const distance = Math.sqrt(dx * dx + dy * dy) || 1;
        
        const force = repulsionForce / (distance * distance);
        fx += (dx / distance) * force;
        fy += (dy / distance) * force;
      });
      
      // Apply attraction along edges
      edges.forEach(edge => {
        if (edge.source === nodeA.id || edge.target === nodeA.id) {
          const otherId = edge.source === nodeA.id ? edge.target : edge.source;
          const otherPos = positions.get(otherId)!;
          const posA = positions.get(nodeA.id)!;
          
          const dx = otherPos.x - posA.x;
          const dy = otherPos.y - posA.y;
          const distance = Math.sqrt(dx * dx + dy * dy) || 1;
          
          const force = (distance - idealDistance) * attractionForce;
          fx += (dx / distance) * force;
          fy += (dy / distance) * force;
        }
      });
      
      // Update position
      const pos = positions.get(nodeA.id)!;
      positions.set(nodeA.id, {
        x: pos.x + fx * 0.1,
        y: pos.y + fy * 0.1,
      });
    });
  }
  
  // Apply final positions
  newNodes.forEach(node => {
    const pos = positions.get(node.id);
    if (pos) {
      node.position = pos;
    }
  });
  
  return newNodes;
}

/**
 * Auto layout based on graph structure
 */
export function autoLayout(nodes: Node[], edges: Edge[]): Node[] {
  if (nodes.length === 0) return nodes;
  if (nodes.length <= 3) {
    // Simple vertical layout for small graphs
    return nodes.map((node, index) => ({
      ...node,
      position: {
        x: 100,
        y: index * 150 + 100,
      },
    }));
  }
  
  // Check if it's a tree structure
  const hasMultipleParents = new Set();
  edges.forEach(edge => {
    if (hasMultipleParents.has(edge.target)) {
      // Not a tree, use hierarchical
      return applyHierarchicalLayout(nodes, edges);
    }
    hasMultipleParents.add(edge.target);
  });
  
  // Use tree layout for tree structures
  return applyTreeLayout(nodes, edges);
}

/**
 * Check if layout would cause overlaps
 */
export function wouldOverlap(
  nodes: Node[],
  minDistance: number = 50
): boolean {
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      const dx = nodes[i].position.x - nodes[j].position.x;
      const dy = nodes[i].position.y - nodes[j].position.y;
      const distance = Math.sqrt(dx * dx + dy * dy);
      
      if (distance < minDistance) {
        return true;
      }
    }
  }
  return false;
}

/**
 * Fix overlapping nodes
 */
export function fixOverlaps(nodes: Node[], minDistance: number = 200): Node[] {
  const newNodes = nodes.map(n => ({ ...n }));
  let iterations = 0;
  const maxIterations = 100;
  
  while (wouldOverlap(newNodes, minDistance) && iterations < maxIterations) {
    for (let i = 0; i < newNodes.length; i++) {
      for (let j = i + 1; j < newNodes.length; j++) {
        const dx = newNodes[i].position.x - newNodes[j].position.x;
        const dy = newNodes[i].position.y - newNodes[j].position.y;
        const distance = Math.sqrt(dx * dx + dy * dy) || 1;
        
        if (distance < minDistance) {
          const overlap = minDistance - distance;
          const moveX = (dx / distance) * overlap * 0.5;
          const moveY = (dy / distance) * overlap * 0.5;
          
          newNodes[i].position.x += moveX;
          newNodes[i].position.y += moveY;
          newNodes[j].position.x -= moveX;
          newNodes[j].position.y -= moveY;
        }
      }
    }
    iterations++;
  }
  
  return newNodes;
}
