import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri 约定：固定端口 1420，禁止占用失败后漂移
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // cargo 编译产物变化频繁且被文件锁占用，禁止 vite watch
    watch: {
      ignored: ["**/src-tauri/target/**", "**/dist/**"],
    },
  },
  build: {
    outDir: "dist",
    target: "chrome105",
  },
});
