/**
 * Workflow Templates Library
 * Pre-built workflow templates for common use cases
 */

import type { Workflow, WorkflowNode, WorkflowEdge } from '@/utils/chatToCanvas';
import { useModelStore } from '@/store/modelStore';

// Placeholder constants
export const TEMPLATE_PLACEHOLDERS = {
  DEFAULT_MODEL: '{{DEFAULT_MODEL}}',
  DEFAULT_PROVIDER: '{{DEFAULT_PROVIDER}}',
  DEFAULT_VENDOR: '{{DEFAULT_VENDOR}}',
} as const;

/**
 * Resolve template placeholders with actual values from modelStore
 */
export function resolveTemplatePlaceholders(workflow: Workflow): Workflow {
  try {
    const state = useModelStore.getState();
    const enabledProviders = state.providers.filter((p) => p.enabled && (!p.modelType || p.modelType === 'chat'));
    const sorted = enabledProviders.sort((a, b) => a.priority - b.priority);
    const defaultProvider = sorted[0];

    const resolvedNodes: WorkflowNode[] = workflow.nodes.map((node) => {
      if (node.node_type === 'llm' || node.node_type === 'agent') {
        const config = { ...(node.data.config || {}) };
        
        // Replace model placeholder
        if (config.model === TEMPLATE_PLACEHOLDERS.DEFAULT_MODEL) {
          config.model = defaultProvider?.model || 'gpt-4';
        }
        
        // Replace provider placeholder
        if (config.modelProvider === TEMPLATE_PLACEHOLDERS.DEFAULT_PROVIDER) {
          config.modelProvider = defaultProvider?.id;
        }
        
        // Replace vendor placeholder
        if (config.modelVendor === TEMPLATE_PLACEHOLDERS.DEFAULT_VENDOR) {
          config.modelVendor = defaultProvider?.vendor;
        }
        
        return {
          ...node,
          data: {
            ...node.data,
            config,
          },
        };
      }
      return node;
    });

    return {
      ...workflow,
      nodes: resolvedNodes,
    };
  } catch (e) {
    console.warn('Failed to resolve template placeholders:', e);
    return workflow;
  }
}

export interface WorkflowTemplate {
  id: string;
  name: string;
  description: string;
  category: string;
  icon: string;
  workflow: Workflow;
}

