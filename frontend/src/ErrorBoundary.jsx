import { Component } from "react";

export class ErrorBoundary extends Component {
  constructor(props) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error) {
    return { error };
  }

  componentDidCatch(error, info) {
    console.error("SlotForge UI error:", error, info);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex min-h-screen flex-col bg-bg-primary p-8 text-danger font-mono text-sm">
          <h1 className="font-display text-lg font-semibold text-accent">SlotForge failed to start</h1>
          <pre className="mt-4 whitespace-pre-wrap text-text-primary">{String(this.state.error)}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}
