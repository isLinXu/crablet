import React, { useEffect } from 'react';
import { ReactFlow, Controls, Background, useNodesState, useEdgesState, MarkerType } from '@xyflow/react';
import type { Node, Edge, NodeProps } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import dagre from 'dagre';
import type { TaskGraph, TaskNode } from '@/types/domain';
import { Activity, CheckCircle, XCircle, Clock, Pause } from 'lucide-react';

// Dagre Layout
const getLayoutedElements = (nodes: Node[], edges: Edge[]) => {
    const dagreGraph = new dagre.graphlib.Graph();
    dagreGraph.setDefaultEdgeLabel(() => ({}));

    const nodeWidth = 250;
    const nodeHeight = 100;

    dagreGraph.setGraph({ rankdir: 'LR' });

    nodes.forEach((node) => {
        dagreGraph.setNode(node.id, { width: nodeWidth, height: nodeHeight });
    });

    edges.forEach((edge) => {
        dagreGraph.setEdge(edge.source, edge.target);
    });

    dagre.layout(dagreGraph);

    const layoutedNodes = nodes.map((node) => {
        const nodeWithPosition = dagreGraph.node(node.id);
        return {
            ...node,
            position: {
                x: nodeWithPosition.x - nodeWidth / 2,
                y: nodeWithPosition.y - nodeHeight / 2,
            },
        };
    });

    return { nodes: layoutedNodes, edges };
};

// Custom Node Component
const TaskNodeComponent = ({ data }: NodeProps) => {
    // Cast data to expected type since NodeProps is generic
    const { node, onClick } = data as unknown as { node: TaskNode, onClick: (n: TaskNode) => void };
    
    let statusColor = 'bg-white border-gray-200';
    let statusIcon = Clock;
    let statusText = 'Pending';

    if (typeof node.status === 'object') {
        if ('Running' in node.status) {
            statusColor = 'bg-blue-50 border-blue-400 ring-2 ring-blue-100';
            statusIcon = Activity;
            statusText = 'Running';
        } else if ('Paused' in node.status) {
            statusColor = 'bg-yellow-50 border-yellow-400';
            statusIcon = Pause;
            statusText = 'Paused';
        } else if ('Completed' in node.status) {
            statusColor = 'bg-green-50 border-green-400';
            statusIcon = CheckCircle;
            statusText = 'Completed';
        } else if ('Failed' in node.status) {
            statusColor = 'bg-red-50 border-red-400';
            statusIcon = XCircle;
            statusText = 'Failed';
        }
    }

    const Icon = statusIcon;

    return (
        <div 
            onClick={() => onClick(node)}
            className={`w-[240px] p-3 rounded-lg border-2 shadow-sm cursor-pointer hover:shadow-md transition-all ${statusColor} text-left`}
        >
            <div className="flex justify-between items-center mb-2">
                <span className="font-bold text-xs uppercase tracking-wider flex items-center gap-1">
                    <Icon size={14} />
                    {node.agent_role}
                </span>
                <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-black/5 font-mono">
                    {statusText}
                </span>
            </div>
            <div className="text-xs text-gray-600 line-clamp-2 font-medium" title={node.prompt}>
                {node.prompt}
            </div>
        </div>
    );
};

const nodeTypes = {
    task: TaskNodeComponent,
};

interface SwarmFlowGraphProps {
    graph: TaskGraph;
    onNodeClick: (node: TaskNode) => void;
}

export const SwarmFlowGraph: React.FC<SwarmFlowGraphProps> = ({ graph, onNodeClick }) => {
    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

    // Transform TaskGraph to ReactFlow elements
    useEffect(() => {
        const flowNodes: Node[] = [];
        const flowEdges: Edge[] = [];

        if (graph && graph.nodes) {
            Object.values(graph.nodes).forEach(task => {
                flowNodes.push({
                    id: task.id,
                    type: 'task',
                    data: { node: task, onClick: onNodeClick },
                    position: { x: 0, y: 0 }, // Will be set by dagre
                });

                if (task.dependencies) {
                    task.dependencies.forEach(depId => {
                        flowEdges.push({
                            id: `${depId}-${task.id}`,
                            source: depId,
                            target: task.id,
                            type: 'smoothstep',
                            markerEnd: { type: MarkerType.ArrowClosed },
                            animated: true,
                            style: { stroke: '#9ca3af' },
                        });
                    });
                }
            });
        }

        const { nodes: layoutedNodes, edges: layoutedEdges } = getLayoutedElements(flowNodes, flowEdges);
        setNodes(layoutedNodes);
        setEdges(layoutedEdges);
    }, [graph, onNodeClick, setNodes, setEdges]);

    return (
        <div className="h-[400px] w-full border rounded-lg bg-gray-50 dark:bg-gray-900 overflow-hidden">
            <ReactFlow
                nodes={nodes}
                edges={edges}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                nodeTypes={nodeTypes}
                fitView
                attributionPosition="bottom-right"
            >
                <Background />
                <Controls />
            </ReactFlow>
        </div>
    );
};
