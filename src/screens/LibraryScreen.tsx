import { listLibrary } from "../api/library";
import { LoadableList, useLoad } from "../components/Loadable";
import PosterCard from "../components/PosterCard";

export default function LibraryScreen() {
  const [library, reload] = useLoad(listLibrary);
  return (
    <section>
      <h2>Library</h2>
      <LoadableList state={library} onRetry={reload} empty="Your library is empty.">
        {(items) => (
          <ul className="poster-grid">
            {items.map((item) => (
              <PosterCard
                key={item.id}
                name={item.name || item.id}
                poster={item.poster}
                title={{ type: item.type, id: item.id }}
              />
            ))}
          </ul>
        )}
      </LoadableList>
    </section>
  );
}
