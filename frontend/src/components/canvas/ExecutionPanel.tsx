import React, { useState, useEffect, useCallback } from 'react';
import {
  Play,
  X,
  CheckCircle,
  XCircle,
  Loader2,
  Clock,
  Terminal,
  Variable,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { Button } from '../ui/Button';
import { executionApi } from '../../services/workflowApi';
import type { Node, Edge } from '@xyflow/react';
import type { ExecutionEvent } from '../../types/workflow';

interface ExecutionPanelProps {
  nodes: Node[];
  edges: Edge[];
  workflowId?: string;
  workflowName: string;
  onClose: () => void;
  onNodeStatusUpdate: (nodeId: string, status: string, error?: string) => void;
}

export const ExecutionPanel: React.FC<ExecutionPanelProps> = ({
  nodes,
  edges,
  workflowId,
  workflowName,
  onClose,
  onNodeStatusUpdate,
}) => {
  const [isExecuting, setIsExecuting] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const [inputs, setInputs] = useState<Record<string, unknown>>({});
  const [outputs, setOutputs] = useState<Record<string, unknown> | null>(null);
  const [executionStatus, setExecutionStatus] = useState<'idle' | 'running' | 'completed' | 'failed'>('idle');
  const [expandedLogs, setExpandedLogs] = useState(true);

  // Get input variables from start node
  const startNode = nodes.find((n) => n.type === 'start');
  const inputVariables = ((startNode?.data as any)?.config?.variables as Array<{
    name: string;
    type: string;
    required?: boolean;
    default?: unknown;
  }>) || [];

  const handleInputChange = (name: string, value: unknown) => {
    setInputs((prev) => ({ ...prev, [name]: value }));
  };

  const addLog = useCallback((message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [...prev, `[${timestamp}] ${message}`]);
  }, []);

  const handleExecute = async () => {
    // Reset state
    setLogs([]);
    setOutputs(null);
    setExecutionStatus('running');
    setIsExecuting(true);

    // Reset all node statuses
    nodes.forEach((node) => {
      onNodeStatusUpdate(node.id, '');
    });

    addLog('Starting workflow execution...');

    // Check if we have a backend workflow
    if (workflowId) {
      // Execute via backend API
      const cleanup = executionApi.runWorkflowStream(
        workflowId,
        { inputs },
        (event: ExecutionEvent) => {
          handleExecutionEvent(event);
        },
        (error) => {
          console.error('Execution error:', error);
          addLog(`Error: ${error.message}`);
          setExecutionStatus('failed');
          setIsExecuting(false);
        }
      );

      // Store cleanup for later
      return () => cleanup();
    } else {
      // Simulate execution for unsaved workflows
      await simulateExecution();
    }
  };

  const handleExecutionEvent = (event: ExecutionEvent) => {
    switch (event.event_type) {
      case 'execution_started':
        addLog('Execution started');
        break;

      case 'node_started':
        addLog(`Node "${event.node_id}" (${event.node_type}) started`);
        onNodeStatusUpdate(event.node_id as string, 'running');
        break;

      case 'node_completed':
        addLog(`Node "${event.node_id}" completed`);
        onNodeStatusUpdate(event.node_id as string, 'completed');
        break;

      case 'node_failed':
        addLog(`Node "${event.node_id}" failed: ${event.error}`);
        onNodeStatusUpdate(event.node_id as string, 'failed', event.error as string);
        break;

      case 'execution_completed':
        addLog('Execution completed successfully');
        setOutputs(event.outputs as Record<string, unknown>);
        setExecutionStatus('completed');
        setIsExecuting(false);
        break;

      case 'execution_failed':
        addLog(`Execution failed: ${event.error}`);
        setExecutionStatus('failed');
        setIsExecuting(false);
        break;

      case 'variable_updated':
        addLog(`Variable "${event.variable}" updated`);
        break;
    }
  };

  const simulateExecution = async () => {
    // Simple simulation for demo
    const sortedNodes = [...nodes].sort((a, b) => a.position.y - b.position.y);

    for (const node of sortedNodes) {
      addLog(`Executing ${node.type} node: ${node.data.label}...`);
      onNodeStatusUpdate(node.id, 'running');

      // Simulate processing time
      await new Promise((resolve) => setTimeout(resolve, 800));

      addLog(`✓ ${node.type} node completed`);
      onNodeStatusUpdate(node.id, 'completed');
    }

    addLog('Workflow execution completed!');
    setExecutionStatus('completed');
    setIsExecuting(false);
  };

  const getStatusIcon = () => {
    switch (executionStatus) {
      case 'completed':
        return <CheckCircle className="w-5 h-5 text-green-500" />;
      case 'failed':
        return <XCircle className="w-5 h-5 text-red-500" />;
      case 'running':
        return <Loader2 className="w-5 h-5 animate-spin text-yellow-500" />;
      default:
        return <Clock className="w-5 h-5 text-gray-400" />;
    }
  };

  return (
    <>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <Terminal className="w-5 h-5 text-blue-500" />
          <h3 className="font-semibold text-gray-900 dark:text-white">Execution</h3>
          {executionStatus !== 'idle' && getStatusIcon()}
        </div>
        <button
          onClick={onClose}
          className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded transition-colors"
        >
          <X className="w-4 h-4 text-gray-500" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {/* Input Variables Section */}
        {inputVariables.length > 0 && (
          <div className="p-4 border-b border-gray-200 dark:border-gray-700">
            <div className="flex items-center gap-2 mb-3">
              <Variable className="w-4 h-4 text-gray-500" />
              <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                Input Variables
              </h4>
            </div>
            <div className="space-y-3">
              {inputVariables.map((variable) => (
                <div key={variable.name}>
                  <label className="block text-xs text-gray-500 dark:text-gray-400 mb-1">
                    {variable.name}
                    {variable.required && <span className="text-red-500 ml-1">*</span>}
                    <span className="text-gray-400 ml-1">({variable.type})</span>
                  </label>
                  {variable.type === 'boolean' ? (
                    <select
                      value={String(inputs[variable.name] ?? variable.default ?? false)}
                      onChange={(e) =>
                        handleInputChange(variable.name, e.target.value === 'true')
                      }
                      className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                    >
                      <option value="true">True</option>
                      <option value="false">False</option>
                    </select>
                  ) : variable.type === 'number' ? (
                    <input
                      type="number"
                      value={String(inputs[variable.name] ?? variable.default ?? '')}
                      onChange={(e) =>
                        handleInputChange(variable.name, parseFloat(e.target.value))
                      }
                      className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                    />
                  ) : (
                    <input
                      type="text"
                      value={String(inputs[variable.name] ?? variable.default ?? '')}
                      onChange={(e) => handleInputChange(variable.name, e.target.value)}
                      placeholder={`Enter ${variable.name}...`}
                      className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                    />
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Execute Button */}
        <div className="p-4 border-b border-gray-200 dark:border-gray-700">
          <Button
            onClick={handleExecute}
            disabled={isExecuting}
            className="w-full"
            variant={executionStatus === 'failed' ? 'danger' : 'primary'}
          >
            {isExecuting ? (
              <>
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                Running...
              </>
            ) : executionStatus === 'completed' ? (
              <>
                <Play className="w-4 h-4 mr-2" />
                Run Again
              </>
            ) : (
              <>
                <Play className="w-4 h-4 mr-2" />
                Execute Workflow
              </>
            )}
          </Button>
        </div>

        {/* Logs Section */}
        {logs.length > 0 && (
          <div className="p-4">
            <button
              onClick={() => setExpandedLogs(!expandedLogs)}
              className="flex items-center justify-between w-full mb-2"
            >
              <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 flex items-center gap-2">
                <Terminal className="w-4 h-4" />
                Execution Logs
              </h4>
              {expandedLogs ? (
                <ChevronUp className="w-4 h-4 text-gray-400" />
              ) : (
                <ChevronDown className="w-4 h-4 text-gray-400" />
              )}
            </button>
            {expandedLogs && (
              <div className="bg-gray-900 rounded-lg p-3 max-h-64 overflow-y-auto">
                {logs.map((log, index) => (
                  <div
                    key={index}
                    className={`text-xs font-mono mb-1 ${
                      log.includes('Error') || log.includes('failed')
                        ? 'text-red-400'
                        : log.includes('completed') || log.includes('✓')
                        ? 'text-green-400'
                        : 'text-gray-300'
                    }`}
                  >
                    {log}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Outputs Section */}
        {outputs && (
          <div className="p-4 border-t border-gray-200 dark:border-gray-700">
            <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Output
            </h4>
            <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-3">
              <pre className="text-xs text-green-800 dark:text-green-200 overflow-x-auto">
                {JSON.stringify(outputs, null, 2)}
              </pre>
            </div>
          </div>
        )}
      </div>
    </>
  );
};
