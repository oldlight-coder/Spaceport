import { useState } from "react";
import { logout, Session, User } from "../api/auth";

interface Props {
  user: User;
  onSession: (session: Session) => void;
}

export default function HomeScreen({ user, onSession }: Props) {
  const [busy, setBusy] = useState(false);

  async function signOut() {
    setBusy(true);
    try {
      onSession(await logout());
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">Spaceport</span>
        <span className="muted">{user.email}</span>
        <button className="secondary" onClick={signOut} disabled={busy}>
          Sign out
        </button>
      </header>
      <main className="content">
        <p className="muted">Signed in.</p>
      </main>
    </div>
  );
}
