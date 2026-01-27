import React, { useEffect, useState } from "react";
import {
  RpcError,
  ScanResultPayload,
  ScanStatus,
  TopFile,
  useServiceClient
} from "../services/ServiceClient";

export const CentralView: React.FC = () => {
  const { serviceState, client } = useServiceClient();

  const [scanPath, setScanPath] = useState<string>("");
  const [currentTaskId, setCurrentTaskId] = useState<string | null>(null);
  const [status, setStatus] = useState<ScanStatus | null>(null);
  const [topFiles, setTopFiles] = useState<TopFile[]>([]);
  const [isStarting, setIsStarting] = useState(false);
  const [isPolling, setIsPolling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleStartScan = async () => {
    if (!scanPath.trim()) {
      setError("请输入要扫描的路径（例如 / 或某个测试目录）");
      return;
    }
    setError(null);
    setTopFiles([]);
    setStatus(null);
    setIsStarting(true);

    try {
      const { taskId } = await client.startScan(scanPath.trim());
      setCurrentTaskId(taskId);
      setIsPolling(true);
    } catch (e) {
      const msg =
        e instanceof RpcError
          ? e.message
          : "启动扫描失败，请确认本地 JSON-RPC 服务已在 127.0.0.1:1234 监听";
      setError(msg);
    } finally {
      setIsStarting(false);
    }
  };

  const handleCancel = async () => {
    if (!currentTaskId) return;
    try {
      await client.cancel(currentTaskId);
    } catch (e) {
      const msg =
        e instanceof RpcError ? e.message : "取消任务失败，请稍后重试";
      setError(msg);
    } finally {
      setIsPolling(false);
    }
  };

  useEffect(() => {
    if (!currentTaskId || !isPolling) return;

    let canceled = false;
    let timer: number | undefined;

    const poll = async () => {
      try {
        const s = await client.getStatus(currentTaskId);
        if (canceled) return;

        setStatus(s);

        const state = s.state.toLowerCase();
        if (state === "completed") {
          try {
            const result: ScanResultPayload = await client.getResult(
              currentTaskId
            );
            if (!canceled) {
              const fromSummary =
                (result.summary as unknown as { top_files?: TopFile[] })
                  ?.top_files || [];
              const list =
                (result.top_files && result.top_files.length > 0
                  ? result.top_files
                  : fromSummary) || [];
              setTopFiles(list);
              setIsPolling(false);
            }
          } catch (e) {
            if (!canceled) {
              const msg =
                e instanceof RpcError ? e.message : "获取扫描结果失败";
              setError(msg);
              setIsPolling(false);
            }
          }
          return;
        }

        if (state === "failed") {
          setError(s.error?.message || "扫描任务失败");
          setIsPolling(false);
          return;
        }

        if (state === "canceled") {
          setIsPolling(false);
          return;
        }
      } catch (e) {
        if (!canceled) {
          const msg =
            e instanceof RpcError ? e.message : "查询任务状态失败";
          setError(msg);
          setIsPolling(false);
        }
        return;
      }

      if (!canceled) {
        timer = window.setTimeout(poll, 1000);
      }
    };

    poll();

    return () => {
      canceled = true;
      if (timer) {
        window.clearTimeout(timer);
      }
    };
  }, [client, currentTaskId, isPolling]);

  const progressPercent =
    status && Number.isFinite(status.progress)
      ? Math.round(status.progress * 100)
      : 0;

  return (
    <main
      style={{
        flex: 1,
        display: "grid",
        gridTemplateColumns: "minmax(0, 1.3fr) minmax(0, 1fr)",
        gap: "1rem",
        padding: "1rem",
        backgroundColor: "#020617"
      }}
    >
      <section
        style={{
          borderRadius: 12,
          border: "1px dashed #1f2937",
          padding: "0.9rem",
          display: "flex",
          flexDirection: "column",
          gap: "0.5rem"
        }}
      >
        <h2 style={{ fontSize: "0.9rem", fontWeight: 600, color: "#9ca3af" }}>
          Treemap 视图（占位）
        </h2>
        <p style={{ fontSize: "0.8rem", color: "#6b7280" }}>
          未来将在此展示磁盘占用 Treemap，可通过鼠标悬停与点击进行下钻浏览。
        </p>
      </section>
      <section
        style={{
          borderRadius: 12,
          border: "1px dashed #1f2937",
          padding: "0.9rem",
          display: "flex",
          flexDirection: "column",
          gap: "0.5rem"
        }}
      >
        <h2 style={{ fontSize: "0.9rem", fontWeight: 600, color: "#9ca3af" }}>
          最小端到端扫描（JSON-RPC）
        </h2>
        <p style={{ fontSize: "0.8rem", color: "#6b7280" }}>
          在下方输入要扫描的路径，点击「开始扫描」，GUI 将通过 JSON-RPC
          调用本地服务启动任务并轮询状态，在完成后展示 Top 文件列表。
        </p>

        <div
          style={{
            marginTop: "0.75rem",
            padding: "0.6rem 0.75rem",
            borderRadius: 8,
            backgroundColor: "#020617",
            display: "flex",
            flexDirection: "column",
            gap: "0.5rem"
          }}
        >
          <label
            style={{
              fontSize: "0.78rem",
              color: "#9ca3af",
              display: "flex",
              flexDirection: "column",
              gap: "0.3rem"
            }}
          >
            <span>扫描路径</span>
            <input
              type="text"
              value={scanPath}
              onChange={(e) => setScanPath(e.target.value)}
              placeholder="例如 /Users/you 或 workspaces/delivery-runner/test/tmp/tc1.Wp8z"
              style={{
                width: "100%",
                padding: "0.45rem 0.6rem",
                borderRadius: 8,
                border: "1px solid #1f2937",
                backgroundColor: "#020617",
                color: "#e5e7eb",
                fontSize: "0.8rem"
              }}
            />
          </label>
          <div
            style={{
              display: "flex",
              gap: "0.5rem",
              alignItems: "center"
            }}
          >
            <button
              type="button"
              onClick={handleStartScan}
              disabled={isStarting}
              style={{
                padding: "0.35rem 0.9rem",
                borderRadius: 999,
                border: "1px solid #22c55e",
                background: isStarting ? "#064e3b" : "#022c22",
                color: "#bbf7d0",
                fontSize: "0.8rem",
                cursor: isStarting ? "default" : "pointer",
                opacity: isStarting ? 0.7 : 1
              }}
            >
              {isStarting ? "正在启动扫描..." : "开始扫描"}
            </button>
            <button
              type="button"
              onClick={handleCancel}
              disabled={!currentTaskId || !isPolling}
              style={{
                padding: "0.35rem 0.9rem",
                borderRadius: 999,
                border: "1px solid #f97316",
                background: "transparent",
                color: "#fed7aa",
                fontSize: "0.8rem",
                cursor:
                  !currentTaskId || !isPolling ? "not-allowed" : "pointer",
                opacity: !currentTaskId || !isPolling ? 0.4 : 1
              }}
            >
              取消当前任务
            </button>
            {currentTaskId && (
              <span
                style={{
                  fontSize: "0.7rem",
                  color: "#6b7280",
                  marginLeft: "auto"
                }}
              >
                task_id: {currentTaskId}
              </span>
            )}
          </div>
          {error && (
            <div
              style={{
                marginTop: "0.25rem",
                fontSize: "0.75rem",
                color: "#fecaca"
              }}
            >
              错误：{error}
            </div>
          )}
        </div>

        <div
          style={{
            marginTop: "0.75rem",
            padding: "0.6rem 0.75rem",
            borderRadius: 8,
            backgroundColor: "#0f172a",
            fontSize: "0.78rem",
            lineHeight: 1.6,
            display: "flex",
            flexDirection: "column",
            gap: "0.4rem"
          }}
        >
          <strong style={{ color: "#f97316" }}>任务状态栏：</strong>
          <div style={{ color: "#e5e7eb" }}>
            <span>
              状态：
              {status
                ? status.state.toLowerCase()
                : serviceState.connected
                ? "idle"
                : "disconnected"}
            </span>
            {status && (
              <span style={{ marginLeft: "0.75rem" }}>
                进度：{progressPercent}%
              </span>
            )}
          </div>
          {status && (
            <div style={{ color: "#9ca3af", fontSize: "0.75rem" }}>
              <span>已扫描文件数：{status.scanned_files ?? 0}</span>
              <span style={{ marginLeft: "0.75rem" }}>
                已扫描字节数：{status.scanned_bytes ?? 0}
              </span>
              {status.eta_seconds != null && (
                <span style={{ marginLeft: "0.75rem" }}>
                  预计剩余：{status.eta_seconds}s
                </span>
              )}
            </div>
          )}
          <div
            style={{
              marginTop: "0.25rem",
              height: 6,
              borderRadius: 999,
              backgroundColor: "#020617",
              overflow: "hidden"
            }}
          >
            <div
              style={{
                width: `${progressPercent}%`,
                height: "100%",
                borderRadius: 999,
                background:
                  progressPercent === 100 ? "#22c55e" : "linear-gradient(90deg,#22c55e,#f97316)",
                transition: "width 0.3s ease-out"
              }}
            />
          </div>
        </div>

        <div
          style={{
            marginTop: "0.75rem",
            padding: "0.6rem 0.75rem",
            borderRadius: 8,
            backgroundColor: "#020617",
            fontSize: "0.78rem",
            maxHeight: 220,
            overflow: "auto"
          }}
        >
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: "0.4rem"
            }}
          >
            <strong style={{ color: "#9ca3af" }}>Top 文件列表</strong>
            <span style={{ fontSize: "0.7rem", color: "#6b7280" }}>
              {topFiles.length > 0
                ? `共 ${topFiles.length} 条`
                : "任务完成后将展示 summary.top_files / top_files"}
            </span>
          </div>
          {topFiles.length === 0 ? (
            <div style={{ fontSize: "0.75rem", color: "#6b7280" }}>
              暂无数据。
            </div>
          ) : (
            <ul
              style={{
                listStyle: "none",
                padding: 0,
                margin: 0,
                display: "flex",
                flexDirection: "column",
                gap: "0.35rem"
              }}
            >
              {topFiles.map((file, idx) => (
                <li
                  key={`${file.path}-${idx}`}
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "0.15rem",
                    padding: "0.35rem 0.4rem",
                    borderRadius: 6,
                    backgroundColor: "#020617",
                    border: "1px solid #1f2937"
                  }}
                >
                  <span
                    style={{
                      fontSize: "0.78rem",
                      color: "#e5e7eb",
                      wordBreak: "break-all"
                    }}
                  >
                    {file.path}
                  </span>
                  <span
                    style={{
                      fontSize: "0.7rem",
                      color: "#9ca3af"
                    }}
                  >
                    大小：{file.size_bytes ?? "未知"}
                    {file.last_modified && (
                      <>
                        <span style={{ marginLeft: "0.75rem" }}>
                          修改时间：{file.last_modified}
                        </span>
                      </>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>
    </main>
  );
};
