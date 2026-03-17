import React from 'react';
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
} from 'lucide-react';
import type { NodeTypeDefinition } from '../../types/workflow';

interface NodeTypePanelProps {
  nodeTypes: NodeTypeDefinition[];
  onAddNode: (type: string) => void;
}

const iconMap: Record<string, React.ReactNode> = {
  Play: <Play className="w-4 h-4" />,
  Square: <Square className="w-4 h-4" />,
  Brain: <Brain className="w-4 h-4" />,
  Bot: <Bot className="w-4 h-4" />,
  Code: <Code className="w-4 h-4" />,
  Globe: <Globe className="w-4 h-4" />,
  GitBranch: <GitBranch className="w-4 h-4" />,
  Repeat: <Repeat className="w-4 h-4" />,
  Database: <Database className="w-4 h-4" />,
  FileText: <FileText className="w-4 h-4" />,
  Book: <Book className="w-4 h-4" />,
};

const categoryOrder = ['control', 'ai', 'processing', 'integration', 'data'];
const categoryLabels: Record<string, string> = {
  control: 'Control Flow',
  ai: 'AI & Agents',
  processing: 'Processing',
  integration: 'Integration',
  data: 'Data',
};

export const NodeTypePanel: React.FC<NodeTypePanelProps> = ({
  nodeTypes,
  onAddNode,
}) => {
  // Group node types by category
  const groupedTypes = nodeTypes.reduce((acc, type) => {
    const category = type.category || 'other';
    if (!acc[category]) acc[category] = [];
    acc[category].push(type);
    return acc;
  }, {} as Record<string, NodeTypeDefinition[]>);

  const handleDragStart = (e: React.DragEvent, nodeType: string) => {
    e.dataTransfer.setData('application/reactflow', nodeType);
    e.dataTransfer.effectAllowed = 'move';
  };

  return (
    <div className="flex-1 overflow-y-auto p-3 space-y-4">
      {categoryOrder.map(
        (category) =>
          groupedTypes[category]?.length > 0 && (
            <div key={category}>
              <h3 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2 px-1">
                {categoryLabels[category] || category}
              </h3>
              <div className="space-y-1">
                {groupedTypes[category].map((type) => (
                  <div
                    key={type.type}
                    draggable
                    onDragStart={(e) => handleDragStart(e, type.type)}
                    onClick={() => onAddNode(type.type)}
                    className="flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors group"
                  >
                    <div
                      className="w-8 h-8 rounded-lg flex items-center justify-center text-white"
                      style={{ backgroundColor: type.color || '#6366f1' }}
                    >
                      {iconMap[type.icon] || <div className="w-4 h-4" />}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-gray-900 dark:text-white truncate">
                        {type.name}
                      </div>
                      <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
                        {type.description}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )
      )}
    </div>
  );
};
