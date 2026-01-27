import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";

// 参考 Tauri 官方模板的 Vite 配置，当前仅用于开发与构建 React 前端。

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true
  },
  clearScreen: false
});
