interface Props {
  name: string;
  poster: string | null;
  shape?: string | null;
}

export default function PosterCard({ name, poster, shape }: Props) {
  const className = shape === "landscape" ? "poster-card landscape" : "poster-card";
  return (
    <li className={className} title={name}>
      {poster ? (
        <img src={poster} alt="" loading="lazy" referrerPolicy="no-referrer" />
      ) : (
        <div className="poster-placeholder">{name.slice(0, 1)}</div>
      )}
      <span className="poster-title">{name}</span>
    </li>
  );
}
