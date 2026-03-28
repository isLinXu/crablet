/**
 * Chat to Canvas Converter
 * Converts chat messages to workflow nodes
 */

import type { ExtendedMessage } from '@/store/chatStore';
import { useModelStore } from '@/store/modelStore';

// Re-define types to avoid import issues
export interface WorkflowNode {
  id: string;
  node_type: string;
  position: { x: number; y: number };
  data: {
    label: string;
    description?: string;
    config?: Record<string, unknown>;
    inputs?: { name: string; type: string; description?: string; required?: boolean; default?: unknown }[];
    outputs?: { name: string; type: string; description?: string }[];
  };
}

export interface WorkflowEdge {
  id: string;
  source: string;
  target: string;
  source_handle?: string;
  target_handle?: string;
  label?: string;
  condition?: string;
}

export interface Workflow {
  id: string;
  name: string;
  description?: string;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  variables?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  created_by?: string;
  version?: number;
  is_active?: boolean;
}

export interface ChatToCanvasOptions {
  workflowName?: string;
  includeSystemMessages?: boolean;
  groupConsecutiveMessages?: boolean;
  sessionModel?: string;           // Current session model name
  sessionProvider?: string;        // Current session provider ID
  sessionVendor?: string;          // Current session vendor name
}

/**
 * Detect node type from message content
 */
function detectNodeType(content: string): string {
  const lowerContent = content.toLowerCase();
  
  // Code detection
  if (content.includes('```') || /\b(function|class|const|let|var|import|export|def|print)\b/.test(content)) {
    return 'code';
  }
  
  // HTTP/API detection
  if (/\b(http|https|api|request|endpoint|url|fetch|axios)\b/i.test(lowerContent)) {
    return 'http';
  }
  
  // Condition/Logic detection
  if (/\b(if|else|condition|check|validate|verify|compare)\b/i.test(lowerContent) && 
      (lowerContent.includes('?') || lowerContent.includes('condition'))) {
    return 'condition';
  }
  
  // Loop detection
  if (/\b(loop|for|while|iterate|each|every|repeat)\b/i.test(lowerContent)) {
    return 'loop';
  }
  
  // Knowledge base detection
  if (/\b(search|query|find|lookup|knowledge|database|retrieve)\b/i.test(lowerContent)) {
    return 'knowledge';
  }
  
  // Template detection
  if (content.includes('{{') && content.includes('}}')) {
    return 'template';
  }
  
  // Variable detection
  if (/\b(variable|set|assign|store|save)\b/i.test(lowerContent)) {
    return 'variable';
  }
  
  // Agent detection
  if (/\b(agent|task|execute|run|perform|action)\b/i.test(lowerContent)) {
    return 'agent';
  }
  
  // Default to LLM for assistant messages
  return 'llm';
}

// Get default model from store (for non-React context)
function getDefaultModelFromStore(): { model: string; modelProvider?: string; modelVendor?: string } {
  try {
    const state = useModelStore.getState();
    const enabledProviders = state.providers.filter((p) => p.enabled && (!p.modelType || p.modelType === 'chat'));
    const sorted = enabledProviders.sort((a, b) => a.priority - b.priority);
    const defaultProvider = sorted[0];
    
    if (defaultProvider) {
      return {
        model: defaultProvider.model,
        modelProvider: defaultProvider.id,
        modelVendor: defaultProvider.vendor,
      };
    }
  } catch (e) {
    console.warn('Failed to get default model from store:', e);
  }
  
  return { model: 'gpt-4' };
}

/**
 * Extract configuration from message content
 */
