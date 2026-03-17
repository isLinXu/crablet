import type {
  Workflow,
  WorkflowExecution,
  CreateWorkflowRequest,
  UpdateWorkflowRequest,
  ExecuteWorkflowRequest,
  NodeTypeDefinition,
  ExecutionEvent,
} from '../types/workflow';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000';

const getAuthHeaders = (): HeadersInit => {
  const token = localStorage.getItem('token');
  return {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };
};

// Workflow CRUD Operations
export const workflowApi = {
  // Create a new workflow
  async createWorkflow(request: CreateWorkflowRequest): Promise<Workflow> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows`, {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || 'Failed to create workflow');
    }

    return response.json();
  },

  // List all workflows
  async listWorkflows(): Promise<Workflow[]> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows`, {
      headers: getAuthHeaders(),
    });

    if (!response.ok) {
      throw new Error('Failed to list workflows');
    }

    return response.json();
  },

  // Get a workflow by ID
  async getWorkflow(id: string): Promise<Workflow> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows/${id}`, {
      headers: getAuthHeaders(),
    });

    if (!response.ok) {
      throw new Error('Failed to get workflow');
    }

    return response.json();
  },

  // Update a workflow
  async updateWorkflow(id: string, request: UpdateWorkflowRequest): Promise<Workflow> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows/${id}`, {
      method: 'PUT',
      headers: getAuthHeaders(),
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || 'Failed to update workflow');
    }

    return response.json();
  },

  // Delete a workflow
  async deleteWorkflow(id: string): Promise<void> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows/${id}`, {
      method: 'DELETE',
      headers: getAuthHeaders(),
    });

    if (!response.ok) {
      throw new Error('Failed to delete workflow');
    }
  },

  // Validate a workflow
  async validateWorkflow(request: CreateWorkflowRequest): Promise<{ valid: boolean; errors: string[] }> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows/validate`, {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error('Failed to validate workflow');
    }

    return response.json();
  },

  // Get available node types
  async getNodeTypes(): Promise<NodeTypeDefinition[]> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows/node-types`, {
      headers: getAuthHeaders(),
    });

    if (!response.ok) {
      throw new Error('Failed to get node types');
    }

    return response.json();
  },
};

// Workflow Execution Operations
export const executionApi = {
  // Execute a workflow (async)
  async executeWorkflow(
    workflowId: string,
    request: ExecuteWorkflowRequest
  ): Promise<WorkflowExecution> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows/${workflowId}/execute`, {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || 'Failed to execute workflow');
    }

    return response.json();
  },

  // Run a workflow with real-time SSE updates
  runWorkflowStream(
    workflowId: string,
    request: ExecuteWorkflowRequest,
    onEvent: (event: ExecutionEvent) => void,
    onError?: (error: Error) => void
  ): () => void {
    const token = localStorage.getItem('token');
    const url = new URL(`${API_BASE_URL}/api/v1/workflows/${workflowId}/run`);
    
    // For SSE with auth, we need to use EventSource with headers polyfill or query param
    // Here we use a simple fetch-based approach for broader compatibility
    const controller = new AbortController();
    
    fetch(url.toString(), {
      method: 'POST',
      headers: {
        ...getAuthHeaders(),
        'Accept': 'text/event-stream',
      },
      body: JSON.stringify(request),
      signal: controller.signal,
    }).then(async (response) => {
      if (!response.ok) {
        throw new Error('Failed to start execution stream');
      }
      
      const reader = response.body?.getReader();
      if (!reader) return;
      
      const decoder = new TextDecoder();
      let buffer = '';
      
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        
        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';
        
        for (const line of lines) {
          if (line.startsWith('data: ')) {
            try {
              const data = JSON.parse(line.slice(6));
              onEvent(data);
            } catch (e) {
              console.error('Failed to parse SSE data:', e);
            }
          }
        }
      }
    }).catch((error) => {
      if (error.name !== 'AbortError') {
        onError?.(error);
      }
    });

    // Return cleanup function
    return () => {
      controller.abort();
    };
  },

  // Get execution by ID
  async getExecution(executionId: string): Promise<WorkflowExecution> {
    const response = await fetch(`${API_BASE_URL}/api/v1/executions/${executionId}`, {
      headers: getAuthHeaders(),
    });

    if (!response.ok) {
      throw new Error('Failed to get execution');
    }

    return response.json();
  },

  // List executions for a workflow
  async listExecutions(workflowId: string): Promise<WorkflowExecution[]> {
    const response = await fetch(`${API_BASE_URL}/api/v1/workflows/${workflowId}/executions`, {
      headers: getAuthHeaders(),
    });

    if (!response.ok) {
      throw new Error('Failed to list executions');
    }

    return response.json();
  },

  // Cancel an execution
  async cancelExecution(executionId: string): Promise<void> {
    const response = await fetch(`${API_BASE_URL}/api/v1/executions/${executionId}`, {
      method: 'DELETE',
      headers: getAuthHeaders(),
    });

    if (!response.ok) {
      throw new Error('Failed to cancel execution');
    }
  },
};
