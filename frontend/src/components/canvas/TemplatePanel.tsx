/**
 * Workflow Template Panel
 * Browse and select from pre-built workflow templates
 */

import React, { useState } from 'react';
import { workflowTemplates, getTemplateCategories, type WorkflowTemplate } from '@/utils/workflowTemplates';
import { Brain, Database, Bot, Book, Repeat, Globe, X, LayoutTemplate } from 'lucide-react';

interface TemplatePanelProps {
  onSelectTemplate: (template: WorkflowTemplate) => void;
  onClose: () => void;
}

const iconMap: Record<string, React.ReactNode> = {
  Brain: <Brain className="w-6 h-6" />,
  Database: <Database className="w-6 h-6" />,
  Bot: <Bot className="w-6 h-6" />,
  Book: <Book className="w-6 h-6" />,
  Repeat: <Repeat className="w-6 h-6" />,
  Globe: <Globe className="w-6 h-6" />,
};

export const TemplatePanel: React.FC<TemplatePanelProps> = ({
  onSelectTemplate,
  onClose,
}) => {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const categories = getTemplateCategories();

  const filteredTemplates = selectedCategory
    ? workflowTemplates.filter((t) => t.category === selectedCategory)
    : workflowTemplates;

  return (
    <div className="h-full flex flex-col bg-white dark:bg-gray-800">
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <h2 className="font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <LayoutTemplate className="w-5 h-5" />
          Templates
        </h2>
        <button
          onClick={onClose}
          className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
        >
          <X className="w-5 h-5 text-gray-500" />
        </button>
      </div>

      {/* Category Filter */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex flex-wrap gap-2">
          <button
            onClick={() => setSelectedCategory(null)}
            className={`px-3 py-1.5 text-sm rounded-full transition-colors ${
              selectedCategory === null
                ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400'
                : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400 hover:bg-gray-200'
            }`}
          >
            All
          </button>
          {categories.map((category) => (
            <button
              key={category}
              onClick={() => setSelectedCategory(category)}
              className={`px-3 py-1.5 text-sm rounded-full transition-colors ${
                selectedCategory === category
                  ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400'
                  : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400 hover:bg-gray-200'
              }`}
            >
              {category}
            </button>
          ))}
        </div>
      </div>

      {/* Template List */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-3">
          {filteredTemplates.map((template) => (
            <button
              key={template.id}
              onClick={() => onSelectTemplate(template)}
              className="w-full text-left p-4 bg-gray-50 dark:bg-gray-700/50 rounded-xl hover:bg-gray-100 dark:hover:bg-gray-700 transition-all group"
            >
              <div className="flex items-start gap-3">
                <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center text-white shadow-lg group-hover:shadow-xl transition-shadow">
                  {iconMap[template.icon] || <LayoutTemplate className="w-6 h-6" />}
                </div>
                <div className="flex-1 min-w-0">
                  <h3 className="font-semibold text-gray-900 dark:text-white truncate">
                    {template.name}
                  </h3>
                  <p className="text-sm text-gray-500 dark:text-gray-400 mt-1 line-clamp-2">
                    {template.description}
                  </p>
                  <span className="inline-block mt-2 text-xs px-2 py-1 bg-gray-200 dark:bg-gray-600 rounded-full text-gray-600 dark:text-gray-300">
                    {template.category}
                  </span>
                </div>
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-gray-200 dark:border-gray-700 text-center text-sm text-gray-500">
        {filteredTemplates.length} template{filteredTemplates.length !== 1 ? 's' : ''} available
      </div>
    </div>
  );
};
