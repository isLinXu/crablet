import React, { useEffect, useRef, useState } from 'react';
import { Card } from '../ui/Card';
import { MessageSquare, ArrowRight, Filter, X } from 'lucide-react';

export interface SwarmMessage {
    graphId: string;
    taskId: string;
    from: string;
    to: string;
    type: string;
    content: string;
    timestamp: number;
}

interface SwarmActivityFeedProps {
    messages: SwarmMessage[];
}

export const SwarmActivityFeed: React.FC<SwarmActivityFeedProps> = ({ messages }) => {
    const bottomRef = useRef<HTMLDivElement>(null);
    const [filterType, setFilterType] = useState<string | null>(null);
    const [isFilterOpen, setIsFilterOpen] = useState(false);

    const filteredMessages = filterType 
        ? messages.filter(m => m.type === filterType)
        : messages;

    useEffect(() => {
        if (bottomRef.current) {
            bottomRef.current.scrollIntoView({ behavior: 'smooth' });
        }
    }, [messages, filterType]); // Scroll on new messages or filter change

    return (
        <Card className="h-[400px] flex flex-col p-0 overflow-hidden relative">
            <div className="p-3 border-b bg-gray-50 dark:bg-gray-800 flex justify-between items-center z-10">
                <h3 className="font-semibold text-sm flex items-center gap-2">
                    <MessageSquare size={16} />
                    Live Activity
                </h3>
                <div className="flex items-center gap-2">
                    {filterType && (
                        <button 
                            onClick={() => setFilterType(null)}
                            className="text-[10px] flex items-center gap-1 px-1.5 py-0.5 bg-blue-100 text-blue-700 rounded hover:bg-blue-200"
                        >
                            {filterType} <X size={10} />
                        </button>
                    )}
                    <div className="relative">
                         <button 
                            onClick={() => setIsFilterOpen(!isFilterOpen)}
                            className={`p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 ${isFilterOpen ? 'bg-gray-200 dark:bg-gray-700' : ''}`}
                            title="Filter by type"
                         >
                             <Filter size={14} className="text-gray-500" />
                         </button>
                         {isFilterOpen && (
                             <div className="absolute right-0 top-full mt-1 bg-white dark:bg-gray-800 border dark:border-gray-700 shadow-lg rounded-md p-1 z-20 w-24">
                                 {['Task', 'Result', 'Error', 'Status'].map(type => (
                                     <button
                                         key={type}
                                         onClick={() => { setFilterType(type); setIsFilterOpen(false); }}
                                         className={`block w-full text-left px-2 py-1.5 text-xs hover:bg-gray-100 dark:hover:bg-gray-700 rounded ${filterType === type ? 'bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400' : 'text-gray-700 dark:text-gray-300'}`}
                                     >
                                         {type}
                                     </button>
                                 ))}
                                 <button
                                     onClick={() => { setFilterType(null); setIsFilterOpen(false); }}
                                     className="block w-full text-left px-2 py-1.5 text-xs hover:bg-gray-100 dark:hover:bg-gray-700 rounded text-gray-500 border-t dark:border-gray-700 mt-1"
                                 >
                                     Clear Filter
                                 </button>
                             </div>
                         )}
                    </div>
                    <span className="text-[10px] text-gray-400 min-w-[3ch] text-right">{filteredMessages.length}</span>
                </div>
            </div>
            <div className="flex-1 overflow-y-auto p-4 space-y-3 bg-white dark:bg-gray-900">
                {filteredMessages.length === 0 ? (
                    <div className="text-center text-gray-400 text-sm mt-10">
                        {messages.length > 0 ? 'No matching events.' : 'No activity yet.'}
                    </div>
                ) : (
                    filteredMessages.map((msg, idx) => (
                        <div key={idx} className="flex flex-col gap-1 animate-fadeIn">
                            <div className="flex items-center gap-2 text-[10px] text-gray-500 uppercase tracking-wider">
                                <span className="font-bold text-blue-600 dark:text-blue-400 truncate max-w-[80px]">{msg.from}</span>
                                <ArrowRight size={10} />
                                <span className="font-bold text-green-600 dark:text-green-400 truncate max-w-[80px]">{msg.to}</span>
                                <span className="ml-auto opacity-50 normal-case">
                                    {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                                </span>
                            </div>
                            <div className={`p-2 rounded-md text-sm border shadow-sm ${
                                msg.type === 'Error' ? 'bg-red-50 border-red-100 text-red-800 dark:bg-red-900/20 dark:border-red-900/50 dark:text-red-200' :
                                msg.type === 'Result' ? 'bg-green-50 border-green-100 text-green-800 dark:bg-green-900/20 dark:border-green-900/50 dark:text-green-200' :
                                msg.type === 'Task' ? 'bg-blue-50 border-blue-100 text-blue-800 dark:bg-blue-900/20 dark:border-blue-900/50 dark:text-blue-200' :
                                'bg-gray-50 border-gray-100 text-gray-800 dark:bg-gray-800 dark:border-gray-700 dark:text-gray-200'
                            }`}>
                                {msg.type !== 'Task' && msg.type !== 'Result' && <span className="font-bold mr-1 text-xs uppercase opacity-70">[{msg.type}]</span>}
                                {msg.content}
                            </div>
                        </div>
                    ))
                )}
                <div ref={bottomRef} />
            </div>
        </Card>
    );
};
