import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 开发时通过该环境变量支持局域网/移动端调试（对齐 jpcg）。
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // src-tauri 由 Tauri CLI 管理，避免前端热更新反复触发。
      ignored: ["**/src-tauri/**"],
    },
  },
});
