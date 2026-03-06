import React, { useEffect, useState, useRef } from 'react';
import type { HitlReview, TaskGraph, TaskNode } from '@/types/domain';
import { Card } from '../ui/Card';
import { Skeleton } from '../ui/Skeleton';
import { LayoutTemplate } from 'lucide-react';
import toast from 'react-hot-toast';
import { SwarmStats } from './SwarmStats';
import { SwarmActivityFeed } from './SwarmActivityFeed';
import type { SwarmMessage } from './SwarmActivityFeed';
import { AddTaskModal } from './AddTaskModal';
import { TemplatesModal, SaveTemplateModal } from './TemplatesModal';
import { NodeDetailsModal } from './graph/NodeDetailsModal';
import { GraphFilters } from './graph/GraphFilters';
import { BatchActions } from './graph/BatchActions';
import { GraphList } from './graph/GraphList';
import { Pagination } from './graph/Pagination';
import { dashboardService } from '@/services/dashboardService';
import { HitlReviewPanel } from './HitlReviewPanel';

export const SwarmGraph: React.FC = () => {
  const [graphs, setGraphs] = useState<TaskGraph[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<{ graphId: string, node: TaskNode, graphStatus: string } | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [statusFilter, setStatusFilter] = useState('Active');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedGraphIds, setSelectedGraphIds] = useState<Set<string>>(new Set());
  const [messages, setMessages] = useState<SwarmMessage[]>([]);
  const [addModalGraphId, setAddModalGraphId] = useState<string | null>(null);
  const [showTemplates, setShowTemplates] = useState(false);
  const [saveTemplateGraphId, setSaveTemplateGraphId] = useState<string | null>(null);
  const [reviews, setReviews] = useState<HitlReview[]>([]);
  const [reviewsLoading, setReviewsLoading] = useState(false);
  
  // WebSocket setup
  const wsRef = useRef<WebSocket | null>(null);
  const getApiErrorMessage = (fallback: string, err: any) =>
    err?.response?.status === 404 ? '当前后端版本暂不支持该操作。' : fallback;

  const fetchGraphs = async () => {
    try {
      const data = await dashboardService.getSwarmGraphs(page, 5, statusFilter, searchQuery);
      setGraphs(data.graphs || []);
      if (data.pagination) {
          setTotalPages(data.pagination.total_pages);
      }
      
      // Update selected node if it's open
      if (selectedNode) {
          const graph = data.graphs.find(g => g.id === selectedNode.graphId);
          if (graph) {
              const node = graph.nodes[selectedNode.node.id];
              if (node) {
                  setSelectedNode({ graphId: graph.id!, node, graphStatus: graph.status || 'Active' });
              }
          }
      }
    } catch (err) {
      setError('Failed to load swarm graphs');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const fetchReviews = async () => {
    try {
      setReviewsLoading(true);
      const data = await dashboardService.listSwarmReviews();
      setReviews(data);
    } catch (err) {
      console.error(err);
    } finally {
      setReviewsLoading(false);
    }
  };

  useEffect(() => {
    fetchGraphs();
    fetchReviews();
    // Poll less frequently if WS is connected, or keep polling as backup
    const interval = setInterval(fetchGraphs, 5000);
    const reviewInterval = setInterval(fetchReviews, 5000);
    return () => {
      clearInterval(interval);
      clearInterval(reviewInterval);
    };
  }, [page, statusFilter, searchQuery]);

  // WebSocket Connection
  useEffect(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    
    const connectWs = () => {
        const ws = new WebSocket(wsUrl);
        ws.onopen = () => console.log('Swarm WS Connected');
        ws.onmessage = (event) => {
            const msg = event.data;
            if (typeof msg === 'string') {
                if (msg.startsWith('SWARM_GRAPH:')) {
                    // SWARM_GRAPH: id|status
                    const parts = msg.substring(12).split('|');
                    if (parts.length >= 2) {
                        const [id, status] = parts;
                        setGraphs(prev => prev.map(g => g.id === id ? { ...g, status: status as any } : g));
                    }
                } else if (msg.startsWith('SWARM_TASK:')) {
                    // SWARM_TASK: graph_id|task_id|status
                    const parts = msg.substring(11).split('|');
                    if (parts.length >= 3) {
                        const [graphId, taskId, status] = parts;
                        setGraphs(prev => prev.map(g => {
                            if (g.id === graphId && g.nodes[taskId]) {
                                return {
                                    ...g,
                                    nodes: {
                                        ...g.nodes,
                                        [taskId]: {
                                            ...g.nodes[taskId],
                                            status: status === 'Pending' ? 'Pending' : { [status]: {} } as any // Simplified status parsing
                                        }
                                    }
                                };
                            }
                            return g;
                        }));
                        // Trigger fetch to get full details (result, logs, etc) as WS payload is minimal
                        // Debounce this in real app
                        fetchGraphs();
                    }
                } else if (msg.startsWith('SWARM_LOG:')) {
                    const parts = msg.substring(11).split('|');
                    if (parts.length >= 3) {
                        const graphId = parts[0];
                        const taskId = parts[1];
                        const b64Content = parts.slice(2).join('|'); // Join back in case content had |
                        
                        try {
                            const content = atob(b64Content);
                            setGraphs(prev => prev.map(g => {
                                if (g.id === graphId && g.nodes[taskId]) {
                                    const node = g.nodes[taskId];
                                    const currentLogs = (node as any).logs || [];
                                    
                                    // Avoid duplicate logs if possible or just append
                                    // For simple streaming, just append.
                                    return {
                                        ...g,
                                        nodes: {
                                            ...g.nodes,
                                            [taskId]: {
                                                ...node,
                                                logs: [...currentLogs, content]
                                            }
                                        }
                                    };
                                }
                                return g;
                            }));
                        } catch (e) {
                            console.error('Failed to decode log content', e);
                        }
                    }
                } else if (msg.startsWith('SWARM_MSG:')) {
                    // SWARM_MSG: graph_id|task_id|from|to|type|b64_content
                    const parts = msg.substring(11).split('|');
                    if (parts.length >= 6) {
                        const [graphId, taskId, from, to, type, b64Content] = parts;
                         try {
                            const content = atob(b64Content);
                            const newMessage: SwarmMessage = {
                                graphId,
                                taskId,
                                from,
                                to,
                                type,
                                content,
                                timestamp: Date.now()
                            };
                            setMessages(prev => [...prev, newMessage]);
                            if (type === 'HITLReviewRequested' || type === 'HITLDecision') {
                                fetchReviews();
                            }
                        } catch (e) {
                            console.error('Failed to decode msg content', e);
                        }
                    }
                }
            }
        };
        ws.onclose = () => {
            console.log('Swarm WS Disconnected. Reconnecting...');
            setTimeout(connectWs, 3000);
        };
        wsRef.current = ws;
    };

    connectWs();
    return () => {
        if (wsRef.current) wsRef.current.close();
    };
  }, []);

  const handlePause = async (id: string) => {
    try {
        await dashboardService.pauseSwarmTask(id);
        toast.success('Task paused');
        fetchGraphs();
    } catch (e) {
        toast.error(getApiErrorMessage('Failed to pause task', e));
    }
  };

  const handleResume = async (id: string) => {
    try {
        await dashboardService.resumeSwarmTask(id);
        toast.success('Task resumed');
        fetchGraphs();
    } catch (e) {
        toast.error(getApiErrorMessage('Failed to resume task', e));
    }
  };
  
  const handleUpdatePrompt = async (newPrompt: string, dependencies?: string[]) => {
      if (!selectedNode) return;
      try {
          await dashboardService.updateTaskPrompt(selectedNode.graphId, selectedNode.node.id, newPrompt, dependencies);
          toast.success('Task updated');
          fetchGraphs();
      } catch (e) {
          toast.error(getApiErrorMessage('Failed to update task', e));
      }
  };

  const handleRetryNode = async () => {
      if (!selectedNode) return;
      try {
          await dashboardService.retryTaskNode(selectedNode.graphId, selectedNode.node.id);
          toast.success('Task retry initiated');
          fetchGraphs();
      } catch (e) {
          toast.error(getApiErrorMessage('Failed to retry task', e));
      }
  };

  const toggleSelectGraph = (id: string) => {
      const newSet = new Set(selectedGraphIds);
      if (newSet.has(id)) newSet.delete(id);
      else newSet.add(id);
      setSelectedGraphIds(newSet);
  };

  const handleBatchAction = async (action: 'pause' | 'resume' | 'delete') => {
      if (selectedGraphIds.size === 0) return;
      if (action === 'delete' && !confirm(`Delete ${selectedGraphIds.size} tasks?`)) return;
      
      try {
          await dashboardService.batchSwarmAction(action, Array.from(selectedGraphIds));
          toast.success(`Batch ${action} successful`);
          setSelectedGraphIds(new Set());
          fetchGraphs();
      } catch (e) {
          toast.error(getApiErrorMessage(`Batch ${action} failed`, e));
      }
  };

  const handleAddTask = async (role: string, prompt: string, dependencies: string[]) => {
      if (!addModalGraphId) return;
      try {
          await dashboardService.addTaskToGraph(addModalGraphId, role, prompt, dependencies);
          toast.success('Task added successfully');
          setAddModalGraphId(null);
          fetchGraphs();
      } catch (e) {
          toast.error(getApiErrorMessage('Failed to add task', e));
      }
  };

  const submitReviewDecision = async (
    taskId: string,
    payload: { decision: 'approved' | 'rejected' | 'edited' | 'selected' | 'feedback'; value?: string; selected_index?: number }
  ) => {
    await dashboardService.decideSwarmReview(taskId, payload);
    await fetchReviews();
    toast.success('审批已提交');
  };

  if (loading && graphs.length === 0) {
    return <Skeleton className="h-64 w-full" />;
  }

  if (error && graphs.length === 0) {
    return <div className="p-4 text-red-500 bg-red-50 rounded-lg">{error}</div>;
  }

  return (
    <div className="space-y-6">
      <SwarmStats />
      
      <div className="flex flex-col gap-4 sm:flex-row sm:justify-between sm:items-center">
          <div className="flex items-center gap-2">
              <h2 className="text-xl font-bold text-gray-800 dark:text-gray-100">Swarm Task Graphs</h2>
              <button 
                  onClick={() => setShowTemplates(true)}
                  className="p-1.5 text-gray-500 hover:text-blue-600 rounded hover:bg-blue-50 dark:hover:bg-blue-900/20"
                  title="Templates"
              >
                  <LayoutTemplate size={20} />
              </button>
          </div>
          
          <GraphFilters 
            statusFilter={statusFilter}
            setStatusFilter={setStatusFilter}
            searchQuery={searchQuery}
            setSearchQuery={setSearchQuery}
            onPageChange={setPage}
          />
      </div>
      
      {selectedGraphIds.size > 0 && (
          <BatchActions 
            selectedCount={selectedGraphIds.size}
            onAction={handleBatchAction}
            onClear={() => setSelectedGraphIds(new Set())}
          />
      )}

      {graphs.length === 0 ? (
          <Card className="p-6 text-center text-gray-500">
            <p>No tasks found.</p>
          </Card>
      ) : (
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
              <GraphList 
                graphs={graphs}
                selectedGraphIds={selectedGraphIds}
                toggleSelectGraph={toggleSelectGraph}
                onPause={handlePause}
                onResume={handleResume}
                onAdd={(id) => setAddModalGraphId(id)}
                onTemplate={(id) => setSaveTemplateGraphId(id)}
                onNodeClick={(graph, node) => setSelectedNode({ graphId: graph.id!, node, graphStatus: graph.status || 'Active' })}
              />
              <div className="lg:col-span-1">
                  <div className="sticky top-4 space-y-4">
                    <HitlReviewPanel
                      reviews={reviews}
                      loading={reviewsLoading}
                      onRefresh={fetchReviews}
                      onApprove={(taskId) => submitReviewDecision(taskId, { decision: 'approved' })}
                      onReject={(taskId, reason) => submitReviewDecision(taskId, { decision: 'rejected', value: reason })}
                      onEdit={(taskId, content) => submitReviewDecision(taskId, { decision: 'edited', value: content })}
                      onFeedback={(taskId, feedback) => submitReviewDecision(taskId, { decision: 'feedback', value: feedback })}
                      onSelect={(taskId, index) => submitReviewDecision(taskId, { decision: 'selected', selected_index: index })}
                    />
                    <SwarmActivityFeed messages={messages} />
                  </div>
              </div>
          </div>
      )}
      
      <Pagination 
        page={page} 
        totalPages={totalPages} 
        setPage={setPage} 
      />
      
      {selectedNode && (
          <NodeDetailsModal 
              isOpen={!!selectedNode} 
              onClose={() => setSelectedNode(null)} 
              node={selectedNode.node}
              graphStatus={selectedNode.graphStatus}
              existingNodes={selectedNode.graphId ? Object.values(graphs.find(g => g.id === selectedNode.graphId)?.nodes || {}) : []}
              onUpdatePrompt={handleUpdatePrompt}
              onRetry={handleRetryNode}
          />
      )}

      {addModalGraphId && (
          <AddTaskModal 
              isOpen={!!addModalGraphId}
              onClose={() => setAddModalGraphId(null)}
              onSubmit={handleAddTask}
              existingNodes={Object.values(graphs.find(g => g.id === addModalGraphId)?.nodes || {})}
          />
      )}

      <TemplatesModal 
          isOpen={showTemplates} 
          onClose={() => setShowTemplates(false)} 
      />
      
      {saveTemplateGraphId && (
          <SaveTemplateModal 
              isOpen={!!saveTemplateGraphId} 
              onClose={() => setSaveTemplateGraphId(null)} 
              graphId={saveTemplateGraphId} 
          />
      )}
    </div>
  );
};
