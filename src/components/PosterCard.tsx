import { TitleRef, useOpenTitle } from "../navigation";

interface Props {
  name: string;
  poster: string | null;
  shape?: string | null;
  title: TitleRef;
}

export default function PosterCard({ name, poster, shape, title }: Props) {
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
        <span className="poster-title">{name}</span>
      </button>
    </li>
  );
}
