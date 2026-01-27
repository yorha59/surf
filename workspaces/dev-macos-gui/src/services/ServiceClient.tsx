import React, { createContext, useContext, useMemo, useState } from "react";

export interface ServiceState {
  connected: boolean;
  statusText: string;
}

export interface ServiceClient {
  scanStart(params: Record<string, unknown>): Promise<{ taskId: string }>;
  scanStatus(taskId: string): Promise<{ state: string; progress: number }>;
  scanResult(taskId: string): Promise<Record<string, unknown>>;
  scanCancel(taskId: string): Promise<{ canceled: boolean }>;
}

interface ServiceClientContextValue {
  serviceState: ServiceState;
  client: ServiceClient;
}

const ServiceClientContext = createContext<ServiceClientContextValue | null>(
  null
);

/**
 * ServiceClient 的占位实现。
 *
 * 当前不会真正连接 JSON-RPC 服务，仅返回模拟数据，用于打通 GUI 调用路径。
 * 后续可以在此位置接入：
 * - Tauri `invoke` 到 Rust 命令；或
 * - 直接通过 JSON-RPC over TCP/WebSocket 与 `dev-service-api` 通信。
 */
function createMockServiceClient(): ServiceClient {
  return {
    async scanStart(): Promise<{ taskId: string }> {
      return { taskId: "mock-task-id" };
    },
    async scanStatus(): Promise<{ state: string; progress: number }> {
      return { state: "disconnected", progress: 0 };
    },
    async scanResult(): Promise<Record<string, unknown>> {
      return { placeholder: true };
    },
    async scanCancel(): Promise<{ canceled: boolean }> {
      return { canceled: false };
    }
  };
}

export const ServiceClientProvider: React.FC<React.PropsWithChildren> = ({
  children
}) => {
  const [serviceState] = useState<ServiceState>({
    connected: false,
    statusText: "未连接到 JSON-RPC 服务（占位状态）"
  });

  const client = useMemo(() => createMockServiceClient(), []);

  const value: ServiceClientContextValue = {
    serviceState,
    client
  };

  return (
    <ServiceClientContext.Provider value={value}>
      {children}
    </ServiceClientContext.Provider>
  );
};

export function useServiceClient(): ServiceClientContextValue {
  const ctx = useContext(ServiceClientContext);
  if (!ctx) {
    throw new Error("useServiceClient must be used within ServiceClientProvider");
  }
  return ctx;
}
