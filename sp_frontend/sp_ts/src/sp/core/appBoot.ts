export function markApplicationBooted(): void {
  const ready = (window as any).__SP_BOOT_READY__;
  if (typeof ready === 'function') {
    ready();
  }
}
