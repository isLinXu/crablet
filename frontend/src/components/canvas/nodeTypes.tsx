import React from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';
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
  Loader2,
  CheckCircle,
  XCircle,
} from 'lucide-react';

// Base node wrapper with common styling
const BaseNode: React.FC<{
  children: React.ReactNode;
  color: string;
  selected?: boolean;
  status?: string;
  error?: string;
  inputs?: { name: string; type: string }[];
  outputs?: { name: string; type: string }[];
}> = ({ children, color, selected, status, error, inputs = [], outputs = [] }) => {
  const getStatusStyles = () => {
    switch (status) {
      case 'running':
        return 'ring-2 ring-yellow-400 shadow-yellow-400/50 animate-pulse';
      case 'completed':
        return 'ring-2 ring-green-400 shadow-green-400/50';
      case 'failed':
        return 'ring-2 ring-red-400 shadow-red-400/50';
      default:
        return selected ? 'ring-2 ring-blue-500 shadow-blue-500/50' : '';
    }
  };

  return (
    <div
      className={`min-w-[180px] bg-white dark:bg-gray-800 rounded-xl shadow-lg border-2 transition-all duration-200 ${getStatusStyles()}`}
      style={{ borderColor: status ? undefined : color }}
    >
      {/* Input Handles */}
      {inputs.map((input, index) => (
        <Handle
          key={`input-${index}`}
          type="target"
          position={Position.Top}
          id={input.name}
          style={{
            top: -8,
            left: `${((index + 1) / (inputs.length + 1)) * 100}%`,
            background: color,
            width: 12,
            height: 12,
          }}
        />
      ))}

      {/* Content */}
      <div className="p-4">
        {children}
        {error && (
          <div className="mt-2 text-xs text-red-500 bg-red-50 dark:bg-red-900/20 p-2 rounded">
            {error}
          </div>
        )}
      </div>

      {/* Output Handles */}
      {outputs.map((output, index) => (
        <Handle
          key={`output-${index}`}
          type="source"
          position={Position.Bottom}
          id={output.name}
          style={{
            bottom: -8,
            left: `${((index + 1) / (outputs.length + 1)) * 100}%`,
            background: color,
            width: 12,
            height: 12,
          }}
        />
      ))}
    </div>
  );
};

// Status badge component
const StatusBadge: React.FC<{ status?: string }> = ({ status }) => {
  if (!status) return null;

  const icons = {
    running: <Loader2 className="w-4 h-4 animate-spin text-yellow-500" />,
    completed: <CheckCircle className="w-4 h-4 text-green-500" />,
    failed: <XCircle className="w-4 h-4 text-red-500" />,
  };

  return (
    <div className="absolute -top-2 -right-2 bg-white dark:bg-gray-800 rounded-full p-1 shadow-md">
      {icons[status as keyof typeof icons]}
    </div>
  );
};

