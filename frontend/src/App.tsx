import { Routes, Route, Navigate } from 'react-router-dom';
import { Toaster } from 'react-hot-toast';
import { useEffect, lazy, Suspense, useCallback, useRef } from 'react';
import { useModelStore } from '@/store/modelStore';
import { ErrorBoundary } from './components/ErrorBoundary';
import { Sidebar } from './components/sidebar/Sidebar';

const routeLoaders = {
  '/chat': () => import('./layouts/ChatLayout'),
  '/dashboard': () => import('./components/dashboard/Dashboard'),
  '/canvas': () => import('./components/canvas/Canvas'),
  '/skills': () => import('./components/sidebar/SkillBrowser'),
  '/activity': () => import('./components/activity/ActivityCenter'),
  '/mcp': () => import('./components/mcp/McpCenter'),
  '/memory': () => import('./components/memory/MemoryCenter'),
  '/knowledge': () => import('./components/knowledge/KnowledgeBase'),
  '/settings': () => import('./components/settings/SettingsPanel'),
} as const;

const ChatLayout = lazy(() =>
  routeLoaders['/chat']().then((m) => ({ default: m.ChatLayout }))
);
const Dashboard = lazy(() =>
  routeLoaders['/dashboard']().then((m) => ({ default: m.Dashboard }))
);
const Canvas = lazy(() =>
  routeLoaders['/canvas']().then((m) => ({ default: m.Canvas }))
);
const SkillBrowser = lazy(() =>
  routeLoaders['/skills']().then((m) => ({ default: m.SkillBrowser }))
);
const ActivityCenter = lazy(() =>
  routeLoaders['/activity']().then((m) => ({ default: m.ActivityCenter }))
);
const McpCenter = lazy(() =>
  routeLoaders['/mcp']().then((m) => ({ default: m.McpCenter }))
);
const MemoryCenter = lazy(() =>
  routeLoaders['/memory']().then((m) => ({ default: m.MemoryCenter }))
);
const KnowledgeBase = lazy(() =>
  routeLoaders['/knowledge']().then((m) => ({ default: m.KnowledgeBase }))
);
const SettingsPanel = lazy(() =>
  routeLoaders['/settings']().then((m) => ({ default: m.SettingsPanel }))
);

function App() {
  const syncModels = useModelStore((s) => s.syncFromBackend);
  const preloadedRoutes = useRef(new Set<string>());

  const preloadRoute = useCallback((path: string) => {
    if (preloadedRoutes.current.has(path)) {
      return;
    }
    const loader = routeLoaders[path as keyof typeof routeLoaders];
    if (!loader) {
      return;
    }
    preloadedRoutes.current.add(path);
    void loader();
  }, []);

  useEffect(() => {
    syncModels();
    preloadRoute('/chat');
    preloadRoute('/dashboard');
  }, [syncModels]);

  return (
    <ErrorBoundary>
        <div className="flex h-screen w-full bg-zinc-50 dark:bg-zinc-950 transition-colors duration-200">
          <Sidebar onRouteIntent={preloadRoute} />
          
          <main className="flex-1 overflow-hidden bg-white dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100 relative">
            <Suspense fallback={<div className="h-full w-full bg-white dark:bg-zinc-900" />}>
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
            </Suspense>
          </main>
          <Toaster position="top-right" toastOptions={{
            className: 'dark:bg-zinc-800 dark:text-white',
          }} />
        </div>
    </ErrorBoundary>
  );
}

export default App;
