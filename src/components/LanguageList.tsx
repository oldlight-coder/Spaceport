import { LANGUAGE_CODES, languageName } from "../api/settings";

interface Props {
  label: string;
  value: string[];
  onChange: (value: string[]) => void;
}

/** An ordered list of languages: first is preferred most. */
export default function LanguageList({ label, value, onChange }: Props) {
  const available = LANGUAGE_CODES.filter((c) => !value.includes(c));

  function move(index: number, delta: number) {
    const next = [...value];
    const [item] = next.splice(index, 1);
    next.splice(index + delta, 0, item);
    onChange(next);
  }

  return (
    <div className="setting">
      <span className="setting-label">{label}</span>
      <ol className="language-list">
        {value.map((code, i) => (
          <li key={code}>
            <span className="muted">{i + 1}.</span> {languageName(code)}
            <span className="row-actions">
              <button
                className="secondary icon"
                onClick={() => move(i, -1)}
                disabled={i === 0}
                aria-label={`Move ${languageName(code)} up`}
              >
                ↑
              </button>
              <button
                className="secondary icon"
                onClick={() => move(i, 1)}
                disabled={i === value.length - 1}
                aria-label={`Move ${languageName(code)} down`}
              >
                ↓
              </button>
              <button
                className="secondary icon"
                onClick={() => onChange(value.filter((c) => c !== code))}
                aria-label={`Remove ${languageName(code)}`}
              >
                ✕
              </button>
            </span>
          </li>
        ))}
      </ol>
      <select
        value=""
        onChange={(e) => e.target.value && onChange([...value, e.target.value])}
        aria-label={`Add to ${label}`}
      >
        <option value="">Add a language…</option>
        {available
          .map((c) => ({ c, name: languageName(c) }))
          .sort((a, b) => a.name.localeCompare(b.name))
          .map(({ c, name }) => (
            <option key={c} value={c}>
              {name}
            </option>
          ))}
      </select>
    </div>
  );
}
