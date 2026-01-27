import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";

// 参考 Tauri 官方模板的 Vite 配置，当前仅用于开发与构建 React 前端。

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    // 为前端提供到本地 JSON-RPC 服务 (127.0.0.1:1234) 的代理，避免浏览器跨域限制。
    proxy: {
      "/rpc": {
        target: "http://127.0.0.1:1234",
        changeOrigin: true,
        rewrite: (path) => path
      }
    }
  },
  clearScreen: false
});
