import { createContext, useContext } from "react";

/** A title to open on the details page; `source` is the addon it was found in, if known. */
export interface TitleRef {
  type: string;
  id: string;
  source?: string;
}

export const OpenTitleContext = createContext<(title: TitleRef) => void>(() => {});

export const useOpenTitle = () => useContext(OpenTitleContext);

/** Searches all addons for a term, as if typed in the search box (cast and director names). */
export const SearchContext = createContext<(term: string) => void>(() => {});

export const useSearch = () => useContext(SearchContext);

/** Opens the settings page at its Addons section, for empty states that need an addon. */
export const OpenAddonsContext = createContext<() => void>(() => {});

export const useOpenAddons = () => useContext(OpenAddonsContext);
