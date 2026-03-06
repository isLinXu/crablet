import React, { useState, useEffect } from 'react';
import { Modal } from '../ui/Modal';
import type { TaskNode, AgentInfo } from '@/types/domain';
import { Plus, Info } from 'lucide-react';
import { dashboardService } from '@/services/dashboardService';

interface AddTaskModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (role: string, prompt: string, dependencies: string[]) => Promise<void>;
    existingNodes: TaskNode[];
}

export const AddTaskModal: React.FC<AddTaskModalProps> = ({ isOpen, onClose, onSubmit, existingNodes }) => {
    const [role, setRole] = useState('researcher');
    const [prompt, setPrompt] = useState('');
    const [dependencies, setDependencies] = useState<string[]>([]);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [availableRoles, setAvailableRoles] = useState<AgentInfo[]>([]);

    useEffect(() => {
        if (isOpen) {
            dashboardService.getSwarmAgents().then(agents => {
                setAvailableRoles(agents);
                if (agents.length > 0 && !agents.some(a => a.name === role)) {
                    setRole(agents[0].name);
                }
            }).catch(console.error);
        }
    }, [isOpen]);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setIsSubmitting(true);
        try {
            await onSubmit(role, prompt, dependencies);
            onClose();
            // Reset form
            setRole('researcher');
            setPrompt('');
            setDependencies([]);
        } finally {
            setIsSubmitting(false);
        }
    };

    const toggleDependency = (id: string) => {
        setDependencies(prev => 
            prev.includes(id) ? prev.filter(d => d !== id) : [...prev, id]
        );
    };

    const selectedAgent = availableRoles.find(r => r.name === role);

    return (
        <Modal isOpen={isOpen} onClose={onClose} title="Add New Task">
            <form onSubmit={handleSubmit} className="space-y-4">
                <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Agent Role
                    </label>
                    <select
                        value={role}
                        onChange={(e) => setRole(e.target.value)}
                        className="w-full rounded-md border border-gray-300 dark:border-gray-600 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-700 dark:text-white"
                    >
                        {availableRoles.length > 0 ? availableRoles.map(r => (
                            <option key={r.name} value={r.name}>{r.name}</option>
                        )) : (
                            <option value="researcher">researcher</option>
                        )}
                    </select>
                    
                    {selectedAgent && (
                        <div className="mt-2 p-2 bg-gray-50 dark:bg-gray-800 rounded border dark:border-gray-700 text-xs text-gray-600 dark:text-gray-400">
                            <p className="font-medium mb-1 flex items-center gap-1">
                                <Info size={12} /> {selectedAgent.description}
                            </p>
                            <div className="flex flex-wrap gap-1">
                                {selectedAgent.capabilities.map(cap => (
                                    <span key={cap} className="px-1.5 py-0.5 bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-300 rounded text-[10px]">
                                        {cap}
                                    </span>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
                
                <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Task Prompt
                    </label>
                    <textarea
                        value={prompt}
                        onChange={(e) => setPrompt(e.target.value)}
                        required
                        rows={3}
                        className="w-full rounded-md border border-gray-300 dark:border-gray-600 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-700 dark:text-white"
                        placeholder="Describe what the agent should do..."
                    />
                </div>
                
                <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                        Dependencies (Optional)
                    </label>
                    <div className="max-h-40 overflow-y-auto border rounded-md p-2 space-y-1 dark:border-gray-600 dark:bg-gray-700/50">
                        {existingNodes.length === 0 ? (
                            <p className="text-xs text-gray-500 italic">No existing tasks to depend on.</p>
                        ) : (
                            existingNodes.map(node => (
                                <div 
                                    key={node.id} 
                                    onClick={() => toggleDependency(node.id)}
                                    className={`flex items-center gap-2 p-2 rounded cursor-pointer text-sm ${dependencies.includes(node.id) ? 'bg-blue-50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800' : 'hover:bg-gray-100 dark:hover:bg-gray-700'}`}
                                >
                                    <div className={`w-4 h-4 rounded border flex items-center justify-center ${dependencies.includes(node.id) ? 'bg-blue-500 border-blue-500' : 'border-gray-300'}`}>
                                        {dependencies.includes(node.id) && <Plus size={10} className="text-white" />}
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="font-medium truncate">{node.agent_role}</div>
                                        <div className="text-xs text-gray-500 truncate">{node.prompt}</div>
                                    </div>
                                </div>
                            ))
                        )}
                    </div>
                </div>

                <div className="flex justify-end gap-2 pt-2">
                    <button
                        type="button"
                        onClick={onClose}
                        className="px-4 py-2 text-sm font-medium text-gray-700 bg-gray-100 rounded-md hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
                    >
                        Cancel
                    </button>
                    <button
                        type="submit"
                        disabled={isSubmitting}
                        className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50 flex items-center gap-2"
                    >
                        {isSubmitting ? 'Adding...' : 'Add Task'}
                    </button>
                </div>
            </form>
        </Modal>
    );
};
