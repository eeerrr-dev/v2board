import type { ReactNode } from 'react';

export function LegacySpin({ loading, children }: { loading: boolean; children: ReactNode }) {
  return (
    <div className="ant-spin-nested-loading">
      <div>
        <div
          className={`ant-spin${loading ? ' ant-spin-spinning' : ''}`}
          style={loading ? undefined : { display: 'none' }}
        >
          <div className="spinner-grow text-primary" />
        </div>
      </div>
      <div className={`ant-spin-container${loading ? ' ant-spin-blur' : ''}`}>
        {children}
      </div>
    </div>
  );
}