// Start Node
export const StartNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#10b981"
      selected={selected}
      status={data.status as string}
      outputs={[{ name: 'output', type: 'any' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center">
          <Play className="w-5 h-5 text-emerald-600" />
        </div>
        <div>
          <div className="font-semibold text-gray-900 dark:text-white">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">
            {(data.config as any)?.variables?.length || 0} variables
          </div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// End Node
export const EndNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#ef4444"
      selected={selected}
      status={data.status as string}
      inputs={[{ name: 'input', type: 'any' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-red-100 dark:bg-red-900/30 flex items-center justify-center">
          <Square className="w-5 h-5 text-red-600" />
        </div>
        <div>
          <div className="font-semibold text-gray-900 dark:text-white">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">Workflow end</div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// LLM Node
export const LLMNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#8b5cf6"
      selected={selected}
      status={data.status as string}
      error={data.error as string}
      inputs={[
        { name: 'prompt', type: 'string' },
        { name: 'system_prompt', type: 'string' },
      ]}
      outputs={[{ name: 'text', type: 'string' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-violet-100 dark:bg-violet-900/30 flex items-center justify-center">
          <Brain className="w-5 h-5 text-violet-600" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-semibold text-gray-900 dark:text-white truncate">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
            {(data.config as { model?: string })?.model || 'gpt-4'}
          </div>
        </div>
      </div>
      {(data.config as { prompt?: string })?.prompt && (
        <div className="mt-2 text-xs text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-700/50 p-2 rounded truncate">
          {(data.config as { prompt?: string }).prompt?.slice(0, 50)}...
        </div>
      )}
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Agent Node
export const AgentNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#3b82f6"
      selected={selected}
      status={data.status as string}
      error={data.error as string}
      inputs={[{ name: 'task', type: 'string' }]}
      outputs={[{ name: 'result', type: 'string' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
          <Bot className="w-5 h-5 text-blue-600" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-semibold text-gray-900 dark:text-white truncate">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400 capitalize">
            {(data.config as { role?: string })?.role || 'coder'}
          </div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Code Node
export const CodeNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#f59e0b"
      selected={selected}
      status={data.status as string}
      error={data.error as string}
      inputs={[{ name: 'input', type: 'any' }]}
      outputs={[{ name: 'result', type: 'any' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-amber-100 dark:bg-amber-900/30 flex items-center justify-center">
          <Code className="w-5 h-5 text-amber-600" />
        </div>
        <div>
          <div className="font-semibold text-gray-900 dark:text-white">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400 uppercase">
            {(data.config as { language?: string })?.language || 'python'}
          </div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// HTTP Node
export const HTTPNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#06b6d4"
      selected={selected}
      status={data.status as string}
      error={data.error as string}
      inputs={[{ name: 'url', type: 'string' }]}
      outputs={[
        { name: 'status', type: 'number' },
        { name: 'body', type: 'any' },
      ]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-cyan-100 dark:bg-cyan-900/30 flex items-center justify-center">
          <Globe className="w-5 h-5 text-cyan-600" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-semibold text-gray-900 dark:text-white truncate">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">
            {(data.config as { method?: string })?.method || 'GET'}
          </div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Condition Node
export const ConditionNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#ec4899"
      selected={selected}
      status={data.status as string}
      inputs={[{ name: 'input', type: 'any' }]}
      outputs={[
        { name: 'true', type: 'any' },
        { name: 'false', type: 'any' },
      ]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-pink-100 dark:bg-pink-900/30 flex items-center justify-center">
          <GitBranch className="w-5 h-5 text-pink-600" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-semibold text-gray-900 dark:text-white truncate">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
            {(data.config as { condition?: string })?.condition || 'condition'}
          </div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Loop Node
export const LoopNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#84cc16"
      selected={selected}
      status={data.status as string}
      inputs={[{ name: 'items', type: 'array' }]}
      outputs={[{ name: 'item', type: 'any' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-lime-100 dark:bg-lime-900/30 flex items-center justify-center">
          <Repeat className="w-5 h-5 text-lime-600" />
        </div>
        <div>
          <div className="font-semibold text-gray-900 dark:text-white">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">Iterate collection</div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Variable Node
export const VariableNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#6366f1"
      selected={selected}
      status={data.status as string}
      inputs={[{ name: 'value', type: 'any' }]}
      outputs={[{ name: 'result', type: 'any' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-indigo-100 dark:bg-indigo-900/30 flex items-center justify-center">
          <Database className="w-5 h-5 text-indigo-600" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-semibold text-gray-900 dark:text-white truncate">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">
            {(data.config as { operation?: string })?.operation || 'set'}
          </div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Template Node
export const TemplateNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#14b8a6"
      selected={selected}
      status={data.status as string}
      inputs={[{ name: 'variables', type: 'object' }]}
      outputs={[{ name: 'result', type: 'string' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-teal-100 dark:bg-teal-900/30 flex items-center justify-center">
          <FileText className="w-5 h-5 text-teal-600" />
        </div>
        <div>
          <div className="font-semibold text-gray-900 dark:text-white">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">Text template</div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Knowledge Node
export const KnowledgeNode: React.FC<NodeProps> = ({ data, selected }) => (
  <div className="relative">
    <BaseNode
      color="#a855f7"
      selected={selected}
      status={data.status as string}
      error={data.error as string}
      inputs={[{ name: 'query', type: 'string' }]}
      outputs={[{ name: 'results', type: 'array' }]}
    >
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-lg bg-purple-100 dark:bg-purple-900/30 flex items-center justify-center">
          <Book className="w-5 h-5 text-purple-600" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-semibold text-gray-900 dark:text-white truncate">{String(data.label)}</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">
            Top {(data.config as { top_k?: number })?.top_k || 5}
          </div>
        </div>
      </div>
    </BaseNode>
    <StatusBadge status={data.status as string} />
  </div>
);

// Export all node types
export const nodeTypes = {
  start: StartNode,
  end: EndNode,
  llm: LLMNode,
  agent: AgentNode,
  code: CodeNode,
  http: HTTPNode,
  condition: ConditionNode,
  loop: LoopNode,
  variable: VariableNode,
  template: TemplateNode,
  knowledge: KnowledgeNode,
};
