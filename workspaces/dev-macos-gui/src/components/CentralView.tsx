import React from "react";
import { useServiceClient } from "../services/ServiceClient";

export const CentralView: React.FC = () => {
  const { serviceState } = useServiceClient();

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
          列表视图（占位）
        </h2>
        <p style={{ fontSize: "0.8rem", color: "#6b7280" }}>
          未来将在此按目录层级展示扫描结果列表，可按名称、大小、修改时间排序与过滤。
        </p>
        <div
          style={{
            marginTop: "0.75rem",
            padding: "0.6rem 0.75rem",
            borderRadius: 8,
            backgroundColor: "#0f172a",
            fontSize: "0.78rem",
            lineHeight: 1.6
          }}
        >
          <strong style={{ color: "#f97316" }}>服务连接状态：</strong>
          <span style={{ marginLeft: "0.35rem" }}>{serviceState.statusText}</span>
          <div style={{ marginTop: "0.25rem", color: "#9ca3af" }}>
            {serviceState.connected ? (
              <span>后续将实时展示来自 JSON-RPC 服务的扫描任务与结果。</span>
            ) : (
              <span>
                当前未检测到正在运行的 Surf JSON-RPC 服务（占位逻辑）。请在本机
                127.0.0.1:1234 启动服务后，后续迭代将接入真实连接检查。
              </span>
            )}
          </div>
        </div>
      </section>
    </main>
  );
};
