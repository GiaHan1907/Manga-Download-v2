import { useId, useState } from "react";

export type TabItem = {
  id: string;
  label: string;
  content: React.ReactNode;
};

export type TabsProps = {
  items: TabItem[];
  defaultValue?: string;
};

export default function Tabs({ items, defaultValue }: TabsProps) {
  const generatedId = useId();
  const [activeId, setActiveId] = useState(defaultValue ?? items[0]?.id);
  const active = items.find((item) => item.id === activeId) ?? items[0];

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    const currentIndex = items.findIndex((item) => item.id === active?.id);
    if (currentIndex < 0) {
      return;
    }

    if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
      event.preventDefault();
      const direction = event.key === "ArrowRight" ? 1 : -1;
      const nextIndex = (currentIndex + direction + items.length) % items.length;
      setActiveId(items[nextIndex].id);
    }
  }

  if (!active) {
    return null;
  }

  return (
    <div className="tabs">
      <div className="tabs__list" role="tablist" aria-label="Nội dung theo tab" onKeyDown={handleKeyDown}>
        {items.map((item) => {
          const selected = item.id === active.id;
          return (
            <button
              key={item.id}
              className="tabs__tab"
              type="button"
              role="tab"
              aria-selected={selected}
              aria-controls={`${generatedId}-${item.id}`}
              id={`${generatedId}-${item.id}-tab`}
              tabIndex={selected ? 0 : -1}
              onClick={() => setActiveId(item.id)}
            >
              {item.label}
            </button>
          );
        })}
      </div>
      <div
        id={`${generatedId}-${active.id}`}
        role="tabpanel"
        aria-labelledby={`${generatedId}-${active.id}-tab`}
        style={{ marginTop: "var(--space-4)" }}
      >
        {active.content}
      </div>
    </div>
  );
}
