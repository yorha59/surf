import React, { useEffect, useState } from "react";
import { SurfConfig } from "../services/ServiceClient";

export interface SidebarProps {
  config: SurfConfig;
  onConfigChange: (config: SurfConfig) => void | Promise<void>;
}

export const Sidebar: React.FC<SidebarProps> = ({ config, onConfigChange }) => {
  const [draft, setDraft] = useState<SurfConfig>(config);
  const [saving, setSaving] = useState(false);

  // 当外部配置更新时，同步侧边栏表单。
  useEffect(() => {
    setDraft(config);
  }, [config]);

  const handleSave = async () => {
    if (saving) return;
    setSaving(true);
    try {
      await onConfigChange(draft);
    } finally {
      setSaving(false);
    }
  };

  return (
    <aside
      style={{
        width: 280,
        display: "flex",
        flexDirection: "column",
        padding: "1rem",
        gap: "1rem",
        backgroundColor: "#020617",
        borderRight: "1px solid #111827"
      }}
    >
      <section
        style={{
          borderRadius: 8,
          border: "1px solid #1f2937",
          padding: "0.75rem",
          display: "flex",
          flexDirection: "column",
          gap: "0.5rem",
          backgroundColor: "#020617"
        }}
      >
        <h2
          style={{
            fontSize: "0.85rem",
            fontWeight: 600,
            color: "#9ca3af",
            marginBottom: 2
          }}
        >
          全局配置（占位设置）
        </h2>
        <p style={{ fontSize: "0.75rem", color: "#6b7280", lineHeight: 1.5 }}>
          此处表单直连 <code>~/.config/surf/config.json</code>
          ，用于快速修改默认扫描路径与 JSON-RPC 地址。
        </p>

        <label
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 4,
            marginTop: "0.25rem"
          }}
        >
          <span style={{ fontSize: "0.8rem", color: "#e5e7eb" }}>默认扫描路径</span>
          <input
            type="text"
            value={draft.default_path}
            onChange={(e) =>
              setDraft((prev) => ({ ...prev, default_path: e.target.value }))
            }
            style={{
              padding: "0.4rem 0.55rem",
              borderRadius: 6,
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
            gap: "0.4rem",
            marginTop: "0.5rem",
            alignItems: "center"
          }}
        >
          <label style={{ flex: 1, display: "flex", flexDirection: "column", gap: 4 }}>
            <span style={{ fontSize: "0.8rem", color: "#e5e7eb" }}>RPC Host</span>
            <input
              type="text"
              value={draft.rpc_host}
              onChange={(e) =>
                setDraft((prev) => ({ ...prev, rpc_host: e.target.value }))
              }
              style={{
                padding: "0.4rem 0.55rem",
                borderRadius: 6,
                border: "1px solid #1f2937",
                backgroundColor: "#020617",
                color: "#e5e7eb",
                fontSize: "0.8rem"
              }}
            />
          </label>
          <label style={{ width: 90, display: "flex", flexDirection: "column", gap: 4 }}>
            <span style={{ fontSize: "0.8rem", color: "#e5e7eb" }}>Port</span>
            <input
              type="number"
              min={1}
              max={65535}
              value={draft.rpc_port}
              onChange={(e) => {
                const v = parseInt(e.target.value, 10);
                const port = Number.isFinite(v) ? Math.min(Math.max(v, 1), 65535) : 1234;
                setDraft((prev) => ({ ...prev, rpc_port: port }));
              }}
              style={{
                padding: "0.4rem 0.55rem",
                borderRadius: 6,
                border: "1px solid #1f2937",
                backgroundColor: "#020617",
                color: "#e5e7eb",
                fontSize: "0.8rem"
              }}
            />
          </label>
        </div>

        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          style={{
            marginTop: "0.6rem",
            alignSelf: "flex-end",
            padding: "0.35rem 0.9rem",
            borderRadius: 999,
            border: "1px solid #22c55e",
            background: saving ? "#064e3b" : "transparent",
            color: "#22c55e",
            fontSize: "0.78rem",
            cursor: saving ? "default" : "pointer",
            opacity: saving ? 0.8 : 1
          }}
        >
          {saving ? "保存中..." : "保存设置"}
        </button>
      </section>

      <section
        style={{
          flex: 1,
          borderRadius: 8,
          border: "1px dashed #1f2937",
          padding: "0.75rem",
          fontSize: "0.85rem",
          color: "#6b7280"
        }}
      >
        收藏路径与历史记录区仍为占位，后续迭代中将接入真实数据。
      </section>
    </aside>
  );
};
