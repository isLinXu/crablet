import { Routes, Route, Navigate } from 'react-router-dom';
import { Toaster } from 'react-hot-toast';
import { useEffect } from 'react';
import { useModelStore } from '@/store/modelStore';
import { ErrorBoundary } from './components/ErrorBoundary';
import { Sidebar } from './components/sidebar/Sidebar';
import { Dashboard } from './components/dashboard/Dashboard';
import { SkillBrowser } from './components/sidebar/SkillBrowser';
import { KnowledgeBase } from './components/knowledge/KnowledgeBase';
import { SettingsPanel } from './components/settings/SettingsPanel';
import { Canvas } from './components/canvas/Canvas';
import { ChatLayout } from './layouts/ChatLayout';
import { ActivityCenter } from './components/activity/ActivityCenter';
import { McpCenter } from './components/mcp/McpCenter';
import { MemoryCenter } from './components/memory/MemoryCenter';

function App() {
  const syncModels = useModelStore((s) => s.syncFromBackend);
  useEffect(() => {
    syncModels();
  }, [syncModels]);

  return (
    <ErrorBoundary>
        <div className="flex h-screen w-full bg-zinc-50 dark:bg-zinc-950 transition-colors duration-200">
          <Sidebar />
          
          <main className="flex-1 overflow-hidden bg-white dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100 relative">
            <Routes>
              <Route path="/" element={<Navigate to="/chat" replace />} />
              <Route path="/chat" element={<ChatLayout />} />
              <Route path="/dashboard" element={<Dashboard />} />
              <Route path="/canvas" element={<Canvas />} />
              <Route path="/skills" element={<SkillBrowser />} />
              <Route path="/activity" element={<ActivityCenter />} />
              <Route path="/mcp" element={<McpCenter />} />
              <Route path="/memory" element={<MemoryCenter />} />
              <Route path="/knowledge" element={<KnowledgeBase />} />
              <Route path="/settings" element={<SettingsPanel />} />
            </Routes>
          </main>
          <Toaster position="top-right" toastOptions={{
            className: 'dark:bg-zinc-800 dark:text-white',
          }} />
        </div>
    </ErrorBoundary>
  );
}

export default App;
