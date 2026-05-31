// ---- 平台检测 ----
// 轻量检测，避免在非 Tauri 环境报错

let _isTauri: boolean | null = null;

export async function isTauri(): Promise<boolean> {
  if (_isTauri !== null) return _isTauri;
  try {
    await import("@tauri-apps/api/core");
    _isTauri = true;
  } catch {
    _isTauri = false;
  }
  return _isTauri;
}
