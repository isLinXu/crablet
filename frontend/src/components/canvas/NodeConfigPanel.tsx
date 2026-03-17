import React from 'react';
import { X, Trash2, Variable, Plus, Settings } from 'lucide-react';
import { Button } from '../ui/Button';
import type { Node } from '@xyflow/react';
import { ModelSelector, ModelSelectorCompact } from './ModelSelector';
import { useModelStore } from '@/store/modelStore';

interface NodeConfigPanelProps {
  node: Node;
  onUpdate: (data: Record<string, unknown>) => void;
  onDelete: () => void;
  onClose: () => void;
}

export const NodeConfigPanel: React.FC<NodeConfigPanelProps> = ({
  node,
  onUpdate,
  onDelete,
  onClose,
}) => {
  const { data, type } = node;
  const config = (data.config as Record<string, unknown>) || {};

  const handleConfigChange = (key: string, value: unknown) => {
    onUpdate({
      config: { ...config, [key]: value },
    });
  };

  const renderVariableList = () => {
    const variables = (config.variables as Array<{ name: string; type: string; required?: boolean; default?: unknown }>) || [];

    return (
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
            Variables
          </label>
          <button
            onClick={() => {
              const newVar = { name: '', type: 'string', required: true };
              handleConfigChange('variables', [...variables, newVar]);
            }}
            className="text-xs text-blue-600 hover:text-blue-700 flex items-center gap-1"
          >
            <Plus className="w-3 h-3" /> Add
          </button>
        </div>
        {variables.map((variable, index) => (
          <div key={index} className="space-y-2 p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
            <input
              type="text"
              value={variable.name}
              onChange={(e) => {
                const newVars = [...variables];
                newVars[index] = { ...variable, name: e.target.value };
                handleConfigChange('variables', newVars);
              }}
              placeholder="Variable name"
              className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
            <div className="flex gap-2">
              <select
                value={variable.type}
                onChange={(e) => {
                  const newVars = [...variables];
                  newVars[index] = { ...variable, type: e.target.value };
                  handleConfigChange('variables', newVars);
                }}
                className="flex-1 px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              >
                <option value="string">String</option>
                <option value="number">Number</option>
                <option value="boolean">Boolean</option>
                <option value="array">Array</option>
                <option value="object">Object</option>
              </select>
              <button
                onClick={() => {
                  const newVars = variables.filter((_, i) => i !== index);
                  handleConfigChange('variables', newVars);
                }}
                className="p-2 text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </div>
          </div>
        ))}
      </div>
    );
  };

  const renderFields = () => {
    switch (type) {
      case 'start':
        return renderVariableList();

      case 'end':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Return Value
              </label>
              <input
                type="text"
                value={(config.return_value as string) || ''}
                onChange={(e) => handleConfigChange('return_value', e.target.value)}
                placeholder="{{variable}}"
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
              <p className="text-xs text-gray-500 mt-1">
                Use {'{{variable}}'} to reference variables
              </p>
            </div>
          </div>
        );

      case 'llm':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Model
              </label>
              <ModelSelector
                value={(config.model as string) || ''}
                onChange={(model, provider) => {
                  handleConfigChange('model', model);
                  if (provider) {
                    handleConfigChange('modelProvider', provider.id);
                    handleConfigChange('modelVendor', provider.vendor);
                  }
                }}
                showProvider={true}
                showIcon={true}
                placeholder="Select a model..."
              />
              <p className="text-xs text-gray-500 mt-1">
                Models are configured in Settings
              </p>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                System Prompt
              </label>
              <textarea
                value={(config.system_prompt as string) || ''}
                onChange={(e) => handleConfigChange('system_prompt', e.target.value)}
                placeholder="You are a helpful assistant..."
                rows={3}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Prompt
              </label>
              <textarea
                value={(config.prompt as string) || ''}
                onChange={(e) => handleConfigChange('prompt', e.target.value)}
                placeholder="Enter your prompt here... Use {{variable}} for variables"
                rows={4}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Temperature
                </label>
                <input
                  type="number"
                  min="0"
                  max="2"
                  step="0.1"
                  value={(config.temperature as number) ?? 0.7}
                  onChange={(e) => handleConfigChange('temperature', parseFloat(e.target.value))}
                  className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Max Tokens
                </label>
                <input
                  type="number"
                  min="1"
                  max="8000"
                  value={(config.max_tokens as number) ?? 2000}
                  onChange={(e) => handleConfigChange('max_tokens', parseInt(e.target.value))}
                  className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                />
              </div>
            </div>
          </div>
        );

      case 'agent':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Agent Role
              </label>
              <select
                value={(config.role as string) || 'coder'}
                onChange={(e) => handleConfigChange('role', e.target.value)}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              >
                <option value="coder">Coder</option>
                <option value="researcher">Researcher</option>
                <option value="analyst">Analyst</option>
                <option value="planner">Planner</option>
                <option value="reviewer">Reviewer</option>
                <option value="security">Security</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Model (Optional)
              </label>
              <ModelSelector
                value={(config.model as string) || ''}
                onChange={(model, provider) => {
                  handleConfigChange('model', model);
                  if (provider) {
                    handleConfigChange('modelProvider', provider.id);
                    handleConfigChange('modelVendor', provider.vendor);
                  }
                }}
                showProvider={true}
                showIcon={true}
                placeholder="Use default model..."
              />
              <p className="text-xs text-gray-500 mt-1">
                Leave empty to use system default model
              </p>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Task
              </label>
              <textarea
                value={(config.task as string) || ''}
                onChange={(e) => handleConfigChange('task', e.target.value)}
                placeholder="Describe the task... Use {{variable}} for variables"
                rows={4}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Context
              </label>
              <textarea
                value={(config.context as string) || ''}
                onChange={(e) => handleConfigChange('context', e.target.value)}
                placeholder="Additional context..."
                rows={3}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
            </div>
          </div>
        );

      case 'code':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Language
              </label>
              <select
                value={(config.language as string) || 'python'}
                onChange={(e) => handleConfigChange('language', e.target.value)}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              >
                <option value="python">Python</option>
                <option value="javascript">JavaScript</option>
                <option value="typescript">TypeScript</option>
                <option value="bash">Bash</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Code
              </label>
              <textarea
                value={(config.code as string) || ''}
                onChange={(e) => handleConfigChange('code', e.target.value)}
                placeholder="# Write your code here...\n# Use 'input' variable for input data\nresult = input"
                rows={10}
                className="w-full px-3 py-2 text-sm font-mono border border-gray-200 dark:border-gray-600 rounded-lg bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-white resize-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Timeout (seconds)
              </label>
              <input
                type="number"
                min="1"
                max="300"
                value={(config.timeout as number) || 30}
                onChange={(e) => handleConfigChange('timeout', parseInt(e.target.value))}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
          </div>
        );

      case 'http':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Method
              </label>
              <select
                value={(config.method as string) || 'GET'}
                onChange={(e) => handleConfigChange('method', e.target.value)}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              >
                <option value="GET">GET</option>
                <option value="POST">POST</option>
                <option value="PUT">PUT</option>
                <option value="DELETE">DELETE</option>
                <option value="PATCH">PATCH</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                URL
              </label>
              <input
                type="text"
                value={(config.url as string) || ''}
                onChange={(e) => handleConfigChange('url', e.target.value)}
                placeholder="https://api.example.com/endpoint"
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Headers (JSON)
              </label>
              <textarea
                value={JSON.stringify((config.headers as Record<string, string>) || {}, null, 2)}
                onChange={(e) => {
                  try {
                    const headers = JSON.parse(e.target.value);
                    handleConfigChange('headers', headers);
                  } catch {
                    // Invalid JSON, ignore
                  }
                }}
                placeholder='{"Content-Type": "application/json"}'
                rows={3}
                className="w-full px-3 py-2 text-sm font-mono border border-gray-200 dark:border-gray-600 rounded-lg bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-white resize-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Body
              </label>
              <textarea
                value={(config.body as string) || ''}
                onChange={(e) => handleConfigChange('body', e.target.value)}
                placeholder="Request body... Use {{variable}} for variables"
                rows={4}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
            </div>
          </div>
        );

      case 'condition':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Condition
              </label>
              <input
                type="text"
                value={(config.condition as string) || ''}
                onChange={(e) => handleConfigChange('condition', e.target.value)}
                placeholder="{{value}} > 10"
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
              <p className="text-xs text-gray-500 mt-1">
                Use {'{{variable}}'} to reference variables
              </p>
            </div>
          </div>
        );

      case 'loop':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Items
              </label>
              <input
                type="text"
                value={(config.items as string) || ''}
                onChange={(e) => handleConfigChange('items', e.target.value)}
                placeholder="{{items_array}}"
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
              <p className="text-xs text-gray-500 mt-1">
                Array variable to iterate over
              </p>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Max Iterations
              </label>
              <input
                type="number"
                min="1"
                max="1000"
                value={(config.max_iterations as number) || 100}
                onChange={(e) => handleConfigChange('max_iterations', parseInt(e.target.value))}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
          </div>
        );

      case 'variable':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Operation
              </label>
              <select
                value={(config.operation as string) || 'set'}
                onChange={(e) => handleConfigChange('operation', e.target.value)}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              >
                <option value="set">Set</option>
                <option value="append">Append</option>
                <option value="merge">Merge</option>
                <option value="delete">Delete</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Variable Name
              </label>
              <input
                type="text"
                value={(config.name as string) || ''}
                onChange={(e) => handleConfigChange('name', e.target.value)}
                placeholder="myVariable"
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Value
              </label>
              <textarea
                value={(config.value as string) || ''}
                onChange={(e) => handleConfigChange('value', e.target.value)}
                placeholder="Value or {{variable}}"
                rows={3}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
            </div>
          </div>
        );

      case 'template':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Template
              </label>
              <textarea
                value={(config.template as string) || ''}
                onChange={(e) => handleConfigChange('template', e.target.value)}
                placeholder="Hello {{name}}, your result is {{result}}"
                rows={6}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
              <p className="text-xs text-gray-500 mt-1">
                Use {'{{variable}}'} to insert variable values
              </p>
            </div>
          </div>
        );

      case 'knowledge':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Query
              </label>
              <textarea
                value={(config.query as string) || ''}
                onChange={(e) => handleConfigChange('query', e.target.value)}
                placeholder="Search query... Use {{variable}} for variables"
                rows={3}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Top K Results
              </label>
              <input
                type="number"
                min="1"
                max="20"
                value={(config.top_k as number) || 5}
                onChange={(e) => handleConfigChange('top_k', parseInt(e.target.value))}
                className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
              />
            </div>
          </div>
        );

      default:
        return (
          <div className="text-gray-500 text-sm">
            No configuration available for this node type.
          </div>
        );
    }
  };

  return (
    <>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <Settings className="w-5 h-5 text-gray-600 dark:text-gray-400" />
          <h3 className="font-semibold text-gray-900 dark:text-white">Node Configuration</h3>
        </div>
        <button
          onClick={onClose}
          className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded transition-colors"
        >
          <X className="w-4 h-4 text-gray-500" />
        </button>
      </div>

      {/* Node Info */}
      <div className="px-4 py-3 bg-gray-50 dark:bg-gray-700/50 border-b border-gray-200 dark:border-gray-700">
        <div className="text-sm font-medium text-gray-900 dark:text-white">{String(data.label)}</div>
        <div className="text-xs text-gray-500 dark:text-gray-400 capitalize">{type} node</div>
      </div>

      {/* Configuration Form */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {renderFields()}
      </div>

      {/* Footer Actions */}
      <div className="p-4 border-t border-gray-200 dark:border-gray-700 space-y-2">
        <Button
          variant="danger"
          size="sm"
          onClick={onDelete}
          className="w-full"
        >
          <Trash2 className="w-4 h-4 mr-2" />
          Delete Node
        </Button>
      </div>
    </>
  );
};
