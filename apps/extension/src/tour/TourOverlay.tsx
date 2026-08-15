import { useCallback, useEffect, useLayoutEffect, useState } from 'react';
import { Button } from '../components/ui/button';
import { TOUR_STEPS } from './steps';

export const TOUR_REPLAY_EVENT = 'vautr:replay-tour';

interface Rect {
  top: number;
  left: number;
  width: number;
  height: number;
}

/**
 * Anchored feature-tour overlay (VTR-077, extension). Same approach as web: dim
 * scrim + highlight ring around the `[data-tour]` element + positioned card.
 */
export function TourOverlay() {
  const [index, setIndex] = useState<number | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);

  const measure = useCallback(() => {
    if (index === null) return;
    const anchor = TOUR_STEPS[index]?.anchor;
    const el = anchor ? document.querySelector<HTMLElement>(`[data-tour="${anchor}"]`) : null;
    if (el) {
      const r = el.getBoundingClientRect();
      setRect({ top: r.top, left: r.left, width: r.width, height: r.height });
    } else {
      setRect(null);
    }
  }, [index]);

  useLayoutEffect(measure, [measure]);

  useEffect(() => {
    const onReplay = () => setIndex(0);
    window.addEventListener(TOUR_REPLAY_EVENT, onReplay);
    window.addEventListener('resize', measure);
    window.addEventListener('scroll', measure, true);
    return () => {
      window.removeEventListener(TOUR_REPLAY_EVENT, onReplay);
      window.removeEventListener('resize', measure);
      window.removeEventListener('scroll', measure, true);
    };
  }, [measure]);

  if (index === null) return null;
  const step = TOUR_STEPS[index];
  if (!step) return null;
  const isLast = index + 1 >= TOUR_STEPS.length;

  const finish = () => setIndex(null);
  const next = () => (isLast ? finish() : setIndex(index + 1));
  const back = () => setIndex(Math.max(0, index - 1));

  const cardTop = rect
    ? Math.min(rect.top + rect.height + 12, window.innerHeight - 220)
    : window.innerHeight / 2 - 100;
  const cardLeft = rect
    ? Math.min(Math.max(rect.left, 16), window.innerWidth - 336)
    : window.innerWidth / 2 - 160;

  return (
    <div className="fixed inset-0 z-[100]" role="dialog" aria-label="Feature tour">
      <button
        type="button"
        className="absolute inset-0 bg-black/60"
        aria-label="Dismiss feature tour"
        onClick={back}
        onKeyDown={(e) => {
          if (e.key === 'Escape' || e.key === 'Enter') back();
        }}
      />
      {rect && (
        <div
          className="pointer-events-none absolute rounded-lg ring-2 ring-accent"
          style={{
            top: rect.top - 6,
            left: rect.left - 6,
            width: rect.width + 12,
            height: rect.height + 12,
          }}
        />
      )}
      <div
        className="absolute w-80 rounded-lg border border-border bg-surface p-4 shadow-lg"
        style={{ top: cardTop, left: cardLeft }}
      >
        <div className="mb-1 text-xs text-text-muted">
          Step {index + 1} of {TOUR_STEPS.length}
        </div>
        <h3 className="mb-1 text-base font-semibold text-text">{step.title}</h3>
        <p className="mb-3 text-sm text-text-muted">{step.body}</p>
        <div className="flex items-center justify-between">
          <Button variant="ghost" onClick={back} disabled={index === 0}>
            Back
          </Button>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={finish}>
              Skip
            </Button>
            <Button onClick={next}>{isLast ? 'Finish' : 'Next'}</Button>
          </div>
        </div>
      </div>
    </div>
  );
}

export function triggerReplayTour() {
  window.dispatchEvent(new Event(TOUR_REPLAY_EVENT));
}
