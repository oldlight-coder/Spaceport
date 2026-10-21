// Placeholders shaped like the content that is loading, with a soft shimmer.

const range = (n: number) => Array.from({ length: n }, (_, i) => i);

/** A grey block; `className` gives it its shape. */
export function Bone({ className = "" }: { className?: string }) {
  return <span className={`bone ${className}`} aria-hidden="true" />;
}

/** One catalog row: a title bar and a line of posters. */
export function PosterRowSkeleton({ heading = true }: { heading?: boolean }) {
  return (
    <div className="skeleton-row" aria-hidden="true">
      {heading && <Bone className="bone-heading" />}
      <div className="poster-row skeleton-posters">
        {range(8).map((i) => (
          <div key={i} className="poster-card">
            <Bone className="bone-poster" />
            <Bone className="bone-line" />
          </div>
        ))}
      </div>
    </div>
  );
}

/** Several catalog rows, for pages made of rows. */
export function RowsSkeleton({ rows = 3 }: { rows?: number }) {
  return (
    <div className="rows" role="status" aria-label="Loading">
      {range(rows).map((i) => (
        <PosterRowSkeleton key={i} />
      ))}
    </div>
  );
}

/** A grid of posters. */
export function PosterGridSkeleton({ count = 12 }: { count?: number }) {
  return (
    <ul className="poster-grid" role="status" aria-label="Loading">
      {range(count).map((i) => (
        <li key={i} className="poster-card">
          <Bone className="bone-poster" />
          <Bone className="bone-line" />
        </li>
      ))}
    </ul>
  );
}

/** Stacked cards, for lists such as streams or addons. */
export function ListSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="skeleton-list" role="status" aria-label="Loading">
      {range(rows).map((i) => (
        <div key={i} className="skeleton-card">
          <Bone className="bone-line wide" />
          <Bone className="bone-line" />
          <Bone className="bone-line short" />
        </div>
      ))}
    </div>
  );
}

/** A title page: poster, title, a few lines and episode rows. */
export function DetailsSkeleton() {
  return (
    <div className="skeleton-details" role="status" aria-label="Loading">
      <div className="details-head">
        <Bone className="bone-details-poster" />
        <div className="details-text skeleton-text">
          <Bone className="bone-title" />
          <Bone className="bone-line short" />
          <Bone className="bone-line wide" />
          <Bone className="bone-line wide" />
          <Bone className="bone-line" />
        </div>
      </div>
      <ListSkeleton rows={3} />
    </div>
  );
}
