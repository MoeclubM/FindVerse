import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const controlApiProxy =
  process.env.FINDVERSE_CONTROL_API_PROXY ?? "http://127.0.0.1:8080";
const queryApiProxy =
  process.env.FINDVERSE_QUERY_API_PROXY ?? "http://127.0.0.1:8081";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: "0.0.0.0",
    port: 3000,
    proxy: {
      "/api": {
        target: controlApiProxy,
        changeOrigin: true,
        router: (request) => {
          const url = request.url ?? "";
          if (
            url.startsWith("/api/v1/search") ||
            url.startsWith("/api/v1/suggest") ||
            url.startsWith("/api/v1/developer/search")
          ) {
            return queryApiProxy;
          }
          return controlApiProxy;
        },
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
});
