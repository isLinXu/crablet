import React, { useEffect, useState, useCallback, useRef } from 'react';
import { getApiBaseUrl } from '../utils/constants';

type SidecarState = 'starting' | 'running' | 'error' | 'exited';

/**
 * 后端连接状态检测组件。
 * 在后端未就绪时显示友好的等待提示，就绪后自动消失。
 * 不阻塞主界面渲染——用户可以先看到界面结构。
 *
 * 同时监听 Tauri sidecar 事件（sidecar-error / sidecar-exit），
 * 在 sidecar 崩溃时立即显示错误信息而非无限等待。
 */
export const BackendStatus: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [ready, setReady] = useState(false);
  const [checking, setChecking] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const [sidecarError, setSidecarError] = useState<string | null>(null);
  const [sidecarState, setSidecarState] = useState<SidecarState>('starting');
  const errorRef = useRef<HTMLDivElement>(null);

  // 监听 Tauri sidecar 事件
  useEffect(() => {
    // 检测 Tauri API 是否可用
    const tauriAPI = (window as unknown as { __TAURI__?: { event?: { listen: (evt: string, cb: (e: { payload: unknown }) => void) => Promise<() => void> } } }).__TAURI__;
    if (!tauriAPI?.event?.listen) return;

    let unlistenError: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;

    tauriAPI.event.listen('sidecar-error', (e) => {
      const msg = typeof e.payload === 'string' ? e.payload : 'Sidecar 启动失败';
      setSidecarError(msg);
      setSidecarState('error');
    });

    tauriAPI.event.listen('sidecar-exit', (e) => {
      const msg = typeof e.payload === 'string' ? e.payload : 'Sidecar 进程已退出';
      setSidecarError(msg);
      setSidecarState('exited');
    });

    return () => {
      unlistenError?.();
      unlistenExit?.();
    };
  }, []);

  const checkBackend = useCallback(async () => {
    try {
      const baseUrl = getApiBaseUrl();
      // health 端点在根路径 /health，不在 /api 下
      const healthUrl = baseUrl.replace(/\/api$/, '/health');
      const resp = await fetch(healthUrl, { method: 'GET', cache: 'no-store' });
      if (resp.ok) {
        setReady(true);
        setSidecarState('running');
        return true;
      }
    } catch {
      // 后端未就绪，继续轮询
    }
    return false;
  }, []);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;

    const poll = async () => {
      while (!cancelled) {
        const ok = await checkBackend();
        if (ok) {
          setChecking(false);
          return;
        }
        setAttempt((a) => a + 1);
        // 等待 1.5s 后重试
        await new Promise<void>((resolve) => {
          timer = setTimeout(resolve, 1500);
        });
      }
    };

    poll();

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [checkBackend]);

  // 后端就绪，正常渲染子组件
  if (ready) {
    return <>{children}</>;
  }

  // sidecar 崩溃或退出 —— 显示错误状态
  const hasError = sidecarState === 'error' || sidecarState === 'exited';

  // 后端未就绪，显示覆盖层
  return (
    <div className="relative h-full w-full">
      {children}
      {/* 半透明覆盖层 */}
      <div className="absolute inset-0 bg-white/80 dark:bg-zinc-900/80 backdrop-blur-sm z-50 flex items-center justify-center">
        <div className="text-center px-8" ref={errorRef}>
          {hasError ? (
            <>
              <div className="text-3xl mb-3">⚠️</div>
              <h2 className="text-lg font-semibold text-red-600 dark:text-red-400 mb-1">
                {sidecarState === 'error' ? '后端启动失败' : '后端进程已退出'}
              </h2>
              <p className="text-sm text-zinc-500 dark:text-zinc-400 mb-3 break-words max-w-md mx-auto">
                {sidecarError}
              </p>
              <div className="flex gap-2 justify-center">
                <button
                  className="px-4 py-2 text-sm rounded-lg bg-orange-500 text-white hover:bg-orange-600 transition-colors"
                  onClick={() => {
                    setSidecarError(null);
                    setSidecarState('starting');
                    setAttempt(0);
                    setChecking(true);
                  }}
                >
                  重新检测
                </button>
                <button
                  className="px-4 py-2 text-sm rounded-lg border border-zinc-300 dark:border-zinc-600 text-zinc-700 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
                  onClick={() => {
                    window.location.hash = '#/settings';
                  }}
                >
                  前往设置
                </button>
              </div>
            </>
          ) : (
            <>
              <div className="flex items-center justify-center gap-1.5 mb-4">
                <div className="w-2 h-2 rounded-full bg-orange-500 animate-bounce" style={{ animationDelay: '0ms' }} />
                <div className="w-2 h-2 rounded-full bg-orange-500 animate-bounce" style={{ animationDelay: '150ms' }} />
                <div className="w-2 h-2 rounded-full bg-orange-500 animate-bounce" style={{ animationDelay: '300ms' }} />
              </div>
              <h2 className="text-lg font-semibold text-zinc-800 dark:text-zinc-200 mb-1">
                正在连接后端服务
              </h2>
              <p className="text-sm text-zinc-500 dark:text-zinc-400 mb-3">
                {attempt === 0
                  ? '正在检测后端状态…'
                  : attempt < 5
                    ? '后端服务启动中，请稍候…'
                  : attempt < 15
                      ? '后端启动较慢，可能需要配置 API Key…'
                      : '后端连接超时，请检查设置或重启应用'}
              </p>
              {attempt >= 5 && (
                <button
                  className="px-4 py-2 text-sm rounded-lg bg-orange-500 text-white hover:bg-orange-600 transition-colors"
                  onClick={() => {
                    // 跳转到设置页面
                    window.location.hash = '#/settings';
                    // 同时继续轮询
                  }}
                >
                  前往设置
                </button>
              )}
              {attempt >= 15 && (
                <button
                  className="ml-2 px-4 py-2 text-sm rounded-lg border border-zinc-300 dark:border-zinc-600 text-zinc-700 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
                  onClick={() => {
                    setAttempt(0);
                    setChecking(true);
                  }}
                >
                  重试连接
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
};
