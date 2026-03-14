import { useState, useEffect } from 'react';
import { MessageSquare, Database, Terminal, Settings, Workflow, Menu, X, LayoutDashboard, Activity, Plug, Brain, Eye } from 'lucide-react';
import { NavLink } from 'react-router-dom';
import clsx from 'clsx';

type SidebarProps = {
  onRouteIntent?: (path: string) => void;
};

// 系统指标类型
interface SystemMetrics {
  vramUsage: number;
  cpuUsage: number;
  activeAgents: number;
  status: 'online' | 'thinking' | 'offline';
}

export const Sidebar = ({ onRouteIntent }: SidebarProps) => {
  const [isOpen, setIsOpen] = useState(false);
  const [metrics, setMetrics] = useState<SystemMetrics>({
    vramUsage: 0,
    cpuUsage: 0,
    activeAgents: 0,
    status: 'online'
  });

  // 模拟获取系统指标
  useEffect(() => {
    const updateMetrics = () => {
      setMetrics({
        vramUsage: Math.floor(Math.random() * 30) + 40, // 40-70%
        cpuUsage: Math.floor(Math.random() * 20) + 10,  // 10-30%
        activeAgents: Math.floor(Math.random() * 3) + 1, // 1-3
        status: 'online'
      });
    };
    updateMetrics();
    const interval = setInterval(updateMetrics, 5000);
    return () => clearInterval(interval);
  }, []);

  const menuItems = [
    { path: '/dashboard', icon: LayoutDashboard, label: 'Dashboard' },
    { path: '/chat', icon: MessageSquare, label: 'Chat' },
    { path: '/canvas', icon: Workflow, label: 'Canvas' },
    { path: '/skills', icon: Terminal, label: 'Skills' },
    { path: '/activity', icon: Activity, label: 'Activity' },
    { path: '/mcp', icon: Plug, label: 'MCP' },
    { path: '/memory', icon: Brain, label: 'Memory' },
    { path: '/knowledge', icon: Database, label: 'Knowledge' },
    { path: '/observability', icon: Eye, label: 'Observability' },
    { path: '/settings', icon: Settings, label: 'Settings' },
  ] as const;

  // 呼吸灯颜色根据状态变化
  const getStatusColor = (status: string) => {
    switch (status) {
      case 'online': return 'bg-emerald-500';
      case 'thinking': return 'bg-amber-500';
      case 'offline': return 'bg-rose-500';
      default: return 'bg-zinc-500';
    }
  };

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
              onMouseEnter={() => onRouteIntent?.(item.path)}
              onFocus={() => onRouteIntent?.(item.path)}
              onTouchStart={() => onRouteIntent?.(item.path)}
              onClick={() => setIsOpen(false)}
              className={({ isActive }) => clsx(
                "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all duration-200 group relative",
                isActive 
                  ? "text-white" 
                  : "text-zinc-400 hover:text-zinc-100"
              )}
            >
              {({ isActive }) => (
                <>
                  {/* 侧边高亮条 */}
                  <div className={clsx(
                    "absolute left-0 top-1/2 -translate-y-1/2 w-1 h-6 rounded-r-full transition-all duration-200",
                    isActive ? "bg-blue-500 opacity-100" : "bg-blue-500 opacity-0 group-hover:opacity-30"
                  )} />
                  <item.icon className={clsx(
                    "w-5 h-5 transition-colors ml-1", 
                    isActive ? "text-blue-400" : "text-zinc-500 group-hover:text-zinc-300"
                  )} />
                  <span className={clsx(
                    "font-medium text-sm",
                    isActive ? "text-zinc-100" : "text-zinc-400 group-hover:text-zinc-100"
                  )}>{item.label}</span>
                </>
              )}
            </NavLink>
          ))}
        </nav>
        
        {/* 系统仪表盘 */}
        <div className="p-4 border-t border-zinc-800">
          <div className="px-3 py-3 rounded-lg bg-zinc-950/50 border border-zinc-800/50 space-y-3">
            {/* 状态指示器 */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className={clsx(
                  "w-2 h-2 rounded-full animate-breathe",
                  getStatusColor(metrics.status)
                )}></div>
                <span className="text-xs font-medium text-zinc-400">
                  {metrics.status === 'online' ? 'System Normal' : 
                   metrics.status === 'thinking' ? 'Thinking...' : 'Offline'}
                </span>
              </div>
              <span className="text-[10px] text-zinc-600 font-mono">v0.2.0-beta</span>
            </div>
            
            {/* 指标网格 */}
            <div className="grid grid-cols-3 gap-2">
              <div className="text-center">
                <div className="text-[10px] text-zinc-500 mb-0.5">VRAM</div>
                <div className="text-xs font-semibold text-zinc-300">{metrics.vramUsage}%</div>
                <div className="w-full h-1 bg-zinc-800 rounded-full mt-1 overflow-hidden">
                  <div 
                    className="h-full bg-blue-500 rounded-full transition-all duration-500"
                    style={{ width: `${metrics.vramUsage}%` }}
                  />
                </div>
              </div>
              <div className="text-center">
                <div className="text-[10px] text-zinc-500 mb-0.5">CPU</div>
                <div className="text-xs font-semibold text-zinc-300">{metrics.cpuUsage}%</div>
                <div className="w-full h-1 bg-zinc-800 rounded-full mt-1 overflow-hidden">
                  <div 
                    className="h-full bg-emerald-500 rounded-full transition-all duration-500"
                    style={{ width: `${metrics.cpuUsage}%` }}
                  />
                </div>
              </div>
              <div className="text-center">
                <div className="text-[10px] text-zinc-500 mb-0.5">Agents</div>
                <div className="text-xs font-semibold text-zinc-300">{metrics.activeAgents}</div>
                <div className="flex justify-center gap-0.5 mt-1">
                  {Array.from({ length: 3 }).map((_, i) => (
                    <div 
                      key={i}
                      className={clsx(
                        "w-1.5 h-1.5 rounded-full",
                        i < metrics.activeAgents ? "bg-amber-500" : "bg-zinc-700"
                      )}
                    />
                  ))}
                </div>
              </div>
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
