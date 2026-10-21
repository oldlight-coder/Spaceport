import { useOpenAddons } from "../navigation";

/** An empty state that only an addon can fix, with a way to the addon settings. */
export default function NeedAddons({ text }: { text: string }) {
  const openAddons = useOpenAddons();
  return (
    <div className="inline-error">
      <p className="muted">{text}</p>
      <button className="secondary" onClick={openAddons}>
        Manage addons
      </button>
    </div>
  );
}
