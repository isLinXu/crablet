import React, { useState } from 'react';
import { SessionList } from '../components/chat/SessionList';
import { ChatWindow } from '../components/chat/ChatWindow';
import { useChatStore } from '../store/chatStore';
import { Menu, X } from 'lucide-react';
import clsx from 'clsx';

export const ChatLayout: React.FC = () => {
  const bootstrapSessions = useChatStore((s) => s.bootstrapSessions);
  const [showSessions, setShowSessions] = useState(false);

  React.useEffect(() => {
    bootstrapSessions();
  }, [bootstrapSessions]);

  return (
    <div className="flex h-full w-full overflow-hidden relative">
      {/* Session List Sidebar */}
      <aside className={clsx(
        "absolute z-20 h-full w-80 bg-zinc-950 border-r border-zinc-800 transition-transform duration-200 ease-in-out md:relative md:translate-x-0",
        showSessions ? "translate-x-0" : "-translate-x-full"
      )}>
        <div className="h-full flex flex-col">
            {/* Mobile Close Button inside Sidebar */}
            <div className="md:hidden p-4 flex justify-end border-b border-zinc-800">
                <button onClick={() => setShowSessions(false)} className="text-zinc-400 hover:text-white">
                    <X size={20} />
                </button>
            </div>
            <SessionList className="flex-1 w-full border-r-0" onSelect={() => setShowSessions(false)} />
        </div>
      </aside>

      {/* Main Chat Window */}
      <section className="flex-1 flex flex-col min-w-0 h-full overflow-hidden w-full relative">
        {/* Mobile Header for Chat */}
        <div className="md:hidden h-14 flex items-center px-4 border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shrink-0 z-10">
            <button 
                onClick={() => setShowSessions(true)} 
                className="p-2 -ml-2 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-lg"
            >
                <Menu size={20} />
            </button>
            <span className="font-semibold ml-3 text-zinc-900 dark:text-zinc-100">Chats</span>
        </div>
        
        <div className="flex-1 overflow-hidden relative">
            <ChatWindow />
        </div>
      </section>
      
      {/* Mobile Overlay */}
      {showSessions && (
        <div 
            className="absolute inset-0 z-10 bg-black/50 md:hidden backdrop-blur-sm"
            onClick={() => setShowSessions(false)}
        />
      )}
    </div>
  );
};
