import React, { useMemo, useState } from "react";
import { SurfConfig } from "../services/ServiceClient";

export interface OnboardingProps {
  /**
   * 从外部推导的一份初始配置，用于预填表单，例如默认线程数与 JSON-RPC 地址。
   */
  initialConfig: SurfConfig;
  /** Onboarding 完成后的回调，由上层负责持久化配置。 */
  onComplete: (config: SurfConfig) => void | Promise<void>;
}

export const Onboarding: React.FC<OnboardingProps> = ({
  initialConfig,
  onComplete
}) => {
  const [form, setForm] = useState<SurfConfig>(() => ({ ...initialConfig }));
  const [saving, setSaving] = useState(false);

  const threadsHint = useMemo(() => {
    if (typeof navigator !== "undefined" && navigator.hardwareConcurrency) {
      return `建议值：${navigator.hardwareConcurrency}（当前逻辑核心数）`;
    }
    return "可根据机器性能自行调整";
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (saving) return;
    setSaving(true);
    try {
      await onComplete(form);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        padding: "2rem",
        background: "linear-gradient(135deg, #0f172a, #020617)",
        color: "#e5e7eb"
      }}
    >
      <div
        style={{
          width: "100%",
          maxWidth: 720,
          borderRadius: 16,
          border: "1px solid #1f2937",
          backgroundColor: "rgba(15,23,42,0.9)",
          boxShadow: "0 20px 40px rgba(15,23,42,0.6)",
          padding: "1.75rem",
          display: "flex",
          flexDirection: "column",
          gap: "1.25rem"
        }}
      >
        <header>
          <h1 style={{ fontSize: "2rem", fontWeight: 700, marginBottom: 8 }}>
            欢迎使用 Surf
          </h1>
          <p style={{ fontSize: "0.95rem", color: "#9ca3af", lineHeight: 1.6 }}>
            首次启动时，我们会为你初始化一份全局配置文件
            <code style={{ marginLeft: 4, marginRight: 4 }}>~/.config/surf/config.json</code>
            ，后续 GUI、CLI 与服务都会读取该配置作为默认行为。
          </p>
        </header>

        <form
          onSubmit={handleSubmit}
          style={{ display: "flex", flexDirection: "column", gap: "1rem" }}
        >
          <section
            style={{
              display: "grid",
              gridTemplateColumns: "minmax(0, 1.1fr) minmax(0, 0.9fr)",
              gap: "1rem"
            }}
          >
            <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
              <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span style={{ fontSize: "0.85rem", color: "#e5e7eb" }}>
                  默认扫描路径
                </span>
                <input
                  type="text"
                  value={form.default_path}
                  onChange={(e) =>
                    setForm((prev) => ({ ...prev, default_path: e.target.value }))
                  }
                  placeholder="例如 ~/ 或 /Users/you"
                  style={{
                    padding: "0.45rem 0.6rem",
                    borderRadius: 8,
                    border: "1px solid #1f2937",
                    backgroundColor: "#020617",
                    color: "#e5e7eb",
                    fontSize: "0.85rem"
                  }}
                />
                <span style={{ fontSize: "0.75rem", color: "#9ca3af" }}>
                  将作为 GUI / CLI 默认扫描路径，可在设置中再次修改。
                </span>
              </label>

              <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span style={{ fontSize: "0.85rem", color: "#e5e7eb" }}>
                  默认线程数
                </span>
                <input
                  type="number"
                  min={1}
                  value={form.threads}
                  onChange={(e) => {
                    const v = parseInt(e.target.value, 10);
                    setForm((prev) => ({ ...prev, threads: Number.isFinite(v) ? v : 1 }));
                  }}
                  style={{
                    padding: "0.45rem 0.6rem",
                    borderRadius: 8,
                    border: "1px solid #1f2937",
                    backgroundColor: "#020617",
                    color: "#e5e7eb",
                    fontSize: "0.85rem"
                  }}
                />
                <span style={{ fontSize: "0.75rem", color: "#9ca3af" }}>{threadsHint}</span>
              </label>

              <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span style={{ fontSize: "0.85rem", color: "#e5e7eb" }}>
                  最小过滤大小
                </span>
                <input
                  type="text"
                  value={form.min_size}
                  onChange={(e) =>
                    setForm((prev) => ({ ...prev, min_size: e.target.value }))
                  }
                  placeholder="例如 100MB 或 0 表示不过滤"
                  style={{
                    padding: "0.45rem 0.6rem",
                    borderRadius: 8,
                    border: "1px solid #1f2937",
                    backgroundColor: "#020617",
                    color: "#e5e7eb",
                    fontSize: "0.85rem"
                  }}
                />
                <span style={{ fontSize: "0.75rem", color: "#9ca3af" }}>
                  与 CLI `--min-size` 语义一致，支持 B/KB/MB/GB 后缀。
                </span>
              </label>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
              <div style={{ fontSize: "0.8rem", color: "#9ca3af" }}>
                <div style={{ marginBottom: 4 }}>JSON-RPC 服务地址</div>
                <span>
                  GUI 将通过 <code style={{ margin: "0 2px" }}>/rpc</code>
                  访问本地服务，默认映射到
                  <code style={{ marginLeft: 4 }}>http://127.0.0.1:1234/rpc</code>
                </span>
              </div>

              <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span style={{ fontSize: "0.85rem", color: "#e5e7eb" }}>Host</span>
                <input
                  type="text"
                  value={form.rpc_host}
                  onChange={(e) =>
                    setForm((prev) => ({ ...prev, rpc_host: e.target.value }))
                  }
                  style={{
                    padding: "0.45rem 0.6rem",
                    borderRadius: 8,
                    border: "1px solid #1f2937",
                    backgroundColor: "#020617",
                    color: "#e5e7eb",
                    fontSize: "0.85rem"
                  }}
                />
              </label>

              <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <span style={{ fontSize: "0.85rem", color: "#e5e7eb" }}>Port</span>
                <input
                  type="number"
                  min={1}
                  max={65535}
                  value={form.rpc_port}
                  onChange={(e) => {
                    const v = parseInt(e.target.value, 10);
                    const port = Number.isFinite(v) ? Math.min(Math.max(v, 1), 65535) : 1234;
                    setForm((prev) => ({ ...prev, rpc_port: port }));
                  }}
                  style={{
                    padding: "0.45rem 0.6rem",
                    borderRadius: 8,
                    border: "1px solid #1f2937",
                    backgroundColor: "#020617",
                    color: "#e5e7eb",
                    fontSize: "0.85rem"
                  }}
                />
              </label>
            </div>
          </section>

          <footer
            style={{
              marginTop: "0.5rem",
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              gap: "0.75rem"
            }}
          >
            <div style={{ fontSize: "0.78rem", color: "#6b7280" }}>
              上述配置将写入 <code>~/.config/surf/config.json</code>
              ，GUI、服务与 CLI 都会共享该配置。
            </div>
            <button
              type="submit"
              disabled={saving}
              style={{
                padding: "0.7rem 1.6rem",
                borderRadius: 999,
                border: "none",
                background: saving ? "#16a34a" : "#22c55e",
                color: "#020617",
                fontSize: "0.95rem",
                fontWeight: 600,
                cursor: saving ? "default" : "pointer",
                opacity: saving ? 0.8 : 1
              }}
            >
              {saving ? "正在写入配置..." : "开始使用 Surf"}
            </button>
          </footer>
        </form>
      </div>
    </div>
  );
};
