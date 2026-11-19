import { useEffect, useState } from "react";

/** Small, global "Saved" confirmation. Call `showToast()` from any screen. */
const TOAST_EVENT = "admin-toast";

export function showToast(message: string) {
  window.dispatchEvent(new CustomEvent<string>(TOAST_EVENT, { detail: message }));
}

export function ToastHost() {
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let timer: number | undefined;

    function onToast(event: Event) {
      const detail = (event as CustomEvent<string>).detail;
      setMessage(detail);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => setMessage(null), 3000);
    }

    window.addEventListener(TOAST_EVENT, onToast);
    return () => {
      window.removeEventListener(TOAST_EVENT, onToast);
      window.clearTimeout(timer);
    };
  }, []);

  if (!message) return null;

  return (
    <div className="admin-toast" role="status" aria-live="polite">
      <span className="admin-toast__check" aria-hidden="true">✓</span>
      {message}
    </div>
  );
}
