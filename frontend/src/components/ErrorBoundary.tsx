import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
  children: ReactNode;
  variant?: 'app' | 'route';
  resetKey?: string;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Uncaught render error:', error, errorInfo);
  }

  public componentDidUpdate(previousProps: Props) {
    if (this.state.hasError && previousProps.resetKey !== this.props.resetKey) {
      this.setState({ hasError: false, error: undefined });
    }
  }

  private reset = () => this.setState({ hasError: false, error: undefined });

  public render() {
    if (!this.state.hasError) return this.props.children;
    const isRoute = this.props.variant === 'route';
    return (
      <div role="alert" className={`flex flex-col items-center justify-center bg-red-50 p-6 text-center dark:bg-red-900/20 ${isRoute ? 'h-full w-full' : 'min-h-screen'}`}>
        <h1 className="mb-2 text-2xl font-bold text-red-600 dark:text-red-400">
          {isRoute ? '此页面暂时无法显示' : '应用遇到了问题'}
        </h1>
        <p className="mb-4 max-w-lg text-sm text-red-500 dark:text-red-300">
          {this.state.error?.message || '发生了未知错误，请重试。'}
        </p>
        <div className="flex gap-2">
          <button type="button" onClick={this.reset} className="rounded bg-red-600 px-4 py-2 text-white hover:bg-red-700">重试</button>
          {!isRoute && <button type="button" onClick={() => window.location.reload()} className="rounded border border-red-300 px-4 py-2 text-red-700 hover:bg-red-100 dark:text-red-200">重新加载应用</button>}
        </div>
      </div>
    );
  }
}
