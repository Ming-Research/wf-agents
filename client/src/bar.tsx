// The part of the bar a view fills with what it needs at hand, such as the flow version shown or the work's facts,
// so that the view's own space is left to its content.

import { createContext, type ReactNode, useContext } from 'react';
import { createPortal } from 'react-dom';

export const BarSlot = createContext<HTMLElement | null>(null);

/// Puts its children in the bar.
export function InBar({ children }: { children: ReactNode }) {
  const slot = useContext(BarSlot);
  return slot ? createPortal(children, slot) : null;
}
