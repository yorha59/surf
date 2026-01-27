import React, { createContext, useContext, useMemo, useState } from "react";

/**
 * JSON-RPC 扫描相关类型（与 Architecture.md 6.2 及 dev-service-api 对齐的最小子集）。
 */

export type ScanState =
  | "queued"
  | "running"
  | "completed"
  | "canceled"
  | "failed";

export interface ScanStatus {
  task_id: string;
  state: ScanState | string;
  progress: number; // 0.0 - 1.0
  scanned_files?: number;
  scanned_bytes?: number;
  eta_seconds?: number;
  error?: { code: number; message: string } | null;
}

export interface ScanSummary {
  root_path: string;
  total_files: number;
  total_dirs: number;
  total_size_bytes: number;
  elapsed_seconds?: number;
}

export interface TopFile {
  path: string;
  size_bytes?: number;
  last_modified?: string;
}

export interface ScanResultPayload {
  task_id?: string;
  summary?: ScanSummary;
  top_files?: TopFile[];
  // 其他字段按需扩展
  by_extension?: unknown;
  stale_files?: unknown;
}

export interface ScanOptions {
  threads?: number;
  min_size?: string | number;
  limit?: number;
  exclude_patterns?: string[];
  stale_days?: number;
}

export interface ServiceState {
  connected: boolean;
  statusText: string;
  lastError?: string | null;
}

export interface ServiceClient {
  /**
   * 启动扫描，对应 JSON-RPC `scan.start`。
   *
   * `path` 同时映射为 `path` 与 `root_path` 字段，以兼容早期实现。
   */
  startScan(
    path: string,
    options?: ScanOptions
  ): Promise<{ taskId: string }>;

  /** 查询进度，对应 JSON-RPC `scan.status`。 */
  getStatus(taskId: string): Promise<ScanStatus>;

  /** 获取结果，对应 JSON-RPC `scan.result`。 */
  getResult(taskId: string): Promise<ScanResultPayload>;

  /** 取消任务，对应 JSON-RPC `scan.cancel`。 */
  cancel(taskId: string): Promise<{ canceled: boolean }>;
}

interface ServiceClientContextValue {
  serviceState: ServiceState;
  client: ServiceClient;
}

const ServiceClientContext = createContext<ServiceClientContextValue | null>(
  null
);

type RpcErrorType = "network" | "rpc";

/**
 * 统一的 JSON-RPC 错误类型，调用方可以通过 `instanceof RpcError` + `type` 字段判断
 * 是网络错误还是业务错误。
 */
export class RpcError extends Error {
  readonly type: RpcErrorType;
  readonly code?: number;
  readonly data?: unknown;

  constructor(type: RpcErrorType, message: string, code?: number, data?: unknown) {
    super(message);
    this.name = "RpcError";
    this.type = type;
    this.code = code;
    this.data = data;
  }
}

const JSON_RPC_ENDPOINT = "/rpc";

interface JsonRpcResponse<TResult> {
  jsonrpc?: string;
  id?: number | string | null;
  result?: TResult;
  error?: { code: number; message: string; data?: unknown } | null;
}

async function callJsonRpc<TResult, TParams = unknown>(
  method: string,
  params?: TParams
): Promise<TResult> {
  const payload = {
    jsonrpc: "2.0" as const,
    method,
    params,
    id: Date.now()
  };

  let res: Response;
  try {
    res = await fetch(JSON_RPC_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify(payload)
    });
  } catch (e) {
    throw new RpcError(
      "network",
      "无法连接到本地 JSON-RPC 服务（127.0.0.1:1234）",
      undefined,
      e
    );
  }

  let json: JsonRpcResponse<TResult>;
  try {
    json = (await res.json()) as JsonRpcResponse<TResult>;
  } catch (e) {
    throw new RpcError("network", "无法解析 JSON-RPC 响应", undefined, e);
  }

  if (!res.ok) {
    throw new RpcError(
      "network",
      `HTTP 请求失败，状态码 ${res.status}`,
      res.status,
      json
    );
  }

  if (json.error) {
    throw new RpcError(
      "rpc",
      json.error.message ?? "JSON-RPC 调用失败",
      json.error.code,
      json.error.data
    );
  }

  if (typeof json.result === "undefined") {
    throw new RpcError("rpc", "JSON-RPC 响应缺少 result 字段");
  }

  return json.result as TResult;
}

function createServiceClient(
  setServiceState: React.Dispatch<React.SetStateAction<ServiceState>>
): ServiceClient {
  const markConnected = () => {
    setServiceState((prev) => ({
      ...prev,
      connected: true,
      statusText: "已连接到本地 JSON-RPC 服务",
      lastError: null
    }));
  };

  const markError = (error: unknown) => {
    const message =
      error instanceof RpcError
        ? error.message
        : "与 JSON-RPC 服务通信失败";
    setServiceState((prev) => ({
      ...prev,
      connected: false,
      statusText: message,
      lastError: message
    }));
  };

  return {
    async startScan(
      path: string,
      options?: ScanOptions
    ): Promise<{ taskId: string }> {
      try {
        const result = await callJsonRpc<{ task_id: string }>("scan.start", {
          // 同时传递 path 与 root_path 以兼容早期实现
          path,
          root_path: path,
          ...options
        });
        markConnected();
        return { taskId: result.task_id };
      } catch (e) {
        markError(e);
        throw e;
      }
    },

    async getStatus(taskId: string): Promise<ScanStatus> {
      try {
        const raw = await callJsonRpc<ScanStatus>("scan.status", {
          task_id: taskId
        });
        markConnected();
        const normalizedState = (raw.state || "").toString().toLowerCase() as
          | ScanState
          | string;
        return { ...raw, state: normalizedState };
      } catch (e) {
        markError(e);
        throw e;
      }
    },

    async getResult(taskId: string): Promise<ScanResultPayload> {
      try {
        const result = await callJsonRpc<ScanResultPayload>("scan.result", {
          task_id: taskId
        });
        markConnected();
        return result;
      } catch (e) {
        markError(e);
        throw e;
      }
    },

    async cancel(taskId: string): Promise<{ canceled: boolean }> {
      try {
        const result = await callJsonRpc<{ task_id?: string; canceled?: boolean } | null>(
          "scan.cancel",
          { task_id: taskId }
        );
        markConnected();
        const canceled = result && typeof result === "object"
          ? (result.canceled ?? true)
          : true;
        return { canceled };
      } catch (e) {
        markError(e);
        throw e;
      }
    }
  };
}

export const ServiceClientProvider: React.FC<React.PropsWithChildren> = ({
  children
}) => {
  const [serviceState, setServiceState] = useState<ServiceState>({
    connected: false,
    statusText: "尚未连接到 JSON-RPC 服务",
    lastError: null
  });

  const client = useMemo(() => createServiceClient(setServiceState), []);

  const value = useMemo<ServiceClientContextValue>(
    () => ({ serviceState, client }),
    [serviceState, client]
  );

  return (
    <ServiceClientContext.Provider value={value}>
      {children}
    </ServiceClientContext.Provider>
  );
};

export function useServiceClient(): ServiceClientContextValue {
  const ctx = useContext(ServiceClientContext);
  if (!ctx) {
    throw new Error("useServiceClient 必须在 ServiceClientProvider 内使用");
  }
  return ctx;
}
