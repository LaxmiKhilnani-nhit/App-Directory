import { useEffect, useState } from "react";
import { readCoreInfo, type CoreInfo } from "./lib/core";
import { Login } from "./pages/Login";
import { Directory } from "./pages/Directory";

/**
 * Phase 0: no session yet. The app shows the login screen and reports whether it is
 * running inside Tauri with a reachable core. Phase 2 adds the real auth handshake and
 * Phase 5 swaps the placeholder grid for the synced snapshot.
 */
export function App() {
  const [core, setCore] = useState<CoreInfo | null>(null);
  const [signedIn, setSignedIn] = useState(false);

  useEffect(() => {
    void readCoreInfo().then(setCore);
  }, []);

  if (!signedIn) {
    return <Login core={core} onContinue={() => setSignedIn(true)} />;
  }

  return <Directory core={core} onSignOut={() => setSignedIn(false)} />;
}
