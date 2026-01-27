import React from "react";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { CentralView } from "./CentralView";

export const MainLayout: React.FC = () => {
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
      <Sidebar />
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
