import { LibraryItem, listLibrary } from "../api/library";
import { LoadableList, useLoad } from "../components/Loadable";
import PosterCard from "../components/PosterCard";

// Movies first, then series, then any other type an addon uses.
const ORDER = ["movie", "series"];
const HEADINGS: Record<string, string> = { movie: "Movies", series: "Series" };

/** Library items grouped by type, keeping the library's order inside each group. */
function byType(items: LibraryItem[]) {
  const groups = new Map<string, LibraryItem[]>();
  for (const item of items) {
    const list = groups.get(item.type) ?? [];
    list.push(item);
    groups.set(item.type, list);
  }
  const rank = (type: string) => (ORDER.includes(type) ? ORDER.indexOf(type) : ORDER.length);
  return [...groups.entries()]
    .sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b))
    .map(([type, list]) => ({
      type,
      heading: HEADINGS[type] ?? type.charAt(0).toUpperCase() + type.slice(1),
      items: list,
    }));
}

export default function LibraryScreen() {
  const [library, reload] = useLoad(listLibrary);
  return (
    <section>
      <h2>Library</h2>
      <LoadableList state={library} onRetry={reload} empty="Your library is empty.">
        {(items) =>
          byType(items).map((group) => (
            <section key={group.type} className="library-group">
              <h3>
                {group.heading} <span className="muted">{group.items.length}</span>
              </h3>
              <ul className="poster-grid">
                {group.items.map((item) => (
                  <PosterCard
                    key={item.id}
                    name={item.name || item.id}
                    poster={item.poster}
                    title={{ type: item.type, id: item.id }}
                  />
                ))}
              </ul>
            </section>
          ))
        }
      </LoadableList>
    </section>
  );
}
