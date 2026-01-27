import React from "react";

export interface OnboardingProps {
  onComplete: () => void;
}

export const Onboarding: React.FC<OnboardingProps> = ({ onComplete }) => {
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        padding: "2rem",
        gap: "1.5rem",
        background: "linear-gradient(135deg, #0f172a, #020617)",
        color: "#e5e7eb"
      }}
    >
      <h1 style={{ fontSize: "2.25rem", fontWeight: 700 }}>欢迎使用 Surf</h1>
      <p style={{ maxWidth: 520, textAlign: "center", lineHeight: 1.6 }}>
        Surf 是一款面向开发者与高级用户的磁盘空间分析工具。
        当前版本的 macOS GUI 处于骨架阶段，本页面用于占位 Onboarding
        流程，包括权限申请与默认配置等后续功能。
      </p>
      <ul style={{ maxWidth: 520, textAlign: "left", lineHeight: 1.6 }}>
        <li>· 将来会引导授予全盘访问权限（Full Disk Access）。</li>
        <li>· 配置默认扫描路径、线程数与最小文件大小过滤。</li>
        <li>· 检查本地 Surf JSON-RPC 服务是否可用。</li>
      </ul>
      <button
        type="button"
        onClick={onComplete}
        style={{
          marginTop: "1rem",
          padding: "0.75rem 1.5rem",
          borderRadius: 999,
          border: "none",
          background: "#22c55e",
          color: "#020617",
          fontSize: "1rem",
          fontWeight: 600,
          cursor: "pointer"
        }}
      >
        开始使用 Surf
      </button>
    </div>
  );
};
