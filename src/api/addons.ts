import { invoke } from "@tauri-apps/api/core";

export interface Addon {
  id: string;
  name: string;
  version: string;
  description: string | null;
  logo: string | null;
  transportUrl: string;
  official: boolean;
  protected: boolean;
  configurable: boolean;
}

export const listAddons = () => invoke<Addon[]>("addons_list");

export const installAddon = (url: string) => invoke<Addon[]>("addons_install", { url });

export const removeAddon = (transportUrl: string) =>
  invoke<Addon[]>("addons_remove", { transportUrl });

export const reorderAddons = (order: string[]) => invoke<Addon[]>("addons_reorder", { order });

export const configureAddon = (transportUrl: string) =>
  invoke<void>("addons_configure", { transportUrl });
