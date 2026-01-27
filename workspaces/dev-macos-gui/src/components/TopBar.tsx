import React, { useCallback, useState } from "react";
import {
  scanResult,
  scanStart,
  scanStatus,
  useServiceClient
} from "../services/ServiceClient";

export const TopBar: React.FC = () => {
  const { serviceState } = useServiceClient();
  const indicatorColor = serviceState.connected ? "#22c55e" : "#f97316";
  const indicatorLabel = serviceState.connected ? "已连接 JSON-RPC" : "未连接 JSON-RPC";

  const [demoRunning, setDemoRunning] = useState(false);
  const [demoError, setDemoError] = useState<string | null>(null);
  const [demoSummary, setDemoSummary] = useState<
    { totalSize: number; topFiles: number } | null
  >(null);

  const handleDemoScan = useCallback(async () => {
    if (demoRunning) return;
    setDemoError(null);
    setDemoSummary(null);
    setDemoRunning(true);

    try {
      // 使用最小联调参数：扫描当前工作目录，限制 Top 文件数量为 10。
      const start = await scanStart({ path: ".", limit: 10 });
      const taskId = start.task_id;

      console.log("[surf gui] demo scanStart task_id=", taskId);

      let finalStatus: Awaited<ReturnType<typeof scanStatus>> | null = null;
      for (let i = 0; i < 30; i++) {
        const status = await scanStatus(taskId);
        finalStatus = status as any;
        const state = (status.state || "").toString().toLowerCase();
        console.log("[surf gui] demo scanStatus", { state, status });

        if (state === "completed" || state === "failed" || state === "canceled") {
          break;
        }

        await new Promise((resolve) => setTimeout(resolve, 1000));
      }

      if (!finalStatus) {
        setDemoError("联调轮询在超时时间内未获得任务状态");
        return;
      }

      const finalState = (finalStatus.state || "").toString().toLowerCase();
      if (finalState !== "completed") {
        setDemoError(`联调任务未完成，最终状态：${finalState}`);
        return;
      }

      const result = await scanResult(taskId);
      const totalSize = result.summary?.total_size_bytes ?? 0;
      const topFilesCount = result.top_files?.length ?? 0;

      setDemoSummary({ totalSize, topFiles: topFilesCount });
      console.log("[surf gui] demo scanResult", {
        totalSize,
        topFilesCount,
        raw: result
      });
    } catch (e: any) {
      console.error("[surf gui] demo scan failed", e);
      setDemoError(e?.message ?? String(e));
    } finally {
      setDemoRunning(false);
    }
  }, [demoRunning]);

  return (
    <header
      style={{
        height: 56,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "0 1rem",
        borderBottom: "1px solid #111827",
        background: "rgba(15,23,42,0.95)",
        backdropFilter: "blur(12px)"
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
        <span
          style={{
            width: 10,
            height: 10,
            borderRadius: 999,
            backgroundColor: indicatorColor
          }}
        />
        <div>
          <div style={{ fontSize: "0.95rem", fontWeight: 600 }}>Surf</div>
          <div style={{ fontSize: "0.7rem", color: "#6b7280" }}>
            macOS GUI · 主界面骨架
          </div>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
        <span style={{ fontSize: "0.7rem", color: "#9ca3af" }}>
          {indicatorLabel}
        </span>
        <input
          type="text"
          placeholder="搜索（占位）"
          style={{
            width: 200,
            padding: "0.35rem 0.6rem",
            borderRadius: 999,
            border: "1px solid #1f2937",
            backgroundColor: "#020617",
            color: "#e5e7eb",
            fontSize: "0.8rem"
          }}
        />
        <button
          type="button"
          onClick={handleDemoScan}
          style={{
            padding: "0.4rem 0.9rem",
            borderRadius: 999,
            border: "1px solid #22c55e",
            background: demoRunning ? "#064e3b" : "transparent",
            color: "#22c55e",
            fontSize: "0.8rem",
            cursor: demoRunning ? "default" : "pointer",
            opacity: demoRunning ? 0.7 : 1
          }}
        >
          {demoRunning ? "开始扫描（联调中...）" : "开始扫描（联调）"}
        </button>
        {demoSummary && !demoError && (
          <span style={{ fontSize: "0.7rem", color: "#6b7280" }}>
            Demo：总大小 {demoSummary.totalSize} bytes · TopFiles {demoSummary.topFiles}
          </span>
        )}
        {demoError && (
          <span style={{ fontSize: "0.7rem", color: "#fca5a5" }}>
            联调错误：{demoError}
          </span>
        )}
      </div>
    </header>
  );
};
