import { useModelStore } from '@/store/modelStore';
import type { NodeTypeDefinition } from '../../types/workflow';

export const defaultNodeTypes: NodeTypeDefinition[] = [
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

export function getDefaultModelConfig(): { model: string; modelProvider?: string; modelVendor?: string } {
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
  return { model: 'gpt-4' };
}

export function getDefaultConfig(type: string): Record<string, unknown> {
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
}