// Helper to create a workflow
function createWorkflow(
  name: string,
  description: string,
  nodes: WorkflowNode[],
  edges: WorkflowEdge[]
): Workflow {
  return {
    id: `template-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    name,
    description,
    nodes,
    edges,
    variables: {},
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    version: 1,
    is_active: true,
  };
}

// Template 1: Simple LLM Chain
const simpleLLMChain: WorkflowTemplate = {
  id: 'simple-llm-chain',
  name: 'Simple LLM Chain',
  description: 'A basic workflow that chains multiple LLM calls together',
  category: 'AI',
  icon: 'Brain',
  workflow: createWorkflow(
    'Simple LLM Chain',
    'Chain multiple LLM calls for complex reasoning',
    [
      {
        id: 'start',
        node_type: 'start',
        position: { x: 100, y: 100 },
        data: { label: 'Start', description: 'Workflow start' },
      },
      {
        id: 'llm-1',
        node_type: 'llm',
        position: { x: 100, y: 250 },
        data: {
          label: 'First LLM',
          description: 'Initial processing',
          config: { 
            model: '{{DEFAULT_MODEL}}', 
            modelProvider: '{{DEFAULT_PROVIDER}}',
            temperature: 0.7, 
            prompt: 'Process the input...' 
          },
        },
      },
      {
        id: 'llm-2',
        node_type: 'llm',
        position: { x: 100, y: 400 },
        data: {
          label: 'Second LLM',
          description: 'Refinement',
          config: { 
            model: '{{DEFAULT_MODEL}}', 
            modelProvider: '{{DEFAULT_PROVIDER}}',
            temperature: 0.5, 
            prompt: 'Refine the output...' 
          },
        },
      },
      {
        id: 'end',
        node_type: 'end',
        position: { x: 100, y: 550 },
        data: { label: 'End', description: 'Workflow end' },
      },
    ],
    [
      { id: 'edge-start-llm1', source: 'start', target: 'llm-1' },
      { id: 'edge-llm1-llm2', source: 'llm-1', target: 'llm-2' },
      { id: 'edge-llm2-end', source: 'llm-2', target: 'end' },
    ]
  ),
};

// Template 2: Data Processing Pipeline
const dataProcessingPipeline: WorkflowTemplate = {
  id: 'data-processing',
  name: 'Data Processing Pipeline',
  description: 'Extract, transform, and load data with validation',
  category: 'Data',
  icon: 'Database',
  workflow: createWorkflow(
    'Data Processing Pipeline',
    'ETL pipeline with validation and error handling',
    [
      {
        id: 'start',
        node_type: 'start',
        position: { x: 100, y: 100 },
        data: { label: 'Start', description: 'Workflow start' },
      },
      {
        id: 'http-fetch',
        node_type: 'http',
        position: { x: 100, y: 250 },
        data: {
          label: 'Fetch Data',
          description: 'Fetch from API',
          config: { method: 'GET', url: 'https://api.example.com/data' },
        },
      },
      {
        id: 'code-transform',
        node_type: 'code',
        position: { x: 100, y: 400 },
        data: {
          label: 'Transform',
          description: 'Transform data',
          config: { language: 'javascript', code: '// Transform logic here' },
        },
      },
      {
        id: 'condition-check',
        node_type: 'condition',
        position: { x: 100, y: 550 },
        data: {
          label: 'Valid?',
          description: 'Check if data is valid',
          config: { condition: 'data.isValid' },
        },
      },
      {
        id: 'variable-store',
        node_type: 'variable',
        position: { x: 300, y: 550 },
        data: {
          label: 'Store',
          description: 'Store valid data',
          config: { operation: 'set', variableName: 'processedData' },
        },
      },
      {
        id: 'end',
        node_type: 'end',
        position: { x: 100, y: 700 },
        data: { label: 'End', description: 'Workflow end' },
      },
    ],
    [
      { id: 'edge-start-http', source: 'start', target: 'http-fetch' },
      { id: 'edge-http-code', source: 'http-fetch', target: 'code-transform' },
      { id: 'edge-code-condition', source: 'code-transform', target: 'condition-check' },
      { id: 'edge-condition-true', source: 'condition-check', target: 'variable-store', label: 'true' },
      { id: 'edge-condition-false', source: 'condition-check', target: 'end', label: 'false' },
    ]
  ),
};

// Template 3: Agent Swarm
const agentSwarm: WorkflowTemplate = {
  id: 'agent-swarm',
  name: 'Agent Swarm',
  description: 'Multiple specialized agents working together',
  category: 'AI',
  icon: 'Bot',
  workflow: createWorkflow(
    'Agent Swarm',
    'Multiple agents collaborating on a complex task',
    [
      {
        id: 'start',
        node_type: 'start',
        position: { x: 300, y: 100 },
        data: { label: 'Start', description: 'Workflow start' },
      },
      {
        id: 'coordinator',
        node_type: 'agent',
        position: { x: 300, y: 250 },
        data: {
          label: 'Coordinator',
          description: 'Task coordinator',
          config: { role: 'coordinator', task: 'Coordinate the swarm' },
        },
      },
      {
        id: 'researcher',
        node_type: 'agent',
        position: { x: 100, y: 400 },
        data: {
          label: 'Researcher',
          description: 'Research agent',
          config: { role: 'researcher', task: 'Research information' },
        },
      },
      {
        id: 'analyst',
        node_type: 'agent',
        position: { x: 300, y: 400 },
        data: {
          label: 'Analyst',
          description: 'Analysis agent',
          config: { role: 'analyst', task: 'Analyze data' },
        },
      },
      {
        id: 'coder',
        node_type: 'agent',
        position: { x: 500, y: 400 },
        data: {
          label: 'Coder',
          description: 'Code agent',
          config: { role: 'coder', task: 'Write code' },
        },
      },
      {
        id: 'synthesizer',
        node_type: 'llm',
        position: { x: 300, y: 550 },
        data: {
          label: 'Synthesizer',
          description: 'Combine results',
          config: { 
            model: '{{DEFAULT_MODEL}}', 
            modelProvider: '{{DEFAULT_PROVIDER}}',
            prompt: 'Synthesize the outputs...' 
          },
        },
      },
      {
        id: 'end',
        node_type: 'end',
        position: { x: 300, y: 700 },
        data: { label: 'End', description: 'Workflow end' },
      },
    ],
    [
      { id: 'edge-start-coord', source: 'start', target: 'coordinator' },
      { id: 'edge-coord-research', source: 'coordinator', target: 'researcher' },
      { id: 'edge-coord-analyst', source: 'coordinator', target: 'analyst' },
      { id: 'edge-coord-coder', source: 'coordinator', target: 'coder' },
      { id: 'edge-research-synth', source: 'researcher', target: 'synthesizer' },
      { id: 'edge-analyst-synth', source: 'analyst', target: 'synthesizer' },
      { id: 'edge-coder-synth', source: 'coder', target: 'synthesizer' },
      { id: 'edge-synth-end', source: 'synthesizer', target: 'end' },
    ]
  ),
};

// Template 4: Knowledge Retrieval
const knowledgeRetrieval: WorkflowTemplate = {
  id: 'knowledge-retrieval',
  name: 'Knowledge Retrieval',
  description: 'RAG pipeline with knowledge base query',
  category: 'AI',
  icon: 'Book',
  workflow: createWorkflow(
    'Knowledge Retrieval',
    'Retrieve and synthesize information from knowledge base',
    [
      {
        id: 'start',
        node_type: 'start',
        position: { x: 100, y: 100 },
        data: { label: 'Start', description: 'Workflow start' },
      },
      {
        id: 'knowledge-query',
        node_type: 'knowledge',
        position: { x: 100, y: 250 },
        data: {
          label: 'Query KB',
          description: 'Query knowledge base',
          config: { query: '{{input}}', top_k: 5 },
        },
      },
      {
        id: 'template-context',
        node_type: 'template',
        position: { x: 100, y: 400 },
        data: {
          label: 'Build Context',
          description: 'Build context from results',
          config: { template: 'Context: {{knowledge.results}}\n\nQuestion: {{input}}' },
        },
      },
      {
        id: 'llm-answer',
        node_type: 'llm',
        position: { x: 100, y: 550 },
        data: {
          label: 'Generate Answer',
          description: 'Generate final answer',
          config: { 
            model: '{{DEFAULT_MODEL}}', 
            modelProvider: '{{DEFAULT_PROVIDER}}',
            prompt: '{{template.result}}' 
          },
        },
      },
      {
        id: 'end',
        node_type: 'end',
        position: { x: 100, y: 700 },
        data: { label: 'End', description: 'Workflow end' },
      },
    ],
    [
      { id: 'edge-start-kb', source: 'start', target: 'knowledge-query' },
      { id: 'edge-kb-template', source: 'knowledge-query', target: 'template-context' },
      { id: 'edge-template-llm', source: 'template-context', target: 'llm-answer' },
      { id: 'edge-llm-end', source: 'llm-answer', target: 'end' },
    ]
  ),
};

// Template 5: Loop Processing
const loopProcessing: WorkflowTemplate = {
  id: 'loop-processing',
  name: 'Batch Processing',
  description: 'Process items in a collection using a loop',
  category: 'Control',
  icon: 'Repeat',
  workflow: createWorkflow(
    'Batch Processing',
    'Process each item in a collection',
    [
      {
        id: 'start',
        node_type: 'start',
        position: { x: 100, y: 100 },
        data: { label: 'Start', description: 'Workflow start' },
      },
      {
        id: 'variable-items',
        node_type: 'variable',
        position: { x: 100, y: 250 },
        data: {
          label: 'Set Items',
          description: 'Initialize items array',
          config: { operation: 'set', variableName: 'items', variableValue: '[]' },
        },
      },
      {
        id: 'loop',
        node_type: 'loop',
        position: { x: 100, y: 400 },
        data: {
          label: 'Process Loop',
          description: 'Iterate over items',
          config: { iterations: 10 },
        },
      },
      {
        id: 'process-item',
        node_type: 'code',
        position: { x: 100, y: 550 },
        data: {
          label: 'Process Item',
          description: 'Process each item',
          config: { language: 'javascript', code: '// Process item logic' },
        },
      },
      {
        id: 'end',
        node_type: 'end',
        position: { x: 100, y: 700 },
        data: { label: 'End', description: 'Workflow end' },
      },
    ],
    [
      { id: 'edge-start-var', source: 'start', target: 'variable-items' },
      { id: 'edge-var-loop', source: 'variable-items', target: 'loop' },
      { id: 'edge-loop-process', source: 'loop', target: 'process-item' },
      { id: 'edge-process-end', source: 'process-item', target: 'end' },
    ]
  ),
};

// Template 6: API Integration
const apiIntegration: WorkflowTemplate = {
  id: 'api-integration',
  name: 'API Integration',
  description: 'Multi-step API integration with error handling',
  category: 'Integration',
  icon: 'Globe',
  workflow: createWorkflow(
    'API Integration',
    'Integrate with external APIs',
    [
      {
        id: 'start',
        node_type: 'start',
        position: { x: 100, y: 100 },
        data: { label: 'Start', description: 'Workflow start' },
      },
      {
        id: 'auth-request',
        node_type: 'http',
        position: { x: 100, y: 250 },
        data: {
          label: 'Authenticate',
          description: 'Get auth token',
          config: { method: 'POST', url: 'https://api.example.com/auth' },
        },
      },
      {
        id: 'auth-check',
        node_type: 'condition',
        position: { x: 100, y: 400 },
        data: {
          label: 'Auth Success?',
          description: 'Check auth response',
          config: { condition: 'response.status === 200' },
        },
      },
      {
        id: 'api-call',
        node_type: 'http',
        position: { x: 300, y: 400 },
        data: {
          label: 'API Call',
          description: 'Make API request',
          config: { method: 'GET', url: 'https://api.example.com/data' },
        },
      },
      {
        id: 'process-response',
        node_type: 'code',
        position: { x: 300, y: 550 },
        data: {
          label: 'Process',
          description: 'Process API response',
          config: { language: 'javascript', code: '// Process response' },
        },
      },
      {
        id: 'end',
        node_type: 'end',
        position: { x: 100, y: 700 },
        data: { label: 'End', description: 'Workflow end' },
      },
    ],
    [
      { id: 'edge-start-auth', source: 'start', target: 'auth-request' },
      { id: 'edge-auth-check', source: 'auth-request', target: 'auth-check' },
      { id: 'edge-check-true', source: 'auth-check', target: 'api-call', label: 'true' },
      { id: 'edge-check-false', source: 'auth-check', target: 'end', label: 'false' },
      { id: 'edge-api-process', source: 'api-call', target: 'process-response' },
      { id: 'edge-process-end', source: 'process-response', target: 'end' },
    ]
  ),
};

// Export all templates
export const workflowTemplates: WorkflowTemplate[] = [
  simpleLLMChain,
  dataProcessingPipeline,
  agentSwarm,
  knowledgeRetrieval,
  loopProcessing,
  apiIntegration,
];

// Get templates by category
export function getTemplatesByCategory(category: string): WorkflowTemplate[] {
  return workflowTemplates.filter(t => t.category === category);
}

// Get all categories
export function getTemplateCategories(): string[] {
  return [...new Set(workflowTemplates.map(t => t.category))];
}

// Get template by ID
export function getTemplateById(id: string): WorkflowTemplate | undefined {
  return workflowTemplates.find(t => t.id === id);
}
