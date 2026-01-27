import React, { useState } from "react";
import { Onboarding } from "./components/Onboarding";
import { MainLayout } from "./components/MainLayout";
import { ServiceClientProvider } from "./services/ServiceClient";

const App: React.FC = () => {
  const [hasOnboarded, setHasOnboarded] = useState(false);

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
        {hasOnboarded ? (
          <MainLayout />
        ) : (
          <Onboarding onComplete={() => setHasOnboarded(true)} />
        )}
      </div>
    </ServiceClientProvider>
  );
};

export default App;
