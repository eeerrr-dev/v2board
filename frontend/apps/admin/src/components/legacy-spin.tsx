import type { ReactNode } from 'react';

export function LegacySpin({ loading, children }: { loading: boolean; children: ReactNode }) {
  return (
    <div className="ant-spin-nested-loading">
      {loading ? (
        <div>
          <div className="ant-spin ant-spin-spinning">
            <div className="spinner-grow text-primary" />
          </div>
        </div>
      ) : null}
      <div className={`ant-spin-container${loading ? ' ant-spin-blur' : ''}`}>
        {children}
      </div>
    </div>
  );
}
