import { Component, type ErrorInfo, type ReactNode } from "react";
import { translate } from "../i18n";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("ErrorBoundary caught:", error, errorInfo);
    this.setState({ errorInfo });
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null, errorInfo: null });
  };

  handleCopyDetails = async () => {
    const { error, errorInfo } = this.state;
    if (!error) return;
    const parts = [
      error.name + ": " + error.message,
      error.stack ?? "",
      errorInfo?.componentStack ?? "",
    ];
    const text = parts.filter(Boolean).join("\n\n---\n\n");
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      /* ignore */
    }
  };

  render() {
    if (this.state.hasError && this.state.error) {
      if (this.props.fallback) {
        return this.props.fallback;
      }
      return (
        <div className="min-h-screen flex flex-col items-center justify-center bg-surface dark:bg-surface-dark p-6">
          <div className="max-w-md w-full text-center space-y-4">
            <div className="text-4xl">⚠️</div>
            <h1 className="text-lg font-semibold text-ink dark:text-ink-dark">
              {translate("error.title")}
            </h1>
            <p className="text-sm text-ink-mute dark:text-ink-dark-mute break-words text-left">
              {this.state.error.message}
            </p>
            <div className="flex flex-wrap gap-2 justify-center">
              <button
                type="button"
                onClick={this.handleRetry}
                className="px-4 py-2 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent-hover transition-colors"
              >
              {translate("common.retry")}
            </button>
              <button
                type="button"
                onClick={() => void this.handleCopyDetails()}
                className="px-4 py-2 rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark text-sm font-medium hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
              >
                {translate("error.copyDetails")}
              </button>
            </div>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute">
              {translate("error.copyHint")}
            </p>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
