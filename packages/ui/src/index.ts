// ============================================================
// @myriad-mind/ui — 共享 React 组件入口
// ============================================================

// Common
export {
  Button,
  Card,
  Modal,
  Input,
  Textarea,
  Select,
  Toggle,
} from "./common/index.js";
export type {
  ButtonProps,
  ButtonVariant,
  ButtonSize,
  CardProps,
  ModalProps,
  InputProps,
  TextareaProps,
  SelectProps,
  SelectOption,
  ToggleProps,
} from "./common/index.js";

// ConfigWizard (首次启动引导)
export { ConfigWizard } from "./ConfigWizard.js";
export type { ConfigWizardProps } from "./ConfigWizard.js";

// SettingsPage (常规设置页)
export { SettingsPage } from "./SettingsPage.js";
export type { SettingsPageProps, ThemeMode } from "./SettingsPage.js";

// NoteRenderer
export { NoteRenderer, renderMarkdown } from "./NoteRenderer.js";
export type { NoteRendererProps } from "./NoteRenderer.js";

// Dashboard
export { Dashboard } from "./Dashboard.js";
export type { DashboardProps } from "./Dashboard.js";

// Types
export type {
  DepInfo,
  DepsInfo,
  SetupIntent,
  HealthStatus,
  HealthItem,
} from "./types.js";

// Version
export const UI_VERSION = "0.3.0";
