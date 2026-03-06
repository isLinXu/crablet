import React, { useState, useEffect } from 'react';
import type { TaskNode } from '@/types/domain';
import { Modal } from '../../ui/Modal';
import { Edit2, Plus, RotateCcw } from 'lucide-react';

interface NodeDetailsModalProps {
    isOpen: boolean;
    onClose: () => void;
    node: TaskNode;
    graphStatus: string;
    existingNodes: TaskNode[];
    onUpdatePrompt: (prompt: string, dependencies?: string[]) => void;
    onRetry: () => void;
}

export const NodeDetailsModal: React.FC<NodeDetailsModalProps> = ({ 
    isOpen, 
    onClose, 
    node, 
    graphStatus, 
    existingNodes, 
    onUpdatePrompt, 
    onRetry 
}) => {
    const [isEditing, setIsEditing] = useState(false);
    const [prompt, setPrompt] = useState(node.prompt);
    const [dependencies, setDependencies] = useState<string[]>(node.dependencies || []);
    
    useEffect(() => {
        setPrompt(node.prompt);
        setDependencies(node.dependencies || []);
    }, [node]);

    const canEdit = graphStatus === 'Paused' && (typeof node.status === 'string' && node.status === 'Pending');
    const isFailed = typeof node.status === 'object' && 'Failed' in node.status;
    const isCompleted = typeof node.status === 'object' && 'Completed' in node.status;

    const handleSave = () => {
        onUpdatePrompt(prompt, dependencies);
        setIsEditing(false);
    };

    const toggleDependency = (id: string) => {
        setDependencies(prev => 
            prev.includes(id) ? prev.filter(d => d !== id) : [...prev, id]
        );
    };

    return (
        <Modal isOpen={isOpen} onClose={onClose} title={`Task Details: ${node.agent_role}`}>
            <div className="space-y-6">
                <div>
                    <div className="flex justify-between items-center mb-2">
                        <h3 className="font-medium text-gray-700 dark:text-gray-300">Prompt</h3>
                        {canEdit && !isEditing && (
                            <button 
                                onClick={() => setIsEditing(true)}
                                className="text-sm text-blue-600 hover:text-blue-700 flex items-center gap-1"
                            >
                                <Edit2 size={14} /> Edit
                            </button>
                        )}
                    </div>
                    
                    {isEditing ? (
                        <div className="space-y-4">
                            <textarea 
                                value={prompt}
                                onChange={(e) => setPrompt(e.target.value)}
                                className="w-full p-2 border rounded-md dark:bg-gray-700 dark:border-gray-600 min-h-[100px]"
                            />
                            
                            <div>
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                                    Dependencies (DAG)
                                </label>
                                <div className="max-h-32 overflow-y-auto border rounded-md p-2 space-y-1 dark:border-gray-600 dark:bg-gray-700/50">
                                    {existingNodes.filter(n => n.id !== node.id).map(n => (
                                        <div 
                                            key={n.id} 
                                            onClick={() => toggleDependency(n.id)}
                                            className={`flex items-center gap-2 p-2 rounded cursor-pointer text-sm ${dependencies.includes(n.id) ? 'bg-blue-50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800' : 'hover:bg-gray-100 dark:hover:bg-gray-700'}`}
                                        >
                                            <div className={`w-4 h-4 rounded border flex items-center justify-center ${dependencies.includes(n.id) ? 'bg-blue-500 border-blue-500' : 'border-gray-300'}`}>
                                                {dependencies.includes(n.id) && <Plus size={10} className="text-white" />}
                                            </div>
                                            <div className="flex-1 min-w-0">
                                                <div className="font-medium truncate">{n.agent_role}</div>
                                                <div className="text-xs text-gray-500 truncate">{n.prompt}</div>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            </div>

                            <div className="flex justify-end gap-2">
                                <button 
                                    onClick={() => setIsEditing(false)}
                                    className="px-3 py-1 text-sm text-gray-600 hover:bg-gray-100 rounded"
                                >
                                    Cancel
                                </button>
                                <button 
                                    onClick={handleSave}
                                    className="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700"
                                >
                                    Save
                                </button>
                            </div>
                        </div>
                    ) : (
                        <div className="space-y-2">
                            <div className="p-3 bg-gray-50 dark:bg-gray-900 rounded-md text-sm whitespace-pre-wrap">
                                {node.prompt}
                            </div>
                            {node.dependencies && node.dependencies.length > 0 && (
                                <div className="flex flex-wrap gap-1">
                                    {node.dependencies.map(depId => (
                                        <span key={depId} className="px-2 py-0.5 bg-gray-100 dark:bg-gray-800 text-xs rounded text-gray-500">
                                            Dep: {existingNodes.find(n => n.id === depId)?.agent_role || depId.substring(0, 8)}
                                        </span>
                                    ))}
                                </div>
                            )}
                        </div>
                    )}
                </div>

                <div>
                    <div className="flex justify-between items-center mb-2">
                        <h3 className="font-medium text-gray-700 dark:text-gray-300">Status</h3>
                        {(isFailed || isCompleted) && (
                            <button 
                                onClick={onRetry}
                                className="text-sm text-orange-600 hover:text-orange-700 flex items-center gap-1 px-2 py-1 bg-orange-50 rounded hover:bg-orange-100 dark:bg-orange-900/20"
                            >
                                <RotateCcw size={14} /> Retry
                            </button>
                        )}
                    </div>
                    <div className="p-3 bg-gray-50 dark:bg-gray-900 rounded-md text-sm font-mono">
                        {JSON.stringify(node.status, null, 2)}
                    </div>
                </div>

                {node.result && (
                    <div>
                        <h3 className="font-medium text-gray-700 dark:text-gray-300 mb-2">Result</h3>
                        <div className="p-3 bg-green-50 dark:bg-green-900/20 rounded-md text-sm whitespace-pre-wrap">
                            {node.result}
                        </div>
                    </div>
                )}
                
                 <div>
                    <h3 className="font-medium text-gray-700 dark:text-gray-300 mb-2">Execution Logs</h3>
                    <div className="p-3 bg-gray-900 text-gray-300 rounded-md text-xs font-mono max-h-48 overflow-y-auto">
                        {(node as any).logs && (node as any).logs.length > 0 ? (
                            (node as any).logs.map((log: string, i: number) => (
                                <div key={i} className="mb-1 border-b border-gray-800 pb-1 last:border-0">{log}</div>
                            ))
                        ) : (
                            <span className="text-gray-500 italic">No logs available</span>
                        )}
                    </div>
                </div>
            </div>
        </Modal>
    );
};
