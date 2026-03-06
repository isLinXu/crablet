import React from 'react';

interface PaginationProps {
    page: number;
    totalPages: number;
    setPage: (page: number | ((p: number) => number)) => void;
}

export const Pagination: React.FC<PaginationProps> = ({ page, totalPages, setPage }) => {
    if (totalPages <= 1) return null;

    return (
        <div className="flex justify-center gap-2 mt-4">
            <button 
                disabled={page === 1}
                onClick={() => setPage(p => Math.max(1, p - 1))}
                className="px-3 py-1 rounded bg-gray-100 dark:bg-gray-800 disabled:opacity-50"
            >
                Previous
            </button>
            <span className="px-3 py-1 text-sm text-gray-600 dark:text-gray-400">
                Page {page} of {totalPages}
            </span>
            <button 
                disabled={page === totalPages}
                onClick={() => setPage(p => Math.min(totalPages, p + 1))}
                className="px-3 py-1 rounded bg-gray-100 dark:bg-gray-800 disabled:opacity-50"
            >
                Next
            </button>
        </div>
    );
};
