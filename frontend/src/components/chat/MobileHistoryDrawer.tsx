/**
 * MobileHistoryDrawer — 移动端历史会话抽屉
 *   当 showMobileHistory=true 时从左侧滑入，点击遮罩关闭
 */
import { X } from 'lucide-react';
import { SessionList } from './SessionList';

interface MobileHistoryDrawerProps {
  onClose: () => void;
}

export const MobileHistoryDrawer = ({ onClose }: MobileHistoryDrawerProps) => {
  return (
    <div className="absolute inset-0 z-50 flex md:hidden">
      <div className="w-64 bg-white dark:bg-zinc-900 border-r border-zinc-200 dark:border-zinc-800 h-full shadow-xl">
        <div className="p-4 border-b border-zinc-200 dark:border-zinc-800 flex justify-between items-center">
          <h2 className="font-semibold text-zinc-700 dark:text-zinc-200">History</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded"
          >
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="flex-1 overflow-hidden h-[calc(100%-60px)]">
          <SessionList />
        </div>
      </div>
      <div
        className="flex-1 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />
    </div>
  );
};
