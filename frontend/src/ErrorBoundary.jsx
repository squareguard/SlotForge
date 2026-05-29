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
        <div
          style={{
            minHeight: "100vh",
            padding: "2rem",
            background: "#0a0a0f",
            color: "#ff2d55",
            fontFamily: "system-ui, sans-serif",
          }}
        >
          <h1 style={{ color: "#00f5ff", marginTop: 0 }}>SlotForge failed to start</h1>
          <pre style={{ whiteSpace: "pre-wrap", color: "#e8f4f8" }}>{String(this.state.error)}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}
