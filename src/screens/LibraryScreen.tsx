import { LibraryItem, listLibrary } from "../api/library";
import { LoadableList, useLoad } from "../components/Loadable";

export default function LibraryScreen() {
  const [library, reload] = useLoad(listLibrary);
  return (
    <section>
      <h2>Library</h2>
      <LoadableList state={library} onRetry={reload} empty="Your library is empty.">
        {(items) => <LibraryGrid items={items} />}
      </LoadableList>
    </section>
  );
}

function LibraryGrid({ items }: { items: LibraryItem[] }) {
  return (
    <ul className="poster-grid">
      {items.map((item) => (
        <li key={item.id} className="poster-card" title={item.name}>
          {item.poster ? (
            <img src={item.poster} alt="" loading="lazy" referrerPolicy="no-referrer" />
          ) : (
            <div className="poster-placeholder">{item.name.slice(0, 1)}</div>
          )}
          <span className="poster-title">{item.name || item.id}</span>
        </li>
      ))}
    </ul>
  );
}
