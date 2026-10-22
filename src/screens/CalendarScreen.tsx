import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LoadableList, useLoad } from "../components/Loadable";
import { ListSkeleton } from "../components/Skeleton";
import { useOpenTitle } from "../navigation";
import { useNewEpisodes } from "../newEpisodes";
import { readWatching } from "../watching";

/** One episode on the calendar, as the backend sends it. */
interface CalendarEntry {
  seriesId: string;
  type: string;
  seriesName: string;
  poster: string | null;
  videoId: string;
  season: number;
  episode: number | null;
  title: string;
  thumbnail: string | null;
  released: string;
}

const dayStart = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** "Today", "Tomorrow", "Yesterday", or the weekday and date. */
function dayLabel(time: number) {
  const days = Math.round((time - dayStart(new Date())) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Tomorrow";
  if (days === -1) return "Yesterday";
  return new Date(time).toLocaleDateString(undefined, {
    weekday: "long",
    day: "numeric",
    month: "long",
  });
}

/** Entries grouped by the local day they come out on, in date order. */
function byDay(entries: CalendarEntry[]) {
  const days = new Map<number, CalendarEntry[]>();
  for (const entry of entries) {
    const at = new Date(entry.released);
    if (Number.isNaN(at.getTime())) continue;
    const day = dayStart(at);
    days.set(day, [...(days.get(day) ?? []), entry]);
  }
  return [...days.entries()].sort(([a], [b]) => a - b);
}

/** Episodes of the series you follow: the last week and the next two months. */
export default function CalendarScreen() {
  const openTitle = useOpenTitle();
  const fresh = useNewEpisodes();
  const load = useCallback(() => {
    const tracked = readWatching().map((i) => ({ id: i.id, type: i.type, since: i.since ?? null }));
    return invoke<CalendarEntry[]>("calendar", { tracked });
  }, []);
  const [entries, reload] = useLoad(load);
  const today = dayStart(new Date());
  return (
    <section>
      <h2>Calendar</h2>
      <p className="muted small calendar-note">
        Series in your library, Continue Watching and marked Watching: the last week and the next
        two months.
      </p>
      <LoadableList
        state={entries}
        onRetry={reload}
        empty="No episodes of the series you follow come out around now."
        skeleton={<ListSkeleton rows={5} />}
      >
        {(list) => (
          <div className="calendar">
            {byDay(list).map(([day, items]) => (
              <section key={day} className={day === today ? "calendar-day today" : "calendar-day"}>
                <h3>{dayLabel(day)}</h3>
                <ul>
                  {items.map((e) => {
                    const isNew = fresh[e.seriesId]?.includes(e.videoId);
                    const upcoming = day > today;
                    return (
                      <li key={e.videoId}>
                        <button
                          className="calendar-entry"
                          onClick={() => openTitle({ type: e.type, id: e.seriesId })}
                        >
                          {e.thumbnail || e.poster ? (
                            <img
                              src={e.thumbnail ?? e.poster ?? undefined}
                              alt=""
                              loading="lazy"
                              referrerPolicy="no-referrer"
                            />
                          ) : (
                            <span className="calendar-thumb">{e.seriesName.slice(0, 1)}</span>
                          )}
                          <span className="calendar-text">
                            <strong>{e.seriesName}</strong>
                            <span className="muted">
                              S{e.season}
                              {e.episode != null && ` E${e.episode}`} · {e.title}
                            </span>
                          </span>
                          {isNew && <span className="new-tag">New</span>}
                          {upcoming && <span className="muted small">Upcoming</span>}
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </section>
            ))}
          </div>
        )}
      </LoadableList>
    </section>
  );
}
