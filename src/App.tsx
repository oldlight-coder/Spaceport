import { useCallback, useEffect, useState } from "react";
import { restoreSession, Session } from "./api/auth";
import LoginScreen from "./screens/LoginScreen";
import { TitleBar } from "./components/WindowControls";
import { useRetryWhenOnline } from "./online";
import HomeScreen from "./screens/HomeScreen";

type View = { kind: "loading" } | { kind: "error"; message: string } | { kind: "session"; session: Session };

export default function App() {
  const [view, setView] = useState<View>({ kind: "loading" });

  const restore = useCallback(() => {
    setView({ kind: "loading" });
    restoreSession()
      .then((session) => setView({ kind: "session", session }))
      .catch((e) => setView({ kind: "error", message: String(e) }));
  }, []);

  useEffect(restore, [restore]);
  // Starting offline keeps the saved sign-in; try again as soon as the connection returns.
  useRetryWhenOnline(restore, view.kind === "error");

  const onSession = (session: Session) => setView({ kind: "session", session });

  switch (view.kind) {
    case "loading":
      return (
        <main className="center">
          <TitleBar />
          <div className="splash" role="status" aria-label="Loading">
            <span className="splash-mark" />
            Spaceport
          </div>
        </main>
      );
    case "error":
      return (
        <main className="center">
          <TitleBar />
          <div className="card">
            <p className="error">{view.message}</p>
            <button onClick={restore}>Retry</button>
          </div>
        </main>
      );
    case "session":
      return view.session.state === "loggedIn" ? (
        <HomeScreen user={view.session.user} onSession={onSession} />
      ) : (
        <>
          <TitleBar />
          <LoginScreen onSession={onSession} />
        </>
      );
  }
}
