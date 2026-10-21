import { FormEvent, useState } from "react";
import { login, Session } from "../api/auth";

interface Props {
  onSession: (session: Session) => void;
}

export default function LoginScreen({ onSession }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onSession(await login(email, password));
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <main className="center">
      <form className="card" onSubmit={submit}>
        <h1>Spaceport</h1>
        <p className="muted">Sign in with your Stremio account</p>
        <label>
          Email
          <input
            type="email"
            autoComplete="username"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoFocus
          />
        </label>
        <label>
          Password
          <input
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </label>
        {error && <p className="error">{error}</p>}
        <button type="submit" disabled={busy}>
          {busy ? "Signing in…" : "Sign in"}
        </button>
      </form>
    </main>
  );
}
