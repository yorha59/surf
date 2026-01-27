import React from "react";

export const Sidebar: React.FC = () => {
  return (
    <aside
      style={{
        width: 260,
        display: "flex",
        flexDirection: "column",
        padding: "1rem",
        gap: "1rem",
        backgroundColor: "#020617",
        borderRight: "1px solid #111827"
      }}
    >
      <h2 style={{ fontSize: "0.9rem", fontWeight: 600, color: "#9ca3af" }}>
        收藏路径（占位）
      </h2>
      <div
        style={{
          flex: 1,
          borderRadius: 8,
          border: "1px dashed #1f2937",
          padding: "0.75rem",
          fontSize: "0.85rem",
          color: "#6b7280"
        }}
      >
        未来将在此展示常用扫描路径、最近扫描记录和设置入口。
      </div>
    </aside>
  );
};