function extractNodeConfig(
  content: string, 
  type: string, 
  sessionModelInfo?: { model?: string; provider?: string; vendor?: string }
): Record<string, unknown> {
  const config: Record<string, unknown> = {};
  
  switch (type) {
    case 'code': {
      // Extract code blocks
      const codeMatch = content.match(/```(\w+)?\n([\s\S]*?)```/);
      if (codeMatch) {
        config.language = codeMatch[1] || 'javascript';
        config.code = codeMatch[2].trim();
      } else {
        config.language = 'javascript';
        config.code = content;
      }
      break;
    }
      
    case 'http': {
      // Extract URL
      const urlMatch = content.match(/https?:\/\/[^\s)]+/);
      if (urlMatch) {
        config.url = urlMatch[0];
      }
      config.method = 'GET';
      // Check for method mentions
      if (/\bPOST\b/i.test(content)) config.method = 'POST';
      else if (/\bPUT\b/i.test(content)) config.method = 'PUT';
      else if (/\bDELETE\b/i.test(content)) config.method = 'DELETE';
      break;
    }
      
    case 'condition':
      config.condition = content.slice(0, 200);
      break;
      
    case 'loop': {
      const loopMatch = content.match(/\b(\d+)\s*times?\b/i) || content.match(/\bfor\s+(\d+)\b/i);
      if (loopMatch) {
        config.iterations = parseInt(loopMatch[1], 10);
      }
      break;
    }
      
    case 'knowledge': {
      const queryMatch = content.match(/["']([^"']+)["']/);
      if (queryMatch) {
        config.query = queryMatch[1];
      } else {
        config.query = content.slice(0, 100);
      }
      break;
    }
      
    case 'template':
      config.template = content;
      break;
      
    case 'variable': {
      const varMatch = content.match(/\b(\w+)\s*=\s*["']([^"']+)["']/);
      if (varMatch) {
        config.variableName = varMatch[1];
        config.variableValue = varMatch[2];
      }
      break;
    }
      
    case 'agent':
      config.role = 'assistant';
      if (/\bcoder?\b/i.test(content)) config.role = 'coder';
      else if (/\bresearcher?\b/i.test(content)) config.role = 'researcher';
      else if (/\banalyst?\b/i.test(content)) config.role = 'analyst';
      config.task = content.slice(0, 500);
      // Agent can optionally use a specific model
      // Don't set model here, let it use system default
      break;
      
    case 'llm':
    default:
      config.prompt = content.slice(0, 2000);
      
      // Use session model if available, otherwise try to detect from content, then fallback to default
      if (sessionModelInfo?.model) {
        config.model = sessionModelInfo.model;
        if (sessionModelInfo.provider) config.modelProvider = sessionModelInfo.provider;
        if (sessionModelInfo.vendor) config.modelVendor = sessionModelInfo.vendor;
      } else {
        // Try to detect model from content
        const modelMatch = content.match(/\b(gpt-4|gpt-3\.5|claude|llama|gemini|qwen|deepseek)\b/i);
        if (modelMatch) {
          config.model = modelMatch[1].toLowerCase();
        } else {
          // Use default from store
          const defaultModel = getDefaultModelFromStore();
          config.model = defaultModel.model;
          if (defaultModel.modelProvider) config.modelProvider = defaultModel.modelProvider;
          if (defaultModel.modelVendor) config.modelVendor = defaultModel.modelVendor;
        }
      }
      config.temperature = 0.7;
      break;
  }
  
  return config;
}

/**
 * Convert chat messages to workflow
 */
