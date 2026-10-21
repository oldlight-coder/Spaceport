import { invoke } from "@tauri-apps/api/core";

export interface User {
  _id: string;
  email: string;
}

export type Session = { state: "loggedOut" } | { state: "loggedIn"; user: User };

export const restoreSession = () => invoke<Session>("auth_restore");

export const login = (email: string, password: string) =>
  invoke<Session>("auth_login", { email, password });

export const logout = () => invoke<Session>("auth_logout");
