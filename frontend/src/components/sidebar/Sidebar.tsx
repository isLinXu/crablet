import { useState } from 'react';
import { MessageSquare, Database, Terminal, Settings, Workflow, Menu, X, LayoutDashboard, Activity, Plug, Brain } from 'lucide-react';
import { NavLink } from 'react-router-dom';
import clsx from 'clsx';

export const Sidebar = () => {
  const [isOpen, setIsOpen] = useState(false);

  const menuItems = [
    { path: '/dashboard', icon: LayoutDashboard, label: 'Dashboard' },
    { path: '/chat', icon: MessageSquare, label: 'Chat' },
    { path: '/canvas', icon: Workflow, label: 'Canvas' },
    { path: '/skills', icon: Terminal, label: 'Skills' },
    { path: '/activity', icon: Activity, label: 'Activity' },
    { path: '/mcp', icon: Plug, label: 'MCP' },
    { path: '/memory', icon: Brain, label: 'Memory' },
    { path: '/knowledge', icon: Database, label: 'Knowledge' },
    { path: '/settings', icon: Settings, label: 'Settings' },
  ] as const;

  return (
    <>
      {/* Mobile Burger Button */}
      <button 
        className={clsx(
          "md:hidden fixed top-4 left-4 z-50 p-2 rounded-lg shadow-lg transition-colors",
          isOpen ? "bg-zinc-800 text-white" : "bg-white dark:bg-zinc-800 text-zinc-900 dark:text-white"
        )}
        onClick={() => setIsOpen(!isOpen)}
      >
        {isOpen ? <X size={20} /> : <Menu size={20} />}
      </button>

      {/* Sidebar: Mobile overlay + Desktop fixed */}
      <aside className={clsx(
        'fixed md:static inset-y-0 left-0 z-40 w-64 bg-zinc-900 text-white flex flex-col h-full transform transition-transform duration-300 ease-in-out border-r border-zinc-800',
        isOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0'
      )}>
        <div className="p-6 border-b border-zinc-800 flex items-center gap-3">
          <div className="w-8 h-8 bg-blue-600 rounded-lg flex items-center justify-center shadow-lg shadow-blue-500/20">
            <span className="text-xl">🦀</span>
          </div>
          <h1 className="text-lg font-bold tracking-wide text-zinc-100">
            Crablet
          </h1>
        </div>
        
        <nav className="flex-1 px-3 py-6 space-y-1">
          {menuItems.map((item) => (
            <NavLink
              key={item.path}
              to={item.path}
              onClick={() => setIsOpen(false)}
              className={({ isActive }) => clsx(
                "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all duration-200 group",
                isActive 
                  ? "bg-blue-600 text-white shadow-md shadow-blue-900/20" 
                  : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
              )}
            >
              {({ isActive }) => (
                <>
                  <item.icon className={clsx("w-5 h-5 transition-colors", isActive ? "text-white" : "text-zinc-500 group-hover:text-zinc-300")} />
                  <span className="font-medium text-sm">{item.label}</span>
                </>
              )}
            </NavLink>
          ))}
        </nav>
        
        <div className="p-4 border-t border-zinc-800">
          <div className="px-3 py-2 rounded-lg bg-zinc-950/50 border border-zinc-800/50">
            <div className="flex items-center gap-2 mb-1">
                <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></div>
                <span className="text-xs font-medium text-zinc-400">System Normal</span>
            </div>
            <div className="text-[10px] text-zinc-600 font-mono">
                v0.2.0-beta
            </div>
          </div>
        </div>
      </aside>

      {/* Mobile Overlay */}
      {isOpen && (
        <div 
          className="md:hidden fixed inset-0 bg-black/60 z-30 backdrop-blur-sm transition-opacity" 
          onClick={() => setIsOpen(false)} 
        />
      )}
    </>
  );
};
