import React from 'react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// 小螃蟹加载动画
interface CrabLoaderProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export const CrabLoader: React.FC<CrabLoaderProps> = ({ size = 'md', className }) => {
  const sizeClasses = {
    sm: 'w-8 h-8',
    md: 'w-12 h-12',
    lg: 'w-20 h-20',
  };

  return (
    <div className={cn("relative", sizeClasses[size], className)}>
      {/* 螃蟹身体 */}
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="crab-body relative">
          {/* 身体 */}
          <div className={cn(
            "rounded-full bg-gradient-to-br from-red-400 to-red-600 shadow-lg",
            size === 'sm' ? 'w-5 h-4' : size === 'md' ? 'w-8 h-6' : 'w-14 h-10'
          )}>
            {/* 眼睛 */}
            <div className="absolute -top-1 left-1/4 flex gap-1">
              <div className={cn(
                "bg-white rounded-full shadow-sm",
                size === 'sm' ? 'w-1.5 h-1.5' : size === 'md' ? 'w-2 h-2' : 'w-3 h-3'
              )}>
                <div className={cn(
                  "bg-black rounded-full mt-0.5 ml-0.5",
                  size === 'sm' ? 'w-0.5 h-0.5' : size === 'md' ? 'w-1 h-1' : 'w-1.5 h-1.5'
                )} />
              </div>
              <div className={cn(
                "bg-white rounded-full shadow-sm",
                size === 'sm' ? 'w-1.5 h-1.5' : size === 'md' ? 'w-2 h-2' : 'w-3 h-3'
              )}>
                <div className={cn(
                  "bg-black rounded-full mt-0.5 ml-0.5",
                  size === 'sm' ? 'w-0.5 h-0.5' : size === 'md' ? 'w-1 h-1' : 'w-1.5 h-1.5'
                )} />
              </div>
            </div>
            {/* 嘴巴 */}
            <div className={cn(
              "absolute bottom-1 left-1/2 -translate-x-1/2 bg-red-800 rounded-full",
              size === 'sm' ? 'w-2 h-0.5' : size === 'md' ? 'w-3 h-1' : 'w-5 h-1.5'
            )} />
          </div>
          
          {/* 左钳子 */}
          <div className={cn(
            "absolute -left-1 top-1/2 -translate-y-1/2 origin-right animate-claw-left",
            size === 'sm' ? 'w-2 h-2' : size === 'md' ? 'w-3 h-3' : 'w-5 h-5'
          )}>
            <div className="w-full h-full bg-red-500 rounded-full border-2 border-red-600" />
          </div>
          
          {/* 右钳子 */}
          <div className={cn(
            "absolute -right-1 top-1/2 -translate-y-1/2 origin-left animate-claw-right",
            size === 'sm' ? 'w-2 h-2' : size === 'md' ? 'w-3 h-3' : 'w-5 h-5'
          )}>
            <div className="w-full h-full bg-red-500 rounded-full border-2 border-red-600" />
          </div>
        </div>
      </div>
      
      {/* 爬行动画 */}
      <div className="absolute inset-0 animate-crab-walk">
        {/* 腿 - 左侧 */}
        <div className="absolute left-0 top-1/2 -translate-y-1/2">
          {[0, 1, 2].map((i) => (
            <div
              key={`left-${i}`}
              className={cn(
                "absolute bg-red-500 rounded-full origin-right animate-leg",
                size === 'sm' ? 'w-2 h-0.5' : size === 'md' ? 'w-3 h-1' : 'w-4 h-1.5'
              )}
              style={{
                transform: `rotate(${-30 + i * 20}deg)`,
                top: `${-4 + i * 4}px`,
                left: '0',
                animationDelay: `${i * 100}ms`,
              }}
            />
          ))}
        </div>
        
        {/* 腿 - 右侧 */}
        <div className="absolute right-0 top-1/2 -translate-y-1/2">
          {[0, 1, 2].map((i) => (
            <div
              key={`right-${i}`}
              className={cn(
                "absolute bg-red-500 rounded-full origin-left animate-leg",
                size === 'sm' ? 'w-2 h-0.5' : size === 'md' ? 'w-3 h-1' : 'w-4 h-1.5'
              )}
              style={{
                transform: `rotate(${30 - i * 20}deg)`,
                top: `${-4 + i * 4}px`,
                right: '0',
                animationDelay: `${i * 100 + 150}ms`,
              }}
            />
          ))}
        </div>
      </div>
    </div>
  );
};

// 螃蟹空状态
interface CrabEmptyStateProps {
  title?: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}

export const CrabEmptyState: React.FC<CrabEmptyStateProps> = ({
  title = '小螃蟹准备好了',
  description = '开始你的第一次对话吧',
  action,
  className,
}) => {
  return (
    <div className={cn("flex flex-col items-center justify-center p-8 text-center", className)}>
      {/* 螃蟹动画容器 */}
      <div className="relative mb-6">
        {/* 波浪效果 */}
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="w-32 h-32 bg-blue-400/10 rounded-full animate-ping" style={{ animationDuration: '3s' }} />
        </div>
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="w-24 h-24 bg-blue-400/20 rounded-full animate-ping" style={{ animationDuration: '3s', animationDelay: '0.5s' }} />
        </div>
        
        {/* 螃蟹 */}
        <div className="relative z-10">
          <CrabLoader size="lg" />
        </div>
        
        {/* 气泡 */}
        <div className="absolute -top-2 -right-4 animate-bubble">
          <div className="bg-white dark:bg-zinc-800 rounded-full px-3 py-1.5 shadow-lg border border-zinc-200 dark:border-zinc-700">
            <span className="text-sm text-zinc-600 dark:text-zinc-300">🦀 嗨！</span>
          </div>
        </div>
      </div>
      
      <h3 className="text-xl font-semibold text-zinc-800 dark:text-zinc-100 mb-2">
        {title}
      </h3>
      <p className="text-sm text-zinc-500 dark:text-zinc-400 max-w-xs mb-4">
        {description}
      </p>
      {action}
    </div>
  );
};

// 螃蟹思考中
interface CrabThinkingProps {
  message?: string;
  className?: string;
}

export const CrabThinking: React.FC<CrabThinkingProps> = ({
  message = '小螃蟹正在思考...',
  className,
}) => {
  return (
    <div className={cn("flex items-center gap-3 p-3 rounded-xl bg-zinc-100/50 dark:bg-zinc-800/50", className)}>
      <CrabLoader size="sm" />
      <div className="flex-1">
        <p className="text-sm text-zinc-600 dark:text-zinc-300">{message}</p>
        <div className="flex gap-1 mt-1.5">
          <span className="w-1.5 h-1.5 bg-zinc-400 rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
          <span className="w-1.5 h-1.5 bg-zinc-400 rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
          <span className="w-1.5 h-1.5 bg-zinc-400 rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
        </div>
      </div>
    </div>
  );
};

export default { CrabLoader, CrabEmptyState, CrabThinking };
