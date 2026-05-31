import { cn } from "@/lib/utils";

// ---- Props ----

interface NavButtonProps {
  active: boolean;
  icon: string;
  label: string;
  hotkey: string;
  onClick: () => void;
}

// ---- Component ----

export function NavButton({ active, icon, label, hotkey, onClick }: NavButtonProps) {
  return (
    <button
      className={cn("nav-btn", active && "nav-btn-active")}
      onClick={onClick}
    >
      <span style={{ fontSize: 16 }}>{icon}</span>
      <span style={{ flex: 1 }}>{label}</span>
      <span style={{ fontSize: 10, opacity: 0.4 }}>{hotkey}</span>
    </button>
  );
}
