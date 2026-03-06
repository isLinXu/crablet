import React from 'react';
import type { TaskGraph, TaskNode } from '@/types/domain';
import { CheckSquare, Square, Pause, Play, Plus, Copy } from 'lucide-react';
import { SwarmFlowGraph } from '../SwarmFlowGraph';

interface GraphListProps {
    graphs: TaskGraph[];
    selectedGraphIds: Set<string>;
    toggleSelectGraph: (id: string) => void;
    onPause: (id: string) => void;
    onResume: (id: string) => void;
    onAdd: (id: string) => void;
    onTemplate: (id: string) => void;
    onNodeClick: (graph: TaskGraph, node: TaskNode) => void;
}

export const GraphList: React.FC<GraphListProps> = ({
    graphs,
    selectedGraphIds,
    toggleSelectGraph,
    onPause,
    onResume,
    onAdd,
    onTemplate,
    onNodeClick
}) => {
    return (
        <div className="lg:col-span-2 space-y-8">
            {graphs.map((graph, idx) => (
                <div key={graph.id || idx} className="flex gap-2 items-start border-b pb-8 last:border-0">
                    <button 
                        onClick={() => graph.id && toggleSelectGraph(graph.id)}
                        className="mt-4 text-gray-400 hover:text-blue-500 flex-shrink-0"
                    >
                        {graph.id && selectedGraphIds.has(graph.id) ? <CheckSquare size={20} className="text-blue-500" /> : <Square size={20} />}
                    </button>
                    <div className="flex-1 min-w-0 space-y-3">
                         <div className="flex justify-between items-center">
                            <div className="flex items-center gap-2">
                                <h3 className="font-semibold text-gray-700 dark:text-gray-300">
                                    Graph {graph.id?.substring(0, 8)}
                                </h3>
                                <span className={`px-2 py-0.5 rounded text-xs font-medium ${
                                    graph.status === 'Paused' ? 'bg-yellow-100 text-yellow-800' :
                                    graph.status === 'Completed' ? 'bg-green-100 text-green-800' :
                                    graph.status === 'Failed' ? 'bg-red-100 text-red-800' :
                                    'bg-blue-100 text-blue-800'
                                }`}>
                                    {graph.status || 'Active'}
                                </span>
                            </div>
                            <div className="flex gap-2">
                                {graph.status === 'Active' && graph.id && (
                                    <button onClick={() => onPause(graph.id!)} className="p-1.5 hover:bg-gray-100 rounded-full text-gray-600" title="Pause">
                                        <Pause size={16} />
                                    </button>
                                )}
                                {graph.status === 'Paused' && graph.id && (
                                    <button onClick={() => onResume(graph.id!)} className="p-1.5 hover:bg-gray-100 rounded-full text-gray-600" title="Resume">
                                        <Play size={16} />
                                    </button>
                                )}
                                {graph.id && (
                                    <button 
                                        onClick={() => onAdd(graph.id!)} 
                                        className="p-1.5 hover:bg-gray-100 rounded-full text-gray-600" 
                                        title="Add Task"
                                    >
                                        <Plus size={16} />
                                    </button>
                                )}
                                {graph.id && (
                                    <button 
                                        onClick={() => onTemplate(graph.id!)} 
                                        className="p-1.5 hover:bg-gray-100 rounded-full text-gray-600" 
                                        title="Save as Template"
                                    >
                                        <Copy size={16} />
                                    </button>
                                )}
                            </div>
                         </div>
                        <SwarmFlowGraph 
                            graph={graph} 
                            onNodeClick={(node) => onNodeClick(graph, node)}
                        />
                    </div>
                </div>
            ))}
        </div>
    );
};
