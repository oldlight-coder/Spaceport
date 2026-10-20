import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function App() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("app_version")
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  return (
    <main className="shell">
      <h1>Spaceport</h1>
      <p className="muted">{version ? `v${version}` : "Desktop Stremio client"}</p>
    </main>
  );
}
