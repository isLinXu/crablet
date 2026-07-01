import { Button } from '../ui/Button';
import {
  ChevronLeft,
  ChevronRight,
  Search,
  LayoutTemplate,
  FolderOpen,
  Download,
  Brain,
  Trash2,
  Terminal,
  Loader2,
  Save,
  Grid3X3,
  AlignVerticalJustifyCenter,
  Network,
} from 'lucide-react';
import { ModelSelectorCompact } from './ModelSelectorCompact';

interface CanvasToolbarProps {
  workflowName: string;
  setWorkflowName: (v: string) => void;
  searchQuery: string;
  setSearchQuery: (v: string) => void;
  showNodePanel: boolean;
  onToggleNodePanel: () => void;
  onShowTemplatePanel: () => void;
  onImportClick: () => void;
  onExport: () => void;
  hasNodes: boolean;
  aiNodesCount: number;
  onBatchUpdateModel: (modelId: string, modelProvider?: string, modelVendor?: string) => void;
  layoutMode: 'auto' | 'hierarchical' | 'tree';
  onLayout: (mode: 'auto' | 'hierarchical' | 'tree') => void;
  onClear: () => void;
  showExecutionPanel: boolean;
  onToggleExecutionPanel: () => void;
  isSaving: boolean;
  onSave: () => void;
}

export const CanvasToolbar = ({
  workflowName,
  setWorkflowName,
  searchQuery,
  setSearchQuery,
  showNodePanel,
  onToggleNodePanel,
  onShowTemplatePanel,
  onImportClick,
  onExport,
  hasNodes,
  aiNodesCount,
  onBatchUpdateModel,
  layoutMode,
  onLayout,
  onClear,
  showExecutionPanel,
  onToggleExecutionPanel,
  isSaving,
  onSave,
}: CanvasToolbarProps) => {
  return (
    <div className="flex items-center justify-between px-4 py-3 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
      <div className="flex items-center gap-3">
        <button
          onClick={onToggleNodePanel}
          className="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
          title="Toggle Node Panel"
        >
          {showNodePanel ? (
            <ChevronLeft className="w-5 h-5 text-gray-600" />
          ) : (
            <ChevronRight className="w-5 h-5 text-gray-600" />
          )}
        </button>
        <input
          type="text"
          value={workflowName}
          onChange={(e) => setWorkflowName(e.target.value)}
          className="text-lg font-semibold bg-transparent border-none focus:outline-none focus:ring-2 focus:ring-blue-500 rounded px-2 text-gray-900 dark:text-white"
          placeholder="Workflow Name"
        />
      </div>

      <div className="flex items-center gap-2">
        {/* Search Box */}
        <div className="relative">
          <Search className="w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
          <input
            type="text"
            placeholder="Search nodes..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-9 pr-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 w-40"
          />
        </div>

        <div className="w-px h-6 bg-gray-300 dark:bg-gray-600" />

        <Button size="sm" variant="secondary" onClick={onShowTemplatePanel} title="Templates">
          <LayoutTemplate className="w-4 h-4 mr-1" />
          Templates
        </Button>
        <Button size="sm" variant="secondary" onClick={onImportClick} title="Import Workflow">
          <FolderOpen className="w-4 h-4 mr-1" />
          Import
        </Button>
        <Button size="sm" variant="secondary" onClick={onExport} disabled={!hasNodes} title="Export Workflow">
          <Download className="w-4 h-4 mr-1" />
          Export
        </Button>

        <div className="w-px h-6 bg-gray-300 dark:bg-gray-600" />

        {/* Batch Model Update Dropdown */}
        {aiNodesCount > 0 && (
          <div className="relative group">
            <Button size="sm" variant="secondary" title={`Update model for ${aiNodesCount} AI nodes`}>
              <Brain className="w-4 h-4 mr-1" />
              Set Model
            </Button>
            <div className="absolute right-0 top-full mt-1 hidden group-hover:block bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 min-w-[200px] p-2">
              <p className="text-xs text-gray-500 px-2 py-1 border-b border-gray-200 dark:border-gray-700 mb-2">
                Update all AI nodes ({aiNodesCount})
              </p>
              <ModelSelectorCompact
                value=""
                onChange={(model, provider) => {
                  if (provider) {
                    onBatchUpdateModel(model, provider.id, provider.vendor);
                  }
                }}
                placeholder="Select model..."
              />
            </div>
          </div>
        )}

        {/* Layout Dropdown */}
        <div className="relative group">
          <Button size="sm" variant="secondary" title="Layout Options">
            <LayoutTemplate className="w-4 h-4 mr-1" />
            Layout
          </Button>
          <div className="absolute right-0 top-full mt-1 hidden group-hover:block bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 min-w-[140px]">
            <button
              onClick={() => onLayout('auto')}
              className={`w-full px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 first:rounded-t-lg flex items-center gap-2 ${layoutMode === 'auto' ? 'bg-blue-50 text-blue-600' : ''}`}
            >
              <Grid3X3 className="w-4 h-4" />
              Auto
            </button>
            <button
              onClick={() => onLayout('hierarchical')}
              className={`w-full px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2 ${layoutMode === 'hierarchical' ? 'bg-blue-50 text-blue-600' : ''}`}
            >
              <AlignVerticalJustifyCenter className="w-4 h-4" />
              Hierarchical
            </button>
            <button
              onClick={() => onLayout('tree')}
              className={`w-full px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 last:rounded-b-lg flex items-center gap-2 ${layoutMode === 'tree' ? 'bg-blue-50 text-blue-600' : ''}`}
            >
              <Network className="w-4 h-4" />
              Tree
            </button>
          </div>
        </div>

        <Button size="sm" variant="secondary" onClick={onClear} title="Clear Canvas">
          <Trash2 className="w-4 h-4 mr-1" />
          Clear
        </Button>
        <Button
          size="sm"
          variant="secondary"
          onClick={onToggleExecutionPanel}
          className={showExecutionPanel ? 'bg-blue-100 text-blue-700' : ''}
        >
          <Terminal className="w-4 h-4 mr-1" />
          {showExecutionPanel ? 'Hide' : 'Run'}
        </Button>
        <Button size="sm" onClick={onSave} disabled={isSaving}>
          {isSaving ? (
            <Loader2 className="w-4 h-4 mr-1 animate-spin" />
          ) : (
            <Save className="w-4 h-4 mr-1" />
          )}
          Save
        </Button>
      </div>
    </div>
  );
};
