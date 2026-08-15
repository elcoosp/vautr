/**
 * Tour anchor registry (VTR-078, mobile). RN has no `document.querySelector`,
 * so anchor targets register their element `ref` here under a stable id
 * (`data-tour` equivalent). The TourOverlay reads the registered ref and calls
 * `ref.measure()` to get the target's on-screen rect — the RN analogue of the
 * web/extension `getBoundingClientRect()` spotlight.
 */
import { createElement, forwardRef, type RefObject, useImperativeHandle } from 'react';
import { View, type ViewProps } from 'react-native';

const REGISTRY = new Map<string, RefObject<View>>();

/** Register (or replace) the ref for a tour anchor id. */
export function registerTourAnchor(id: string, ref: RefObject<View>) {
  REGISTRY.set(id, ref);
}

/** Drop a previously-registered anchor (call on unmount). */
export function unregisterTourAnchor(id: string) {
  REGISTRY.delete(id);
}

/** Look up the ref for a tour anchor id (undefined if not mounted). */
export function getTourAnchor(id: string): RefObject<View> | undefined {
  return REGISTRY.get(id);
}

/**
 * Wraps any element so it becomes a tour anchor. The wrapper's native node is
 * registered under `id`; the TourOverlay measures it via `ref.measure()`.
 *
 *   <TourAnchor id="secrets-tab"><Button>...</Button></TourAnchor>
 */
export const TourAnchor = forwardRef<View, ViewProps & { id: string }>(function TourAnchor(
  { id, children, ...props },
  _ref,
) {
  const localRef = (ref: View | null) => {
    // Register the host instance so the overlay can measure it.
    registerTourAnchor(id, { current: ref } as RefObject<View>);
  };
  useImperativeHandle(_ref, () => ({}) as unknown as View);
  return createElement(View, { ref: localRef, ...props }, children);
});
