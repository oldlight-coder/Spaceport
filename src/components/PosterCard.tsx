import { TitleRef, useOpenTitle } from "../navigation";

interface Props {
  name: string;
  poster: string | null;
  shape?: string | null;
  title: TitleRef;
  /** Share watched, 0 to 1, drawn as a bar under the poster. */
  progress?: number | null;
  /** A second line, such as the episode to continue. */
  caption?: string | null;
}

export default function PosterCard({ name, poster, shape, title, progress, caption }: Props) {
  const openTitle = useOpenTitle();
  const className = shape === "landscape" ? "poster-card landscape" : "poster-card";
  return (
    <li className={className}>
      <button className="poster-button" title={name} onClick={() => openTitle(title)}>
        {poster ? (
          <img src={poster} alt="" loading="lazy" referrerPolicy="no-referrer" />
        ) : (
          <div className="poster-placeholder">{name.slice(0, 1)}</div>
        )}
        {progress != null && (
          <span className="progress-bar" aria-label={`${Math.round(progress * 100)}% watched`}>
            <span style={{ width: `${progress * 100}%` }} />
          </span>
        )}
        <span className="poster-title">{name}</span>
        {caption && <span className="poster-caption muted">{caption}</span>}
      </button>
    </li>
  );
}
