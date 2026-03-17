// Workflow types

export interface Workflow {
  id: string;
  name: string;
  description?: string;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  variables: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  created_by?: string;
  version: number;
  is_active: boolean;
}

export interface WorkflowNode {
  id: string;
  node_type: string;
  position: { x: number; y: number };
  data: {
    label: string;
    description?: string;
    config?: Record<string, unknown>;
    inputs?: Variable[];
    outputs?: Variable[];
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

export interface Variable {
  name: string;
  type: string;
  description?: string;
  required?: boolean;
  default?: unknown;
}

export interface WorkflowExecution {
  id: string;
  workflow_id: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  inputs: Record<string, unknown>;
  outputs?: Record<string, unknown>;
  node_executions: NodeExecution[];
  started_at: string;
  completed_at?: string;
  error?: string;
}

export interface NodeExecution {
  node_id: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  inputs?: Record<string, unknown>;
  outputs?: Record<string, unknown>;
  started_at?: string;
  completed_at?: string;
  error?: string;
}

export interface CreateWorkflowRequest {
  name: string;
  description?: string;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
}

export interface UpdateWorkflowRequest {
  name?: string;
  description?: string;
  nodes?: WorkflowNode[];
  edges?: WorkflowEdge[];
}

export interface ExecuteWorkflowRequest {
  inputs: Record<string, unknown>;
}

export interface NodeTypeDefinition {
  type: string;
  name: string;
  description: string;
  category: string;
  icon: string;
  color: string;
  inputs?: { name: string; type: string; optional?: boolean }[];
  outputs?: { name: string; type: string }[];
}

export interface ExecutionEvent {
  event_type: string;
  execution_id: string;
  workflow_id?: string;
  node_id?: string;
  node_type?: string;
  outputs?: Record<string, unknown>;
  error?: string;
  variable?: string;
  value?: unknown;
  timestamp: string;
}
