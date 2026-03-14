import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { ModelSelectorCompact } from './ModelSelectorCompact';
import {
  ReactFlow,
  Controls,
  Background,
  MiniMap,
  Panel,
  useNodesState,
  useEdgesState,
  addEdge,
  Connection,
  Edge,
  Node,
  useReactFlow,
  ReactFlowProvider,
  type ReactFlowInstance,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import {
  Play,
  Square,
  Brain,
  Bot,
  Code,
  Globe,
  GitBranch,
  Repeat,
  Database,
  FileText,
  Book,
  Save,
  Loader2,
  LayoutTemplate,
  Trash2,
  Settings,
  ChevronRight,
  ChevronLeft,
  Terminal,
  CheckCircle,
  XCircle,
  Clock,
  Download,
  Upload,
  FolderOpen,
  Search,
  Grid3X3,
  AlignVerticalJustifyCenter,
  Network,
} from 'lucide-react';
import { Button } from '../ui/Button';
import { NodeTypePanel } from './NodeTypePanel';
import { NodeConfigPanel } from './NodeConfigPanel';
import { ExecutionPanel } from './ExecutionPanel';
import { TemplatePanel } from './TemplatePanel';
import { nodeTypes } from './nodeTypes';
import { workflowTemplates, type WorkflowTemplate, resolveTemplatePlaceholders } from '@/utils/workflowTemplates';
import { workflowApi, executionApi } from '../../services/workflowApi';
import type {
  WorkflowNode,
  WorkflowEdge,
  Workflow,
  NodeTypeDefinition,
} from '../../types/workflow';
import { downloadWorkflow, readWorkflowFromFile, type Workflow as ChatWorkflow } from '@/utils/chatToCanvas';
import { autoLayout, applyHierarchicalLayout, applyTreeLayout } from '@/utils/canvasLayout';
import { useModelStore } from '@/store/modelStore';
import toast from 'react-hot-toast';

// Canvas component with workflow functionality
const CanvasContent: React.FC = () => {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [selectedNode, setSelectedNode] = useState<Node | null>(null);
  const [showNodePanel, setShowNodePanel] = useState(true);
  const [showConfigPanel, setShowConfigPanel] = useState(false);
  const [showExecutionPanel, setShowExecutionPanel] = useState(false);
  const [showTemplatePanel, setShowTemplatePanel] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const [currentWorkflow, setCurrentWorkflow] = useState<Workflow | null>(null);
  const [workflowName, setWorkflowName] = useState('New Workflow');
  const [nodeTypesList, setNodeTypesList] = useState<NodeTypeDefinition[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [layoutMode, setLayoutMode] = useState<'auto' | 'hierarchical' | 'tree'>('auto');
  const { fitView, screenToFlowPosition } = useReactFlow();
  const reactFlowWrapper = useRef<HTMLDivElement>(null);

  // Default node types as fallback
  const defaultNodeTypes: NodeTypeDefinition[] = [
    { type: 'start', name: 'Start', description: 'Workflow start node', category: 'control', icon: 'Play', color: '#10b981' },
    { type: 'end', name: 'End', description: 'Workflow end node', category: 'control', icon: 'Square', color: '#ef4444' },
    { type: 'condition', name: 'Condition', description: 'Branch by condition', category: 'control', icon: 'GitBranch', color: '#ec4899' },
    { type: 'loop', name: 'Loop', description: 'Iterate over collection', category: 'control', icon: 'Repeat', color: '#84cc16' },
    { type: 'llm', name: 'LLM', description: 'Call an LLM model', category: 'ai', icon: 'Brain', color: '#8b5cf6' },
    { type: 'agent', name: 'Agent', description: 'AI agent execution', category: 'ai', icon: 'Bot', color: '#3b82f6' },
    { type: 'knowledge', name: 'Knowledge', description: 'Query knowledge base', category: 'ai', icon: 'Book', color: '#a855f7' },
    { type: 'code', name: 'Code', description: 'Execute code', category: 'processing', icon: 'Code', color: '#f59e0b' },
    { type: 'template', name: 'Template', description: 'Text template rendering', category: 'processing', icon: 'FileText', color: '#14b8a6' },
    { type: 'http', name: 'HTTP Request', description: 'Call external API', category: 'integration', icon: 'Globe', color: '#06b6d4' },
    { type: 'variable', name: 'Variable', description: 'Variable operations', category: 'data', icon: 'Database', color: '#6366f1' },
  ];

  const fileInputRef = useRef<HTMLInputElement>(null);

  // Load node types
  useEffect(() => {
    loadNodeTypes();
  }, []);

  // Check for pending workflow from Chat
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
        console.warn('No node types returned from API, using defaults');
        setNodeTypesList(defaultNodeTypes);
      }
    } catch (error) {
      console.error('Failed to load node types:', error);
      console.info('Using default node types');
      setNodeTypesList(defaultNodeTypes);
    }
  };

  // Export current workflow as JSON
  const handleExportWorkflow = () => {
    if (nodes.length === 0) {
      toast.error('No workflow to export');
      return;
    }

    // Convert ReactFlow nodes to WorkflowNode format
    const workflowNodes: WorkflowNode[] = nodes.map((node) => ({
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

    // Convert ReactFlow edges to WorkflowEdge format
    const workflowEdges: WorkflowEdge[] = edges.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      source_handle: edge.sourceHandle || undefined,
      target_handle: edge.targetHandle || undefined,
      label: typeof edge.label === 'string' ? edge.label : undefined,
      condition: (edge.data as any)?.condition as string | undefined,
    }));

    const workflow: ChatWorkflow = {
      id: currentWorkflow?.id || `workflow-${Date.now()}`,
      name: workflowName,
      description: currentWorkflow?.description || 'Exported workflow',
      nodes: workflowNodes,
      edges: workflowEdges,
      variables: {},
      created_at: currentWorkflow?.created_at || new Date().toISOString(),
      updated_at: new Date().toISOString(),
      version: 1,
      is_active: true,
    };

    downloadWorkflow(workflow);
    toast.success('Workflow exported successfully');
  };

    // Import workflow from file
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

    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  // Load template into canvas
  const handleSelectTemplate = (template: WorkflowTemplate) => {
    // Resolve placeholders before loading
    const resolvedWorkflow = resolveTemplatePlaceholders(template.workflow);
    loadWorkflow(resolvedWorkflow as unknown as Workflow);
    setWorkflowName(resolvedWorkflow.name);
    setShowTemplatePanel(false);
    toast.success(`Loaded template: ${template.name}`);
    setTimeout(() => fitView(), 100);
  };

  // Load workflow into canvas
  const loadWorkflow = (workflow: Workflow | ChatWorkflow) => {
    // Convert workflow nodes to ReactFlow format
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
    
    // Convert workflow edges to ReactFlow format
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

  // Handle connections
  const onConnect = useCallback(
    (connection: Connection) => {
      const edge: Edge = {
        ...connection,
        id: `edge-${Date.now()}`,
        type: 'smoothstep',
        animated: true,
        style: { stroke: '#6366f1', strokeWidth: 2 },
      } as Edge;
      setEdges((eds) => addEdge(edge, eds));
    },
    [setEdges]
  );

  // Handle drag over
  const onDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  // Handle drop
  const onDrop = useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();

      const type = event.dataTransfer.getData('application/reactflow');

      // Check if the dropped element is a valid node type
      if (typeof type === 'undefined' || !type) {
        return;
      }

      // Get the position where the node was dropped
      const position = screenToFlowPosition({
        x: event.clientX,
        y: event.clientY,
      });

      handleAddNode(type, position);
    },
    [screenToFlowPosition, nodeTypesList]
  );

  // Handle node selection
  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    setSelectedNode(node);
    setShowConfigPanel(true);
  }, []);

  // Handle pane click (deselect)
  const onPaneClick = useCallback(() => {
    setSelectedNode(null);
    setShowConfigPanel(false);
  }, []);

  // Add new node
  const handleAddNode = (type: string, position?: { x: number; y: number }) => {
    const nodeType = nodeTypesList.find((t) => t.type === type);
    if (!nodeType) return;

    const newNode: Node = {
      id: `${type}-${Date.now()}`,
      type,
      position: position || {
        x: Math.random() * 400 + 100,
        y: Math.random() * 300 + 100,
      },
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

  // Get default model from store
  const getDefaultModelConfig = useCallback((): { model: string; modelProvider?: string; modelVendor?: string } => {
    const { providers } = useModelStore.getState();
    const enabledProviders = providers.filter((p) => p.enabled && (!p.modelType || p.modelType === 'chat'));
    const sorted = enabledProviders.sort((a, b) => a.priority - b.priority);
    const defaultProvider = sorted[0];
    
    if (defaultProvider) {
      return {
        model: defaultProvider.model,
        modelProvider: defaultProvider.id,
        modelVendor: defaultProvider.vendor,
      };
    }
    
    // Fallback to legacy defaults if no providers configured
    return { model: 'gpt-4' };
  }, []);

  // Get default config for node type
  const getDefaultConfig = useCallback((type: string): Record<string, unknown> => {
    const defaultModel = getDefaultModelConfig();
    
    switch (type) {
      case 'start':
        return { variables: [] };
      case 'end':
        return { outputs: [], return_value: null };
      case 'llm':
        return {
          ...defaultModel,
          prompt: '',
          system_prompt: '',
          temperature: 0.7,
          max_tokens: 2000,
        };
      case 'agent':
        return {
          role: 'coder',
          task: '',
          context: '',
          tools: [],
          // Model is optional for agent, will use system default if not specified
        };
      case 'code':
        return {
          language: 'python',
          code: '',
          timeout: 30,
        };
      case 'http':
        return {
          method: 'GET',
          url: '',
          headers: {},
          body: '',
        };
      case 'condition':
        return {
          condition: '',
          branches: [
            { label: 'True', condition: 'true' },
            { label: 'False', condition: 'false' },
          ],
        };
      case 'loop':
        return {
          items: '',
          max_iterations: 100,
        };
      case 'variable':
        return {
          operation: 'set',
          name: '',
          value: '',
        };
      case 'template':
        return {
          template: '',
          variables: {},
        };
      case 'knowledge':
        return {
          query: '',
          top_k: 5,
          filter: {},
        };
      default:
        return {};
    }
  }, [getDefaultModelConfig]);

  // Update node data
  const handleUpdateNode = (nodeId: string, data: Record<string, unknown>) => {
    setNodes((nds) =>
      nds.map((node) =>
        node.id === nodeId
          ? ({ ...node, data: { ...node.data, ...data } } as Node)
          : node
      )
    );
  };

  // Delete node
  const handleDeleteNode = (nodeId: string) => {
    setNodes((nds) => nds.filter((n) => n.id !== nodeId));
    setEdges((eds) =>
      eds.filter((e) => e.source !== nodeId && e.target !== nodeId)
    );
    if (selectedNode?.id === nodeId) {
      setSelectedNode(null);
      setShowConfigPanel(false);
    }
  };

  // Auto layout with multiple algorithms
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
      case 'auto':
      default:
        newNodes = autoLayout(nodes, edges);
        break;
    }
    
    setNodes(newNodes);
    setTimeout(() => fitView({ padding: 0.2 }), 100);
    toast.success(`Applied ${mode} layout`);
  };

  // Filter nodes based on search
  const filteredNodes = useMemo(() => {
    if (!searchQuery) return nodes;
    
    const query = searchQuery.toLowerCase();
    return nodes.filter(node => 
      String(node.data.label).toLowerCase().includes(query) ||
      String(node.data.description).toLowerCase().includes(query) ||
      node.type?.toLowerCase().includes(query)
    );
  }, [nodes, searchQuery]);

  // Highlight search matches
  const highlightedNodes = useMemo(() => {
    if (!searchQuery) return nodes;
    
    return nodes.map(node => {
      const isMatch = filteredNodes.some((n: Node) => n.id === node.id);
      return {
        ...node,
        data: {
          ...node.data,
          _isSearchMatch: isMatch,
          _isDimmed: !isMatch,
        },
      };
    });
  }, [nodes, filteredNodes, searchQuery]);

  // Clear canvas
  const handleClear = () => {
    if (confirm('Are you sure you want to clear all nodes?')) {
      setNodes([]);
      setEdges([]);
      setSelectedNode(null);
      setShowConfigPanel(false);
    }
  };

  // Save workflow
  const handleSave = async () => {
    if (nodes.length === 0) {
      alert('Please add some nodes first');
      return;
    }

    setIsSaving(true);
    try {
      const workflowNodes: WorkflowNode[] = nodes.map((node) => ({
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

      const workflowEdges: WorkflowEdge[] = edges.map((edge) => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
        source_handle: edge.sourceHandle || undefined,
        target_handle: edge.targetHandle || undefined,
        label: typeof edge.label === 'string' ? edge.label : undefined,
        condition: (edge.data as any)?.condition as string | undefined,
      }));

      if (currentWorkflow) {
        // Update existing
        const updated = await workflowApi.updateWorkflow(currentWorkflow.id, {
          name: workflowName,
          nodes: workflowNodes,
          edges: workflowEdges,
        });
        setCurrentWorkflow(updated);
        alert('Workflow saved successfully!');
      } else {
        // Create new
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

  // Execute workflow
  const handleExecute = () => {
    if (nodes.length === 0) {
      alert('Please add some nodes first');
      return;
    }
    setShowExecutionPanel(true);
  };

  // Update node status during execution
  const updateNodeStatus = (nodeId: string, status: string, error?: string) => {
    setNodes((nds) =>
      nds.map((node) =>
        node.id === nodeId
          ? ({
              ...node,
              data: { ...node.data, status, ...(error && { error }) },
            } as Node)
          : node
      )
    );
  };

  // Batch update model for all AI nodes
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

  // Get AI nodes count
  const aiNodesCount = useMemo(() => {
    return nodes.filter(n => n.type === 'llm' || n.type === 'agent').length;
  }, [nodes]);

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
          <NodeTypePanel
            nodeTypes={nodeTypesList}
            onAddNode={handleAddNode}
          />
        </div>
      )}

      {/* Main Canvas Area */}
      <div className="flex-1 flex flex-col">
        {/* Toolbar */}
        <div className="flex items-center justify-between px-4 py-3 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <button
              onClick={() => setShowNodePanel(!showNodePanel)}
              className="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
              title="Toggle Node Panel"
            >
              {showNodePanel ? (
                <ChevronLeft className="w-5 h-5 text-gray-600" />
              ) : (
                <ChevronRight className="w-5 h-5 text-gray-600" />
              )}
            </button>
            <input
              type="text"
              value={workflowName}
              onChange={(e) => setWorkflowName(e.target.value)}
              className="text-lg font-semibold bg-transparent border-none focus:outline-none focus:ring-2 focus:ring-blue-500 rounded px-2 text-gray-900 dark:text-white"
              placeholder="Workflow Name"
            />
          </div>

          <div className="flex items-center gap-2">
            {/* Hidden file input for import */}
            <input
              ref={fileInputRef}
              type="file"
              accept=".json"
              className="hidden"
              onChange={(e) => handleImportWorkflow(e.target.files)}
            />
            
            {/* Search Box */}
            <div className="relative">
              <Search className="w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
              <input
                type="text"
                placeholder="Search nodes..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9 pr-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 w-40"
              />
            </div>
            
            <div className="w-px h-6 bg-gray-300 dark:bg-gray-600" />
            
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setShowTemplatePanel(true)}
              title="Templates"
            >
              <LayoutTemplate className="w-4 h-4 mr-1" />
              Templates
            </Button>
            
            <Button
              size="sm"
              variant="secondary"
              onClick={() => fileInputRef.current?.click()}
              title="Import Workflow"
            >
              <FolderOpen className="w-4 h-4 mr-1" />
              Import
            </Button>
            
            <Button
              size="sm"
              variant="secondary"
              onClick={handleExportWorkflow}
              disabled={nodes.length === 0}
              title="Export Workflow"
            >
              <Download className="w-4 h-4 mr-1" />
              Export
            </Button>
            
            <div className="w-px h-6 bg-gray-300 dark:bg-gray-600" />
            
            {/* Batch Model Update Dropdown */}
            {aiNodesCount > 0 && (
              <div className="relative group">
                <Button
                  size="sm"
                  variant="secondary"
                  title={`Update model for ${aiNodesCount} AI nodes`}
                >
                  <Brain className="w-4 h-4 mr-1" />
                  Set Model
                </Button>
                <div className="absolute right-0 top-full mt-1 hidden group-hover:block bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 min-w-[200px] p-2">
                  <p className="text-xs text-gray-500 px-2 py-1 border-b border-gray-200 dark:border-gray-700 mb-2">
                    Update all AI nodes ({aiNodesCount})
                  </p>
                  <ModelSelectorCompact
                    value=""
                    onChange={(model, provider) => {
                      if (provider) {
                        handleBatchUpdateModel(model, provider.id, provider.vendor);
                      }
                    }}
                    placeholder="Select model..."
                  />
                </div>
              </div>
            )}
            
            {/* Layout Dropdown */}
            <div className="relative group">
              <Button
                size="sm"
                variant="secondary"
                title="Layout Options"
              >
                <LayoutTemplate className="w-4 h-4 mr-1" />
                Layout
              </Button>
              <div className="absolute right-0 top-full mt-1 hidden group-hover:block bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 min-w-[140px]">
                <button
                  onClick={() => { setLayoutMode('auto'); handleLayout('auto'); }}
                  className={`w-full px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 first:rounded-t-lg flex items-center gap-2 ${layoutMode === 'auto' ? 'bg-blue-50 text-blue-600' : ''}`}
                >
                  <Grid3X3 className="w-4 h-4" />
                  Auto
                </button>
                <button
                  onClick={() => { setLayoutMode('hierarchical'); handleLayout('hierarchical'); }}
                  className={`w-full px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2 ${layoutMode === 'hierarchical' ? 'bg-blue-50 text-blue-600' : ''}`}
                >
                  <AlignVerticalJustifyCenter className="w-4 h-4" />
                  Hierarchical
                </button>
                <button
                  onClick={() => { setLayoutMode('tree'); handleLayout('tree'); }}
                  className={`w-full px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 last:rounded-b-lg flex items-center gap-2 ${layoutMode === 'tree' ? 'bg-blue-50 text-blue-600' : ''}`}
                >
                  <Network className="w-4 h-4" />
                  Tree
                </button>
              </div>
            </div>
            
            <Button
              size="sm"
              variant="secondary"
              onClick={handleClear}
              title="Clear Canvas"
            >
              <Trash2 className="w-4 h-4 mr-1" />
              Clear
            </Button>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setShowExecutionPanel(!showExecutionPanel)}
              className={showExecutionPanel ? 'bg-blue-100 text-blue-700' : ''}
            >
              <Terminal className="w-4 h-4 mr-1" />
              {showExecutionPanel ? 'Hide' : 'Run'}
            </Button>
            <Button
              size="sm"
              onClick={handleSave}
              disabled={isSaving}
            >
              {isSaving ? (
                <Loader2 className="w-4 h-4 mr-1 animate-spin" />
              ) : (
                <Save className="w-4 h-4 mr-1" />
              )}
              Save
            </Button>
          </div>
        </div>

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

// Wrap with ReactFlowProvider
export const Canvas: React.FC = () => {
  return (
    <ReactFlowProvider>
      <CanvasContent />
    </ReactFlowProvider>
  );
};
