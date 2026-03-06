import React from 'react';
import { Search } from 'lucide-react';

interface GraphFiltersProps {
    statusFilter: string;
    setStatusFilter: (status: string) => void;
    searchQuery: string;
    setSearchQuery: (query: string) => void;
    onPageChange: (page: number) => void;
}

export const GraphFilters: React.FC<GraphFiltersProps> = ({
    statusFilter,
    setStatusFilter,
    searchQuery,
    setSearchQuery,
    onPageChange
}) => {
    return (
        <div className="flex gap-2 items-center">
            <div className="relative">
                <Search className="absolute left-2 top-1/2 transform -translate-y-1/2 text-gray-400" size={16} />
                <input 
                    type="text" 
                    placeholder="Search tasks..." 
                    value={searchQuery}
                    onChange={(e) => { setSearchQuery(e.target.value); onPageChange(1); }}
                    className="pl-8 pr-3 py-1.5 text-sm border rounded-md dark:bg-gray-800 dark:border-gray-700"
                />
            </div>

            <div className="flex gap-2 bg-gray-100 dark:bg-gray-800 p-1 rounded-lg">
                {['Active', 'All', 'Completed'].map(status => (
                    <button
                        key={status}
                        onClick={() => { setStatusFilter(status); onPageChange(1); }}
                        className={`px-3 py-1 rounded-md text-sm font-medium transition-colors ${
                            statusFilter === status 
                                ? 'bg-white dark:bg-gray-700 shadow text-gray-900 dark:text-gray-100' 
                                : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
                        }`}
                    >
                        {status}
                    </button>
                ))}
            </div>
        </div>
    );
};
