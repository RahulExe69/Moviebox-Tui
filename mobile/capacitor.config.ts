import type { CapacitorConfig } from "@capacitor/cli";
const config: CapacitorConfig = {
  appId: "com.sevenx.cinema",
  appName: "7xCinema",
  webDir: "dist",
  server: { androidScheme: "https" },
  android: { allowMixedContent: true }
};
export default config;
