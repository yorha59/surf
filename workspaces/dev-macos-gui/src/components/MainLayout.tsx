import React from "react";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { CentralView } from "./CentralView";
import { SurfConfig } from "../services/ServiceClient";

export interface MainLayoutProps {
  config: SurfConfig;
  onConfigChange: (config: SurfConfig) => void | Promise<void>;
}

export const MainLayout: React.FC<MainLayoutProps> = ({
  config,
  onConfigChange
}) => {
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        overflow: "hidden",
        backgroundColor: "#020617",
        color: "#e5e7eb"
      }}
    >
      <Sidebar config={config} onConfigChange={onConfigChange} />
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          borderLeft: "1px solid #111827"
        }}
      >
        <TopBar />
        <CentralView />
      </div>
    </div>
  );
};
