const colors: Record<string, string> = {
  running: "bg-success/20 text-success",
  paused: "bg-warning/20 text-warning",
  stopped: "bg-danger/20 text-danger",
  completed: "bg-bg-elevated text-text-secondary",
};

export default function StatusBadge({ status }: { status: string }) {
  return (
    <span
      className={`px-2 py-0.5 rounded text-xs font-medium uppercase ${colors[status] || colors.completed}`}
    >
      {status}
    </span>
  );
}