export function convertChatToCanvas(
  messages: ExtendedMessage[],
  options: ChatToCanvasOptions = {}
): Workflow {
  const {
    workflowName = 'Chat Workflow',
    includeSystemMessages = false,
    sessionModel,
    sessionProvider,
    sessionVendor,
  } = options;

  // Build session model info
  const sessionModelInfo = sessionModel ? {
    model: sessionModel,
    provider: sessionProvider,
    vendor: sessionVendor,
  } : undefined;

  // Filter messages
  const filteredMessages = messages.filter(m => 
    m.role === 'user' || m.role === 'assistant' || (includeSystemMessages && m.role === 'system')
  );

  if (filteredMessages.length === 0) {
    throw new Error('No valid messages to convert');
  }

  const nodes: WorkflowNode[] = [];
  const edges: WorkflowEdge[] = [];
  
  // Add start node
  const startNode: WorkflowNode = {
    id: 'start',
    node_type: 'start',
    position: { x: 100, y: 100 },
    data: {
      label: 'Start',
      description: 'Workflow start',
    },
  };
  nodes.push(startNode);

  let currentY = 200;
  let previousNodeId = 'start';

  // Convert messages to nodes
  filteredMessages.forEach((message, index) => {
    const content = typeof message.content === 'string' 
      ? message.content 
      : JSON.stringify(message.content);

    // Skip empty messages
    if (!content.trim()) return;

    // Determine node type based on content and role
    const nodeType = detectNodeType(content);

    // Create node
    const nodeId = `node-${index}`;
    const config = extractNodeConfig(content, nodeType, sessionModelInfo);
    
    const node: WorkflowNode = {
      id: nodeId,
      node_type: nodeType,
      position: { x: 100, y: currentY },
      data: {
        label: message.role === 'user' ? `User Input ${index + 1}` : `AI Response ${index + 1}`,
        description: content.slice(0, 100) + (content.length > 100 ? '...' : ''),
        config,
      },
    };
    
    nodes.push(node);

    // Create edge from previous node
    const edge: WorkflowEdge = {
      id: `edge-${previousNodeId}-${nodeId}`,
      source: previousNodeId,
      target: nodeId,
    };
    edges.push(edge);

    previousNodeId = nodeId;
    currentY += 150;
  });

  // Add end node
  const endNode: WorkflowNode = {
    id: 'end',
    node_type: 'end',
    position: { x: 100, y: currentY },
    data: {
      label: 'End',
      description: 'Workflow end',
    },
  };
  nodes.push(endNode);

  // Connect last message node to end
  if (previousNodeId !== 'start') {
    edges.push({
      id: `edge-${previousNodeId}-end`,
      source: previousNodeId,
      target: 'end',
    });
  }

  return {
    id: `workflow-${Date.now()}`,
    name: workflowName,
    description: `Generated from chat with ${messages.length} messages`,
    nodes,
    edges,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
}

/**
 * Export workflow to JSON file
 */
export function exportWorkflowToJSON(workflow: Workflow): string {
  return JSON.stringify(workflow, null, 2);
}

/**
 * Import workflow from JSON
 */
export function importWorkflowFromJSON(jsonString: string): Workflow {
  try {
    const workflow = JSON.parse(jsonString) as Workflow;
    
    // Validate required fields
    if (!workflow.nodes || !Array.isArray(workflow.nodes)) {
      throw new Error('Invalid workflow: missing nodes array');
    }
    if (!workflow.edges || !Array.isArray(workflow.edges)) {
      throw new Error('Invalid workflow: missing edges array');
    }
    
    return workflow;
  } catch (error) {
    throw new Error(`Failed to import workflow: ${error instanceof Error ? error.message : 'Unknown error'}`);
  }
}

/**
 * Download workflow as JSON file
 */
export function downloadWorkflow(workflow: Workflow): void {
  const json = exportWorkflowToJSON(workflow);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  
  const a = document.createElement('a');
  a.href = url;
  a.download = `${workflow.name.replace(/\s+/g, '_')}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  
  URL.revokeObjectURL(url);
}

/**
 * Read workflow from file
 */
export function readWorkflowFromFile(file: File): Promise<Workflow> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    
    reader.onload = (e) => {
      try {
        const content = e.target?.result as string;
        const workflow = importWorkflowFromJSON(content);
        resolve(workflow);
      } catch (error) {
        reject(error);
      }
    };
    
    reader.onerror = () => {
      reject(new Error('Failed to read file'));
    };
    
    reader.readAsText(file);
  });
}
