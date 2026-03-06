import React from 'react';
import { Pause, Play, Trash2 } from 'lucide-react';

interface BatchActionsProps {
    selectedCount: number;
    onAction: (action: 'pause' | 'resume' | 'delete') => void;
    onClear: () => void;
}

export const BatchActions: React.FC<BatchActionsProps> = ({ selectedCount, onAction, onClear }) => {
    if (selectedCount === 0) return null;

    return (
        <div className="flex items-center gap-4 p-2 bg-blue-50 dark:bg-blue-900/20 rounded-lg border border-blue-100 dark:border-blue-800">
            <span className="text-sm font-medium text-blue-800 dark:text-blue-200 ml-2">
                {selectedCount} selected
            </span>
            <div className="h-4 w-px bg-blue-200 dark:bg-blue-700" />
            <button onClick={() => onAction('pause')} className="text-sm text-gray-700 dark:text-gray-300 hover:text-blue-600 flex items-center gap-1">
                <Pause size={14} /> Pause
            </button>
            <button onClick={() => onAction('resume')} className="text-sm text-gray-700 dark:text-gray-300 hover:text-blue-600 flex items-center gap-1">
                <Play size={14} /> Resume
            </button>
            <button onClick={() => onAction('delete')} className="text-sm text-red-600 hover:text-red-700 flex items-center gap-1">
                <Trash2 size={14} /> Delete
            </button>
            <div className="flex-1" />
            <button onClick={onClear} className="text-xs text-gray-500 hover:text-gray-700 mr-2">
                Clear
            </button>
        </div>
    );
};
