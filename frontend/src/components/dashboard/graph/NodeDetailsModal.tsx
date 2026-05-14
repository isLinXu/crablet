import React, { useState, useEffect } from 'react';
import type { SwarmReplaySnapshot, SwarmTimelineEntry, TaskNode } from '@/types/domain';
import { getTaskStatusKey } from '@/types/domain';
import { dashboardService } from '@/services/dashboardService';
import { Modal } from '../../ui/Modal';
import { Edit2, Plus, RotateCcw, RefreshCw, Search, History } from 'lucide-react';

interface NodeDetailsModalProps {
    isOpen: boolean;
    onClose: () => void;
    graphId: string;
    node: TaskNode;
    graphStatus: string;
    existingNodes: TaskNode[];
    onUpdatePrompt: (prompt: string, dependencies?: string[]) => void;
    onRetry: () => void;
    onRecover: (payload: {
        agent_role?: string;
        prompt?: string;
        dependencies?: string[];
        resume_graph?: boolean;
    }) => void;
}

export const NodeDetailsModal: React.FC<NodeDetailsModalProps> = ({ 
    isOpen, 
    onClose, 
    graphId,
    node, 
    graphStatus, 
    existingNodes,
    onUpdatePrompt,
    onRetry,
    onRecover
}) => {
    const [isEditing, setIsEditing] = useState(false);
    const [isRecovering, setIsRecovering] = useState(false);
    const [timelineScope, setTimelineScope] = useState<'node' | 'graph'>('node');
    const [timeline, setTimeline] = useState<SwarmTimelineEntry[]>([]);
    const [timelineLoading, setTimelineLoading] = useState(false);
    const [timelineError, setTimelineError] = useState<string | null>(null);
    const [timelineQuery, setTimelineQuery] = useState('');
    const [timelineEventType, setTimelineEventType] = useState('all');
    const [timelineStatusFilter, setTimelineStatusFilter] = useState('all');
    const [replaySnapshot, setReplaySnapshot] = useState<SwarmReplaySnapshot | null>(null);
    const [replayLoading, setReplayLoading] = useState(false);
    const [replayError, setReplayError] = useState<string | null>(null);
    const [prompt, setPrompt] = useState(node.prompt);
    const [dependencies, setDependencies] = useState<string[]>(node.dependencies || []);
    const [recoverRole, setRecoverRole] = useState(node.agent_role);
    const [resumeGraph, setResumeGraph] = useState(false);
    
    useEffect(() => {
        setPrompt(node.prompt);
        setDependencies(node.dependencies || []);
        setRecoverRole(node.agent_role);
        setResumeGraph(false);
        setIsEditing(false);
        setIsRecovering(false);
        setTimelineScope('node');
        setTimelineQuery('');
        setTimelineEventType('all');
        setTimelineStatusFilter('all');
        setTimeline([]);
        setTimelineError(null);
        setReplaySnapshot(null);
        setReplayLoading(false);
        setReplayError(null);
    }, [graphId, node.id, isOpen]);

    const loadTimeline = async (scope: 'node' | 'graph') => {
        if (!graphId || !node.id) return;

        try {
            setTimelineLoading(true);
            setTimelineError(null);
            const entries = await dashboardService.getSwarmTimeline(
                graphId,
                {
                    nodeId: scope === 'node' ? node.id : undefined,
                    limit: 60,
                    eventType: timelineEventType,
                    status: timelineStatusFilter,
                    query: timelineQuery,
                }
            );
            setTimeline([...entries].reverse());
        } catch (error) {
            console.error(error);
            setTimelineError('Unable to load timeline for this task.');
        } finally {
            setTimelineLoading(false);
        }
    };

    useEffect(() => {
        if (!isOpen) {
            return;
        }

        loadTimeline(timelineScope);
    }, [isOpen, graphId, node.id, timelineScope, timelineEventType, timelineStatusFilter]);

    useEffect(() => {
        if (!isOpen) {
            return;
        }

        const handle = window.setTimeout(() => {
            loadTimeline(timelineScope);
        }, 250);

        return () => window.clearTimeout(handle);
    }, [timelineQuery, timelineScope, isOpen, graphId, node.id]);

    const statusKey = getTaskStatusKey(node.status);
    const canEdit = graphStatus === 'Paused' && statusKey === 'Pending';
    const canRecover = ['Failed', 'Completed', 'Cancelled', 'Paused'].includes(statusKey);
    const isFailed = statusKey === 'Failed';
    const isCompleted = statusKey === 'Completed';

    const formatTimelineTimestamp = (timestamp: number) => {
        if (!timestamp) {
            return 'Snapshot';
        }

        return new Date(timestamp).toLocaleString();
    };

    const getTimelineEntryTone = (entry: SwarmTimelineEntry) => {
        if (entry.status === 'Failed' || entry.message_type === 'Error') {
            return 'border-red-200 bg-red-50 dark:border-red-900/40 dark:bg-red-900/10';
        }

        if (entry.status === 'Completed') {
            return 'border-green-200 bg-green-50 dark:border-green-900/40 dark:bg-green-900/10';
        }

        if (entry.status === 'Cancelled' || entry.status === 'Paused') {
            return 'border-amber-200 bg-amber-50 dark:border-amber-900/40 dark:bg-amber-900/10';
        }

        if (entry.event_type === 'activity') {
            return 'border-blue-200 bg-blue-50 dark:border-blue-900/40 dark:bg-blue-900/10';
        }

        return 'border-gray-200 bg-gray-50 dark:border-gray-700 dark:bg-gray-900/40';
    };

    const timelineStatusOptions = (() => {
        const statuses = new Set<string>();
        for (const entry of timeline) {
            if (entry.status) {
                statuses.add(entry.status);
            }
        }
        return ['all', ...Array.from(statuses)];
    })();

    const loadReplay = async (timestamp: number) => {
        try {
            setReplayLoading(true);
            setReplayError(null);
            const snapshot = await dashboardService.getSwarmReplay(graphId, {
                at: timestamp,
                nodeId: node.id,
            });
            setReplaySnapshot(snapshot || null);
        } catch (error) {
            console.error(error);
            setReplayError('Unable to replay graph state for this timestamp.');
        } finally {
            setReplayLoading(false);
        }
    };

    const replayNodeId = replaySnapshot?.focus_node_id || node.id;
    const replayNode = replaySnapshot?.nodes[replayNodeId];
    const replayStatusKey = replayNode ? getTaskStatusKey(replayNode.status) : null;

    const handleRecover = () => {
        const normalizedRole = recoverRole.trim();
        const roleChanged = normalizedRole !== node.agent_role;
        const promptChanged = prompt.trim() !== node.prompt.trim();
        const dependenciesChanged =
            JSON.stringify(dependencies) !== JSON.stringify(node.dependencies || []);

        onRecover({
            agent_role: roleChanged ? normalizedRole : undefined,
            prompt: promptChanged ? prompt.trim() : undefined,
            dependencies: dependenciesChanged ? dependencies : undefined,
            resume_graph: resumeGraph,
        });
        setIsRecovering(false);
    };

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
        <Modal
            isOpen={isOpen}
            onClose={onClose}
            title={`Task Details: ${node.agent_role}`}
            className="max-w-4xl"
        >
            <div className="space-y-6">
                <div>
                    <div className="flex justify-between items-center mb-2">
                        <h3 className="font-medium text-gray-700 dark:text-gray-300">Prompt</h3>
                        <div className="flex items-center gap-2">
                            {canEdit && !isEditing && (
                                <button 
                                    onClick={() => setIsEditing(true)}
                                    className="text-sm text-blue-600 hover:text-blue-700 flex items-center gap-1"
                                >
                                    <Edit2 size={14} /> Edit
                                </button>
                            )}
                            {canRecover && !isRecovering && (
                                <button
                                    onClick={() => setIsRecovering(true)}
                                    className="text-sm text-emerald-600 hover:text-emerald-700 flex items-center gap-1"
                                >
                                    <RotateCcw size={14} /> Recover
                                </button>
                            )}
                        </div>
                    </div>

                    {isRecovering ? (
                        <div className="space-y-4">
                            <div className="grid grid-cols-1 gap-2">
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300">
                                    Agent Role
                                </label>
                                <input
                                    value={recoverRole}
                                    onChange={(e) => setRecoverRole(e.target.value)}
                                    className="w-full p-2 border rounded-md dark:bg-gray-700 dark:border-gray-600"
                                />
                            </div>
                            <div>
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                                    Prompt
                                </label>
                                <textarea 
                                    value={prompt}
                                    onChange={(e) => setPrompt(e.target.value)}
                                    className="w-full p-2 border rounded-md dark:bg-gray-700 dark:border-gray-600 min-h-[100px]"
                                />
                            </div>
                            <div>
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                                    Dependencies (DAG)
                                </label>
                                <div className="max-h-32 overflow-y-auto border rounded-md p-2 space-y-1 dark:border-gray-600 dark:bg-gray-700/50">
                                    {existingNodes.filter(n => n.id !== node.id).map(n => (
                                        <div 
                                            key={n.id} 
                                            onClick={() => toggleDependency(n.id)}
                                            className={`flex items-center gap-2 p-2 rounded cursor-pointer text-sm ${dependencies.includes(n.id) ? 'bg-emerald-50 dark:bg-emerald-900/30 border border-emerald-200 dark:border-emerald-800' : 'hover:bg-gray-100 dark:hover:bg-gray-700'}`}
                                        >
                                            <div className={`w-4 h-4 rounded border flex items-center justify-center ${dependencies.includes(n.id) ? 'bg-emerald-500 border-emerald-500' : 'border-gray-300'}`}>
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
                            <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
                                <input
                                    type="checkbox"
                                    checked={resumeGraph}
                                    onChange={(e) => setResumeGraph(e.target.checked)}
                                />
                                Resume graph after recovery
                            </label>
                            <div className="flex justify-end gap-2">
                                <button 
                                    onClick={() => setIsRecovering(false)}
                                    className="px-3 py-1 text-sm text-gray-600 hover:bg-gray-100 rounded"
                                >
                                    Cancel
                                </button>
                                <button 
                                    onClick={handleRecover}
                                    className="px-3 py-1 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-700"
                                >
                                    Recover
                                </button>
                            </div>
                        </div>
                    ) : isEditing ? (
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
                        {node.logs && node.logs.length > 0 ? (
                            node.logs.map((log: string, i: number) => (
                                <div key={i} className="mb-1 border-b border-gray-800 pb-1 last:border-0">{log}</div>
                            ))
                        ) : (
                            <span className="text-gray-500 italic">No logs available</span>
                        )}
                    </div>
                </div>

                <div>
                    <div className="flex items-center justify-between mb-2 gap-3">
                        <div>
                            <h3 className="font-medium text-gray-700 dark:text-gray-300">Execution Timeline</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                Review recovery, status transitions, and agent activity for this task.
                            </p>
                        </div>
                        <div className="flex items-center gap-2">
                            <div className="inline-flex rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
                                <button
                                    onClick={() => setTimelineScope('node')}
                                    className={`px-2.5 py-1 text-xs ${timelineScope === 'node' ? 'bg-blue-600 text-white' : 'bg-white text-gray-600 hover:bg-gray-50 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700'}`}
                                >
                                    This Node
                                </button>
                                <button
                                    onClick={() => setTimelineScope('graph')}
                                    className={`px-2.5 py-1 text-xs border-l border-gray-200 dark:border-gray-700 ${timelineScope === 'graph' ? 'bg-blue-600 text-white' : 'bg-white text-gray-600 hover:bg-gray-50 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700'}`}
                                >
                                    Full Graph
                                </button>
                            </div>
                            <button
                                onClick={() => loadTimeline(timelineScope)}
                                className="p-2 rounded-md border border-gray-200 text-gray-500 hover:text-gray-700 hover:bg-gray-50 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-800"
                                title="Refresh timeline"
                            >
                                <RefreshCw size={14} className={timelineLoading ? 'animate-spin' : ''} />
                            </button>
                        </div>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-3 gap-2 mb-3">
                        <label className="relative md:col-span-2">
                            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                            <input
                                value={timelineQuery}
                                onChange={(e) => setTimelineQuery(e.target.value)}
                                placeholder="Search timeline content, task id, actor..."
                                className="w-full rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 pl-9 pr-3 py-2 text-sm"
                            />
                        </label>
                        <div className="grid grid-cols-2 gap-2">
                            <select
                                value={timelineEventType}
                                onChange={(e) => setTimelineEventType(e.target.value)}
                                className="rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-3 py-2 text-sm"
                            >
                                <option value="all">All Events</option>
                                <option value="activity">Activity</option>
                                <option value="task_status">Task Status</option>
                                <option value="graph_status">Graph Status</option>
                                <option value="log">Logs</option>
                            </select>
                            <select
                                value={timelineStatusFilter}
                                onChange={(e) => setTimelineStatusFilter(e.target.value)}
                                className="rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-3 py-2 text-sm"
                            >
                                {timelineStatusOptions.map((status) => (
                                    <option key={status} value={status}>
                                        {status === 'all' ? 'All Statuses' : status}
                                    </option>
                                ))}
                            </select>
                        </div>
                    </div>

                    <div className="space-y-3 max-h-80 overflow-y-auto pr-1">
                        {timelineLoading ? (
                            <div className="p-3 text-sm text-gray-500 bg-gray-50 dark:bg-gray-900 rounded-md">
                                Loading timeline...
                            </div>
                        ) : timelineError ? (
                            <div className="p-3 text-sm text-red-600 bg-red-50 dark:bg-red-900/20 rounded-md">
                                {timelineError}
                            </div>
                        ) : timeline.length === 0 ? (
                            <div className="p-3 text-sm text-gray-500 bg-gray-50 dark:bg-gray-900 rounded-md">
                                No timeline events available yet.
                            </div>
                        ) : (
                            timeline.map((entry, index) => (
                                <div
                                    key={`${entry.timestamp}-${entry.event_type}-${entry.task_id ?? 'graph'}-${index}`}
                                    className={`rounded-lg border p-3 space-y-2 ${getTimelineEntryTone(entry)}`}
                                >
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="flex flex-wrap items-center gap-2">
                                                <span className="text-xs font-semibold uppercase tracking-wide text-gray-700 dark:text-gray-200">
                                                    {entry.message_type}
                                                </span>
                                                {entry.status && (
                                                    <span className="px-2 py-0.5 rounded-full text-[11px] font-medium bg-white/80 text-gray-700 dark:bg-gray-800 dark:text-gray-200">
                                                        {entry.status}
                                                    </span>
                                                )}
                                                {entry.task_id && timelineScope === 'graph' && (
                                                    <span className="px-2 py-0.5 rounded-full text-[11px] font-medium bg-white/80 text-gray-500 dark:bg-gray-800 dark:text-gray-300">
                                                        {existingNodes.find((candidate) => candidate.id === entry.task_id)?.agent_role || entry.task_id.slice(0, 8)}
                                                    </span>
                                                )}
                                            </div>
                                            <div className="text-[11px] text-gray-500 dark:text-gray-400 mt-1">
                                                {entry.from || 'Runtime'} to {entry.to || 'System'}
                                            </div>
                                        </div>
                                        <div className="text-[11px] text-gray-500 dark:text-gray-400 whitespace-nowrap">
                                            {formatTimelineTimestamp(entry.timestamp)}
                                        </div>
                                    </div>
                                    <div className="text-sm whitespace-pre-wrap text-gray-700 dark:text-gray-200">
                                        {entry.content}
                                    </div>
                                    {entry.timestamp > 0 && (
                                        <div className="flex justify-end">
                                            <button
                                                onClick={() => loadReplay(entry.timestamp)}
                                                className="inline-flex items-center gap-1 text-xs text-blue-600 hover:text-blue-700 dark:text-blue-400"
                                            >
                                                <History size={12} /> Replay State Here
                                            </button>
                                        </div>
                                    )}
                                </div>
                            ))
                        )}
                    </div>
                </div>

                <div>
                    <div className="flex items-center justify-between mb-2 gap-3">
                        <div>
                            <h3 className="font-medium text-gray-700 dark:text-gray-300">Runtime Replay</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                Reconstructs runtime state from persisted swarm events at a selected timeline point.
                            </p>
                        </div>
                        {replaySnapshot && (
                            <div className="text-[11px] text-gray-500 dark:text-gray-400 text-right">
                                <div>{formatTimelineTimestamp(replaySnapshot.at)}</div>
                                <div>{replaySnapshot.source} · {replaySnapshot.timeline_len} events</div>
                            </div>
                        )}
                    </div>

                    {replayLoading ? (
                        <div className="p-3 text-sm text-gray-500 bg-gray-50 dark:bg-gray-900 rounded-md">
                            Rebuilding runtime snapshot...
                        </div>
                    ) : replayError ? (
                        <div className="p-3 text-sm text-red-600 bg-red-50 dark:bg-red-900/20 rounded-md">
                            {replayError}
                        </div>
                    ) : replaySnapshot && replayNode ? (
                        <div className="rounded-lg border border-gray-200 dark:border-gray-700 p-4 space-y-3 bg-white/80 dark:bg-gray-900/40">
                            <div className="flex flex-wrap items-center gap-2">
                                <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-blue-50 text-blue-700 dark:bg-blue-900/20 dark:text-blue-300">
                                    Graph: {replaySnapshot.status}
                                </span>
                                <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-200">
                                    Node: {replayStatusKey}
                                </span>
                                <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-300">
                                    Goal: {replaySnapshot.goal || 'N/A'}
                                </span>
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                                <div>
                                    <div className="text-xs uppercase tracking-wide text-gray-500 mb-1">Prompt</div>
                                    <div className="p-3 rounded-md bg-gray-50 dark:bg-gray-900 whitespace-pre-wrap">
                                        {replayNode.prompt}
                                    </div>
                                </div>
                                <div>
                                    <div className="text-xs uppercase tracking-wide text-gray-500 mb-1">Result</div>
                                    <div className="p-3 rounded-md bg-gray-50 dark:bg-gray-900 whitespace-pre-wrap min-h-[84px]">
                                        {replayNode.result || 'No result at this point'}
                                    </div>
                                </div>
                            </div>
                            <div>
                                <div className="text-xs uppercase tracking-wide text-gray-500 mb-1">Replay Logs</div>
                                <div className="p-3 rounded-md bg-gray-950 text-gray-300 text-xs font-mono max-h-40 overflow-y-auto">
                                    {replayNode.logs && replayNode.logs.length > 0 ? (
                                        replayNode.logs.map((log, index) => (
                                            <div key={`${index}-${log.slice(0, 16)}`} className="mb-1 border-b border-gray-800 pb-1 last:border-0">
                                                {log}
                                            </div>
                                        ))
                                    ) : (
                                        <span className="text-gray-500 italic">No logs captured yet at this replay point</span>
                                    )}
                                </div>
                            </div>
                        </div>
                    ) : (
                        <div className="p-3 text-sm text-gray-500 bg-gray-50 dark:bg-gray-900 rounded-md">
                            Pick “Replay State Here” on a timeline event to reconstruct the runtime graph at that point.
                        </div>
                    )}
                </div>
            </div>
        </Modal>
    );
};
