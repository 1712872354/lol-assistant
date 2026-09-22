import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  error: Error | null;
}

/** 根级 Error Boundary：子树渲染崩溃时给出可重试错误态，避免整页白屏 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      if (this.props.fallback) return this.props.fallback;
      return (
        <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-sm">
          <p className="font-medium text-destructive">界面渲染出错</p>
          <p className="max-w-md break-all text-xs text-muted-foreground">
            {this.state.error.message}
          </p>
          <button
            type="button"
            className="rounded-md border px-3 py-1.5 text-xs"
            onClick={() => this.setState({ error: null })}
          >
            重试
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
