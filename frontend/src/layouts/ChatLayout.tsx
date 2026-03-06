import React from 'react';
import { SessionList } from '../components/chat/SessionList';
import { ChatWindow } from '../components/chat/ChatWindow';
import { useChatStore } from '../store/chatStore';

export const ChatLayout: React.FC = () => {
  const bootstrapSessions = useChatStore((s) => s.bootstrapSessions);

  React.useEffect(() => {
    bootstrapSessions();
  }, [bootstrapSessions]);

  return (
    <div
      className="grid h-full w-full overflow-hidden"
      style={{ gridTemplateColumns: '320px minmax(0, 1fr)' }}
    >
      <aside className="h-full overflow-hidden border-r border-zinc-200 dark:border-zinc-800 bg-zinc-950">
        <SessionList className="h-full w-full border-r-0" />
      </aside>
      <section className="min-w-0 h-full overflow-hidden">
        <ChatWindow />
      </section>
    </div>
  );
};
