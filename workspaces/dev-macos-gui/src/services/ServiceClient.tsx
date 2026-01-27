import React, { createContext, useContext, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";

/**
 * JSON-RPC 扫描相关类型（与 Architecture.md 6.2 及 dev-service-api 对齐的最小子集）。
 */

// ---- 全局配置（~/.config/surf/config.json）类型与工具函数 ----

export interface SurfConfig {
  /** 默认扫描路径，例如 `~/` 或某个常用目录。 */
  default_path: string;
  /** 默认扫描线程数，建议为逻辑 CPU 数。 */
  threads: number;
  /** 最小过滤大小，字符串形式，支持 B/KB/MB/GB 等后缀。 */
  min_size: string;
  /** JSON-RPC 服务默认主机。 */
  rpc_host: string;
  /** JSON-RPC 服务默认端口。 */
  rpc_port: number;
  /** CLI 可执行文件路径，可选。 */
  cli_path?: string;
  /** GUI 首选主题，可选："light" / "dark"，缺省表示跟随系统。 */
  theme?: "light" | "dark";
  /** GUI 首选语言，可选："en" / "zh-CN"，缺省表示跟随系统。 */
  language?: "en" | "zh-CN";
}

/**
 * 根据浏览器环境推导一份用于 Onboarding 的默认配置。
 *
 * - default_path: `~/`（用户主目录）
 * - threads: 逻辑 CPU 核心数（如不可用则回退为 4）
 * - min_size: `"100MB"`（与 Architecture.md 4.5.1 建议一致）
 * - rpc_host / rpc_port: 127.0.0.1:1234
 */
export function createDefaultConfig(): SurfConfig {
  const threads = typeof navigator !== "undefined" && navigator.hardwareConcurrency
    ? navigator.hardwareConcurrency
    : 4;

  return {
    default_path: "~/",
    threads,
    min_size: "100MB",
    rpc_host: "127.0.0.1",
    rpc_port: 1234
  };
}

function isTauriAvailable(): boolean {
  if (typeof window === "undefined") return false;
  // `__TAURI_IPC__` 为 Tauri 在 WebView 中注入的标记，在纯浏览器环境下不存在。
  return Boolean((window as any).__TAURI_IPC__);
}

/**
 * 通过 Tauri `invoke` 从 `~/.config/surf/config.json` 读取配置。
 *
 * 在纯浏览器开发模式下（未通过 Tauri 启动）会直接返回 `null`，
 * 由上层逻辑决定是否进入 Onboarding 流程。
 */
export async function readConfig(): Promise<SurfConfig | null> {
  if (!isTauriAvailable()) {
    console.info("[surf gui] 当前不在 Tauri 环境中，readConfig 将返回 null");
    return null;
  }

  try {
    const config = await invoke<SurfConfig | null>("read_config");
    return config ?? null;
  } catch (e) {
    console.error("[surf gui] 调用 read_config 失败，将退回到 Onboarding 路径", e);
    return null;
  }
}

/**
 * 通过 Tauri `invoke` 将配置写入统一路径 `~/.config/surf/config.json`。
 *
 * 在非 Tauri 环境下，为避免阻塞 GUI 使用，函数会记录 warning 并直接返回，
 * 但不会真正写入本地配置文件（属于环境限制，而非功能缺陷）。
 */
export async function writeConfig(config: SurfConfig): Promise<void> {
  if (!isTauriAvailable()) {
    console.warn(
      "[surf gui] 当前不在 Tauri 环境中，写入配置将被跳过（仅在 Tauri 应用中生效）",
      config
    );
    return;
  }

  try {
    await invoke("write_config", { config });
  } catch (e) {
    console.error("[surf gui] 调用 write_config 失败，配置未能持久化", e);
    throw e;
  }
}

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
const JSON_RPC_TIMEOUT_MS = 10_000; // 默认 10 秒超时，避免请求挂死

interface JsonRpcResponse<TResult> {
  jsonrpc?: string;
  id?: number | string | null;
  result?: TResult;
  error?: { code: number; message: string; data?: unknown } | null;
}

/**
 * 统一 JSON-RPC 请求入口，封装 HTTP/网络错误与 JSON-RPC error 字段。
 */
export async function request<TResult, TParams = unknown>(
  method: string,
  params?: TParams,
  timeoutMs: number = JSON_RPC_TIMEOUT_MS
): Promise<TResult> {
  const payload = {
    jsonrpc: "2.0" as const,
    method,
    params,
    id: Date.now()
  };

  let res: Response;
  const controller = new AbortController();
  const timer = window.setTimeout(() => {
    controller.abort();
  }, timeoutMs);

  try {
    res = await fetch(JSON_RPC_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify(payload),
      signal: controller.signal
    });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") {
      throw new RpcError(
        "network",
        "JSON-RPC 请求超时，请确认本地服务是否已启动",
        undefined,
        e
      );
    }
    throw new RpcError(
      "network",
      "无法连接到本地 JSON-RPC 服务（127.0.0.1:1234）",
      undefined,
      e
    );
  } finally {
    window.clearTimeout(timer);
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
        const result = await request<{ task_id: string }>("scan.start", {
          // 仅传递 root_path，服务端通过 serde alias 兼容早期使用 `path` 的实现
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
        const raw = await request<ScanStatus>("scan.status", {
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
        const result = await request<ScanResultPayload>("scan.result", {
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
        const result = await request<
          { task_id?: string; canceled?: boolean } | null
        >(
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

// ---- 与 PRD/Architecture 约定一致的最小 JSON-RPC API ----

export async function scanStart(
  params: {
    path?: string;
    root_path?: string;
    threads?: number;
    min_size?: string | number;
    limit?: number;
    exclude_patterns?: string[];
    stale_days?: number;
  } = {
    path: ".",
    limit: 10
  }
): Promise<{ task_id: string }> {
  // 为兼容服务端既支持 `root_path` 又接受历史上的 `path` 字段，这里允许两者并存。
  return request<{ task_id: string }>("scan.start", params);
}

export async function scanStatus(taskId: string): Promise<ScanStatus> {
  return request<ScanStatus>("scan.status", { task_id: taskId });
}

export async function scanResult(taskId: string): Promise<ScanResultPayload> {
  return request<ScanResultPayload>("scan.result", { task_id: taskId });
}

export async function scanCancel(taskId: string): Promise<unknown> {
  return request<unknown>("scan.cancel", { task_id: taskId });
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
