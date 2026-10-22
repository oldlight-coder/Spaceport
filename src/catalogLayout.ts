import { createContext, useContext } from "react";
import { CatalogRef } from "./api/catalog";
import { CatalogLayout } from "./api/settings";

/**
 * How a catalog is named in the saved layout: per page ("home", "movie", ...), so Home and the
 * type pages are arranged separately, and never by its link, which can hold keys.
 */
export const catalogKey = (page: string, c: CatalogRef) => `${page}:${c.addonId}/${c.type}/${c.id}`;

/**
 * The catalogs in the user's order: listed ones first, in their saved order, then the rest in
 * the addons' order. Hidden ones are left out unless `withHidden` (for the editor).
 */
export function arrange(
  page: string,
  catalogs: CatalogRef[],
  layout: CatalogLayout | undefined,
  withHidden = false,
) {
  const order = layout?.order ?? [];
  const hidden = new Set(layout?.hidden ?? []);
  const rank = (c: CatalogRef) => {
    const i = order.indexOf(catalogKey(page, c));
    return i === -1 ? order.length : i;
  };
  return catalogs
    .map((c, i) => ({ c, i }))
    .filter(({ c }) => withHidden || !hidden.has(catalogKey(page, c)))
    .sort((a, b) => rank(a.c) - rank(b.c) || a.i - b.i)
    .map(({ c }) => c);
}

/** The saved layout and a way to change it, from the home screen. */
export const CatalogLayoutContext = createContext<{
  layout: CatalogLayout | undefined;
  save: (layout: CatalogLayout) => Promise<void>;
}>({ layout: undefined, save: async () => {} });

export const useCatalogLayout = () => useContext(CatalogLayoutContext);
