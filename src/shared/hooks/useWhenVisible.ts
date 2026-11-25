import { useEffect, useRef } from "react";

/**
 * Calls `handler` when the user returns to the app: the window gets the focus
 * or the page becomes visible again. The latest `handler` is used.
 */
export function useWhenVisible(handler: () => void) {
  const latest = useRef(handler);
  latest.current = handler;

  useEffect(() => {
    const onVisible = () => {
      if (document.visibilityState === "visible") latest.current();
    };
    window.addEventListener("focus", onVisible);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("focus", onVisible);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);
}
