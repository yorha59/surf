import React from "react";

export const TopBar: React.FC = () => {
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
            backgroundColor: "#22c55e"
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
          style={{
            padding: "0.4rem 0.9rem",
            borderRadius: 999,
            border: "1px solid #22c55e",
            background: "transparent",
            color: "#22c55e",
            fontSize: "0.8rem",
            cursor: "pointer"
          }}
        >
          扫描（占位）
        </button>
      </div>
    </header>
  );
};
