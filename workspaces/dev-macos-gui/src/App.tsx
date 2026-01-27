import React, { useEffect, useState } from "react";
import { Onboarding } from "./components/Onboarding";
import { MainLayout } from "./components/MainLayout";
import {
  ServiceClientProvider,
  SurfConfig,
  createDefaultConfig,
  readConfig,
  writeConfig
} from "./services/ServiceClient";

const App: React.FC = () => {
  const [config, setConfig] = useState<SurfConfig | null>(null);
  const [loadingConfig, setLoadingConfig] = useState(true);
  const [isOnboarding, setIsOnboarding] = useState(false);

  // 应用启动时从统一路径 (~/.config/surf/config.json) 读取配置，
  // 若不存在或不可用则进入 Onboarding 流程。
  useEffect(() => {
    let cancelled = false;

    (async () => {
      const loaded = await readConfig();
      if (cancelled) return;

      if (loaded) {
        setConfig(loaded);
        setIsOnboarding(false);
      } else {
        // 未找到有效配置：使用一份基于当前环境推导的默认配置，进入 Onboarding。
        setConfig(createDefaultConfig());
        setIsOnboarding(true);
      }

      setLoadingConfig(false);
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const persistConfig = async (next: SurfConfig) => {
    setConfig(next);
    try {
      await writeConfig(next);
    } catch {
      // 写入失败在 `writeConfig` 内已有日志，这里不再额外打断 UI 流程。
    }
  };

  const handleOnboardingComplete = async (next: SurfConfig) => {
    await persistConfig(next);
    setIsOnboarding(false);
  };

  return (
    <ServiceClientProvider>
      <div
        style={{
          width: "100vw",
          height: "100vh",
          display: "flex",
          flexDirection: "column",
          fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif"
        }}
      >
        {loadingConfig || !config ? (
          <div
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              backgroundColor: "#020617",
              color: "#9ca3af",
              fontSize: "0.9rem"
            }}
          >
            正在加载 Surf 配置...
          </div>
        ) : isOnboarding ? (
          <Onboarding initialConfig={config} onComplete={handleOnboardingComplete} />
        ) : (
          <MainLayout config={config} onConfigChange={persistConfig} />
        )}
      </div>
    </ServiceClientProvider>
  );
};

export default App;
