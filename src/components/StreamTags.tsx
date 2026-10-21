import { ParsedStream } from "../api/streams";

const languageNames = (() => {
  try {
    return new Intl.DisplayNames(["en"], { type: "language" });
  } catch {
    return null;
  }
})();

const languageName = (code: string) => languageNames?.of(code) ?? code.toUpperCase();

export function formatSize(bytes: number) {
  const gb = bytes / 1024 ** 3;
  return gb >= 1 ? `${gb.toFixed(2)} GB` : `${(bytes / 1024 ** 2).toFixed(0)} MB`;
}

const resolutionLabel = (r: number) => (r >= 2160 ? "4K" : `${r}p`);

/** Short tags for what the parser read out of a stream's labels. */
export default function StreamTags({ parsed }: { parsed: ParsedStream }) {
  const tags: { text: string; tone?: "good" | "warn" }[] = [];
  if (parsed.cached === true) {
    tags.push({ text: parsed.debrid ? `Cached ${parsed.debrid}` : "Cached", tone: "good" });
  }
  if (parsed.cached === false) tags.push({ text: "Not cached", tone: "warn" });
  if (parsed.resolution) tags.push({ text: resolutionLabel(parsed.resolution) });
  for (const h of parsed.hdr) tags.push({ text: h });
  if (parsed.source) {
    tags.push({ text: parsed.source, tone: parsed.source === "CAM" ? "warn" : undefined });
  }
  if (parsed.codec) {
    const depth = parsed.bitDepth ? ` ${parsed.bitDepth}-bit` : "";
    tags.push({ text: parsed.codec + depth });
  }
  if (parsed.audio.length > 0 || parsed.channels) {
    tags.push({ text: [parsed.audio.join(" "), parsed.channels].filter(Boolean).join(" ") });
  }
  if (parsed.sizeBytes) tags.push({ text: formatSize(parsed.sizeBytes) });
  if (parsed.seeders != null) tags.push({ text: `${parsed.seeders} seeders` });
  if (parsed.languages.length > 0) {
    tags.push({ text: parsed.languages.map(languageName).join(" / ") });
  } else if (parsed.multiAudio) {
    tags.push({ text: "Multi audio" });
  }
  if (parsed.releaseGroup) tags.push({ text: parsed.releaseGroup });

  if (tags.length === 0) return null;
  return (
    <ul className="stream-tags">
      {tags.map((t, i) => (
        <li key={i} className={t.tone}>
          {t.text}
        </li>
      ))}
    </ul>
  );
}
