import SystemStatus from "../features/system/SystemStatus";

export default function App() {
  return (
    <main
      style={{
        minHeight: "100vh",
        padding: "2rem",
        display: "flex",
        flexDirection: "column",
        gap: "1.5rem",
      }}
    >
      <h1
        style={{
          fontSize: "1.5rem",
          fontWeight: 600,
          color: "var(--color-text)",
        }}
      >
        Manga Downloader 1989
      </h1>
      <SystemStatus />
    </main>
  );
}
