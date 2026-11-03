import { useState } from "react";

/**
 * Counts how often a dialog has opened. A dialog keys its form on it, so the form starts fresh
 * on each opening and nothing that changes while it is open (preferences saved elsewhere, the
 * login method, a remembered folder) resets what the user is typing or a task it follows.
 */
export function useOpenings(open: boolean): number {
  const [openings, setOpenings] = useState(0);
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) setOpenings(openings + 1);
  }
  return openings;
}
