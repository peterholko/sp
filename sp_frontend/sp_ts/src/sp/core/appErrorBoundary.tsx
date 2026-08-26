import React from 'react';

interface AppErrorBoundaryState {
  failed: boolean;
}

export default class AppErrorBoundary extends React.Component<
  React.PropsWithChildren,
  AppErrorBoundaryState
> {
  state: AppErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): AppErrorBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    console.error('Perilous UI failed to render', error, errorInfo);
  }

  render(): React.ReactNode {
    if (!this.state.failed) {
      return this.props.children;
    }

    return (
      <main
        role="alert"
        style={{
          alignItems: 'center',
          background: '#050403',
          boxSizing: 'border-box',
          color: '#ead7aa',
          display: 'flex',
          flexDirection: 'column',
          fontFamily: 'Georgia, serif',
          inset: 0,
          justifyContent: 'center',
          padding: '32px',
          position: 'fixed',
          textAlign: 'center',
          zIndex: 100000,
        }}
      >
        <h1 style={{ color: '#d59a4b', fontSize: '24px', margin: '0 0 12px' }}>
          Perilous could not finish loading
        </h1>
        <p style={{ lineHeight: 1.5, margin: '0 0 20px', maxWidth: '440px' }}>
          Your game is still safe on the server. Reload to reconnect.
        </p>
        <button
          type="button"
          onClick={() => window.location.reload()}
          style={{
            background: '#6f3e22',
            border: '1px solid #d59a4b',
            borderRadius: '5px',
            color: '#fff3d1',
            cursor: 'pointer',
            font: '600 16px Georgia, serif',
            padding: '10px 22px',
          }}
        >
          Reload Perilous
        </button>
      </main>
    );
  }
}
