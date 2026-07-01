import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import {
  ReactFlow,
  Controls,
  Background,
  MiniMap,
  useNodesState,
  useEdgesState,
  addEdge,
  Connection,
  Edge,
  Node,
  useReactFlow,
  ReactFlowProvider,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { GitBranch } from 'lucide-react';
import { NodeTypePanel } from './NodeTypePanel';
import { NodeConfigPanel } from './NodeConfigPanel';
import { ExecutionPanel } from './ExecutionPanel';
import { TemplatePanel } from './TemplatePanel';
import { nodeTypes } from './nodeTypes';
import { workflowTemplates, type WorkflowTemplate, resolveTemplatePlaceholders } from '@/utils/workflowTemplates';
import { workflowApi } from '../../services/workflowApi';
import type {
  WorkflowNode,
  WorkflowEdge,
  Workflow,
  NodeTypeDefinition,
} from '../../types/workflow';
import { downloadWorkflow, readWorkflowFromFile, type Workflow as ChatWorkflow } from '@/utils/chatToCanvas';
import { autoLayout, applyHierarchicalLayout, applyTreeLayout } from '@/utils/canvasLayout';
import toast from 'react-hot-toast';
import { defaultNodeTypes, getDefaultConfig } from './canvasConstants';
import { CanvasToolbar } from './CanvasToolbar';

const CanvasContent: React.FC = () => {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [selectedNode, setSelectedNode] = useState<Node | null>(null);
  const [showNodePanel, setShowNodePanel] = useState(true);
  const [showConfigPanel, setShowConfigPanel] = useState(false);
  const [showExecutionPanel, setShowExecutionPanel] = useState(false);
  const [showTemplatePanel, setShowTemplatePanel] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [currentWorkflow, setCurrentWorkflow] = useState<Workflow | null>(null);
  const [workflowName, setWorkflowName] = useState('New Workflow');
  const [nodeTypesList, setNodeTypesList] = useState<NodeTypeDefinition[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [layoutMode, setLayoutMode] = useState<'auto' | 'hierarchical' | 'tree'>('auto');
  const { fitView, screenToFlowPosition } = useReactFlow();
  const reactFlowWrapper = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadNodeTypes();
  }, []);

  useEffect(() => {
    const pendingWorkflowJson = localStorage.getItem('pendingWorkflow');
    if (pendingWorkflowJson) {
      try {
        const workflow = JSON.parse(pendingWorkflowJson) as ChatWorkflow;
        loadWorkflow(workflow as unknown as Workflow);
        localStorage.removeItem('pendingWorkflow');
        toast.success('Workflow loaded from Chat');
      } catch (error) {
        console.error('Failed to load pending workflow:', error);
      }
    }
  }, []);

  const loadNodeTypes = async () => {
    try {
      const types = await workflowApi.getNodeTypes();
      if (types && types.length > 0) {
        setNodeTypesList(types);
      } else {
        setNodeTypesList(defaultNodeTypes);
      }
    } catch (error) {
      console.error('Failed to load node types:', error);
      setNodeTypesList(defaultNodeTypes);
    }
  };

  // --- Workflow I/O helpers ---

  const convertToWorkflowNodes = useCallback((nodesList: Node[]): WorkflowNode[] => {
    return nodesList.map((node) => ({
      id: node.id,
      node_type: node.type || 'default',
      position: node.position,
      data: {
        label: String(node.data.label || ''),
        description: String(node.data.description || ''),
        config: (node.data.config as Record<string, unknown>) || {},
        inputs: (node.data.inputs as any[]) || [],
        outputs: (node.data.outputs as any[]) || [],
      },
    }));
  }, []);

  const convertToWorkflowEdges = useCallback((edgesList: Edge[]): WorkflowEdge[] => {
    return edgesList.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      source_handle: edge.sourceHandle || undefined,
      target_handle: edge.targetHandle || undefined,
      label: typeof edge.label === 'string' ? edge.label : undefined,
      condition: (edge.data as any)?.condition as string | undefined,
    }));
  }, []);

  const handleExportWorkflow = () => {
    if (nodes.length === 0) {
      toast.error('No workflow to export');
      return;
    }
    const workflow: ChatWorkflow = {
      id: currentWorkflow?.id || `workflow-${Date.now()}`,
      name: workflowName,
      description: currentWorkflow?.description || 'Exported workflow',
      nodes: convertToWorkflowNodes(nodes),
      edges: convertToWorkflowEdges(edges),
      variables: {},
      created_at: currentWorkflow?.created_at || new Date().toISOString(),
      updated_at: new Date().toISOString(),
      version: 1,
      is_active: true,
    };
    downloadWorkflow(workflow);
    toast.success('Workflow exported successfully');
  };

  const handleImportWorkflow = async (files: FileList | null) => {
    if (!files?.length) return;
    const file = files[0];
    if (!file.name.endsWith('.json')) {
      toast.error('Please select a JSON file');
      return;
    }
    try {
      const workflow = await readWorkflowFromFile(file);
      loadWorkflow(workflow as unknown as Workflow);
      toast.success(`Workflow "${workflow.name}" imported successfully`);
    } catch (error) {
      toast.error(`Failed to import: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleSelectTemplate = (template: WorkflowTemplate) => {
    const resolvedWorkflow = resolveTemplatePlaceholders(template.workflow);
    loadWorkflow(resolvedWorkflow as unknown as Workflow);
    setWorkflowName(resolvedWorkflow.name);
    setShowTemplatePanel(false);
    toast.success(`Loaded template: ${template.name}`);
    setTimeout(() => fitView(), 100);
  };

  const loadWorkflow = (workflow: Workflow | ChatWorkflow) => {
    const flowNodes: Node[] = workflow.nodes.map((n) => ({
      id: n.id,
      type: n.node_type,
      position: n.position,
      data: {
        label: n.data.label,
        description: n.data.description,
        ...n.data.config,
      },
    }));
    const flowEdges: Edge[] = workflow.edges.map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
      sourceHandle: e.source_handle,
      targetHandle: e.target_handle,
      label: e.label,
      type: 'smoothstep',
    }));
    setNodes(flowNodes);
    setEdges(flowEdges);
    setWorkflowName(workflow.name);
    setCurrentWorkflow(workflow as Workflow);
    setTimeout(() => fitView(), 100);
  };

  // --- ReactFlow event handlers ---

  const onConnect = useCallback((connection: Connection) => {
    const edge: Edge = {
      ...connection,
      id: `edge-${Date.now()}`,
      type: 'smoothstep',
      animated: true,
      style: { stroke: '#6366f1', strokeWidth: 2 },
    } as Edge;
    setEdges((eds) => addEdge(edge, eds));
  }, [setEdges]);

  const onDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  const onDrop = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    const type = event.dataTransfer.getData('application/reactflow');
    if (!type) return;
    const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
    handleAddNode(type, position);
  }, [screenToFlowPosition, nodeTypesList]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    setSelectedNode(node);
    setShowConfigPanel(true);
  }, []);

  const onPaneClick = useCallback(() => {
    setSelectedNode(null);
    setShowConfigPanel(false);
  }, []);

  // --- Node operations ---

  const handleAddNode = (type: string, position?: { x: number; y: number }) => {
    const nodeType = nodeTypesList.find((t) => t.type === type);
    if (!nodeType) return;
    const newNode: Node = {
      id: `${type}-${Date.now()}`,
      type,
      position: position || { x: Math.random() * 400 + 100, y: Math.random() * 300 + 100 },
      data: {
        label: nodeType.name,
        description: nodeType.description,
        config: getDefaultConfig(type),
        inputs: nodeType.inputs || [],
        outputs: nodeType.outputs || [],
      },
    };
    setNodes((nds) => [...nds, newNode]);
  };

  const handleUpdateNode = (nodeId: string, data: Record<string, unknown>) => {
    setNodes((nds) =>
      nds.map((node) =>
        node.id === nodeId ? ({ ...node, data: { ...node.data, ...data } } as Node) : node
      )
    );
  };

  const handleDeleteNode = (nodeId: string) => {
    setNodes((nds) => nds.filter((n) => n.id !== nodeId));
    setEdges((eds) => eds.filter((e) => e.source !== nodeId && e.target !== nodeId));
    if (selectedNode?.id === nodeId) {
      setSelectedNode(null);
      setShowConfigPanel(false);
    }
  };

  // --- Layout ---

  const handleLayout = (mode: 'auto' | 'hierarchical' | 'tree' = layoutMode) => {
    if (nodes.length === 0) return;
    let newNodes: Node[];
    switch (mode) {
      case 'hierarchical':
        newNodes = applyHierarchicalLayout(nodes, edges);
        break;
      case 'tree':
        newNodes = applyTreeLayout(nodes, edges);
        break;
      default:
        newNodes = autoLayout(nodes, edges);
    }
    setNodes(newNodes);
    setLayoutMode(mode);
    setTimeout(() => fitView({ padding: 0.2 }), 100);
    toast.success(`Applied ${mode} layout`);
  };

  // --- Search ---

  const filteredNodes = useMemo(() => {
    if (!searchQuery) return nodes;
    const query = searchQuery.toLowerCase();
    return nodes.filter(node =>
      String(node.data.label).toLowerCase().includes(query) ||
      String(node.data.description).toLowerCase().includes(query) ||
      node.type?.toLowerCase().includes(query)
    );
  }, [nodes, searchQuery]);

  const highlightedNodes = useMemo(() => {
    if (!searchQuery) return nodes;
    return nodes.map(node => {
      const isMatch = filteredNodes.some((n: Node) => n.id === node.id);
      return { ...node, data: { ...node.data, _isSearchMatch: isMatch, _isDimmed: !isMatch } };
    });
  }, [nodes, filteredNodes, searchQuery]);

  // --- Misc ---

  const handleClear = () => {
    if (confirm('Are you sure you want to clear all nodes?')) {
      setNodes([]);
      setEdges([]);
      setSelectedNode(null);
      setShowConfigPanel(false);
    }
  };

  const handleSave = async () => {
    if (nodes.length === 0) {
      alert('Please add some nodes first');
      return;
    }
    setIsSaving(true);
    try {
      const workflowNodes = convertToWorkflowNodes(nodes);
      const workflowEdges = convertToWorkflowEdges(edges);
      if (currentWorkflow) {
        const updated = await workflowApi.updateWorkflow(currentWorkflow.id, {
          name: workflowName,
          nodes: workflowNodes,
          edges: workflowEdges,
        });
        setCurrentWorkflow(updated);
        alert('Workflow saved successfully!');
      } else {
        const created = await workflowApi.createWorkflow({
          name: workflowName,
          description: '',
          nodes: workflowNodes,
          edges: workflowEdges,
        });
        setCurrentWorkflow(created);
        alert('Workflow created successfully!');
      }
    } catch (error) {
      console.error('Failed to save workflow:', error);
      alert('Failed to save workflow. Please try again.');
    } finally {
      setIsSaving(false);
    }
  };

  const updateNodeStatus = (nodeId: string, status: string, error?: string) => {
    setNodes((nds) =>
      nds.map((node) =>
        node.id === nodeId
          ? ({ ...node, data: { ...node.data, status, ...(error && { error }) } } as Node)
          : node
      )
    );
  };

  const handleBatchUpdateModel = useCallback((modelId: string, modelProvider?: string, modelVendor?: string) => {
    setNodes((nds) =>
      nds.map((node) => {
        if (node.type === 'llm' || node.type === 'agent') {
          return {
            ...node,
            data: {
              ...node.data,
              config: {
                ...((node.data.config as Record<string, unknown>) || {}),
                model: modelId,
                ...(modelProvider && { modelProvider }),
                ...(modelVendor && { modelVendor }),
              },
            },
          };
        }
        return node;
      })
    );
    toast.success(`Updated model for all ${nodes.filter(n => n.type === 'llm' || n.type === 'agent').length} AI nodes`);
  }, [nodes]);

  const aiNodesCount = useMemo(() => nodes.filter(n => n.type === 'llm' || n.type === 'agent').length, [nodes]);

  return (
    <div className="h-full w-full bg-gray-50 dark:bg-gray-900 flex">
      {/* Left Panel - Node Types */}
      {showNodePanel && (
        <div className="w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col">
          <div className="p-4 border-b border-gray-200 dark:border-gray-700">
            <h2 className="font-semibold text-gray-900 dark:text-white flex items-center gap-2">
              <GitBranch className="w-5 h-5" />
              Nodes
            </h2>
          </div>
          <NodeTypePanel nodeTypes={nodeTypesList} onAddNode={handleAddNode} />
        </div>
      )}

      {/* Main Canvas Area */}
      <div className="flex-1 flex flex-col">
        <input
          ref={fileInputRef}
          type="file"
          accept=".json"
          className="hidden"
          onChange={(e) => handleImportWorkflow(e.target.files)}
        />
        <CanvasToolbar
          workflowName={workflowName}
          setWorkflowName={setWorkflowName}
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          showNodePanel={showNodePanel}
          onToggleNodePanel={() => setShowNodePanel(!showNodePanel)}
          onShowTemplatePanel={() => setShowTemplatePanel(true)}
          onImportClick={() => fileInputRef.current?.click()}
          onExport={handleExportWorkflow}
          hasNodes={nodes.length > 0}
          aiNodesCount={aiNodesCount}
          onBatchUpdateModel={handleBatchUpdateModel}
          layoutMode={layoutMode}
          onLayout={handleLayout}
          onClear={handleClear}
          showExecutionPanel={showExecutionPanel}
          onToggleExecutionPanel={() => setShowExecutionPanel(!showExecutionPanel)}
          isSaving={isSaving}
          onSave={handleSave}
        />

        {/* React Flow Canvas */}
        <div className="flex-1 relative" ref={reactFlowWrapper}>
          <ReactFlow
            nodes={searchQuery ? highlightedNodes : nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeClick={onNodeClick}
            onPaneClick={onPaneClick}
            onDragOver={onDragOver}
            onDrop={onDrop}
            nodeTypes={nodeTypes}
            fitView
            attributionPosition="bottom-right"
            className="bg-gray-50 dark:bg-gray-900"
            deleteKeyCode={['Backspace', 'Delete']}
          >
            <Background />
            <Controls />
            <MiniMap
              nodeStrokeWidth={3}
              zoomable
              pannable
              className="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700"
            />
          </ReactFlow>

          {nodes.length === 0 && (
            <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
              <div className="text-center text-gray-400 dark:text-gray-600">
                <GitBranch className="w-16 h-16 mx-auto mb-4 opacity-50" />
                <p className="text-lg font-semibold">Canvas Empty</p>
                <p className="text-sm">Drag nodes from the left panel to get started</p>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Right Panel - Template, Node Config or Execution */}
      {showTemplatePanel && (
        <div className="w-96 bg-white dark:bg-gray-800 border-l border-gray-200 dark:border-gray-700 flex flex-col">
          <TemplatePanel
            onSelectTemplate={handleSelectTemplate}
            onClose={() => setShowTemplatePanel(false)}
          />
        </div>
      )}

      {showConfigPanel && selectedNode && !showExecutionPanel && (
        <div className="w-96 bg-white dark:bg-gray-800 border-l border-gray-200 dark:border-gray-700 flex flex-col">
          <NodeConfigPanel
            node={selectedNode}
            onUpdate={(data) => handleUpdateNode(selectedNode.id, data)}
            onDelete={() => handleDeleteNode(selectedNode.id)}
            onClose={() => {
              setShowConfigPanel(false);
              setSelectedNode(null);
            }}
          />
        </div>
      )}

      {showExecutionPanel && (
        <div className="w-96 bg-white dark:bg-gray-800 border-l border-gray-200 dark:border-gray-700 flex flex-col">
          <ExecutionPanel
            nodes={nodes}
            edges={edges}
            workflowId={currentWorkflow?.id}
            workflowName={workflowName}
            onClose={() => setShowExecutionPanel(false)}
            onNodeStatusUpdate={updateNodeStatus}
          />
        </div>
      )}
    </div>
  );
};

export const Canvas: React.FC = () => {
  return (
    <ReactFlowProvider>
      <CanvasContent />
    </ReactFlowProvider>
  );
};
