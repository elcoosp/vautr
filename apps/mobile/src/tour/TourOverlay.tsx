import { useEffect, useLayoutEffect, useState } from 'react';
import { Dimensions, Modal, Pressable, Text, View } from 'react-native';
import { Button, ButtonText } from '../../components/ui/button';
import { getTourAnchor } from './anchors';
import { TOUR_STEPS } from './steps';

export const TOUR_REPLAY_EVENT = 'vautr:replay-tour';

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

const vw = () => Dimensions.get('window').width;
const vh = () => Dimensions.get('window').height;

/**
 * Feature-tour overlay (VTR-078, mobile). Upgraded from the centered-card
 * sequence (VTR-077) to a true element-anchored spotlight: the active step's
 * target registers a ref via `registerTourAnchor`, and we `measure()` it to
 * draw a dim scrim + highlight ring + positioned card. Steps without a mounted
 * target (e.g. a surface not currently open) fall back to a centered card — the
 * same graceful behaviour as the web overlay when no `[data-tour]` element
 * exists.
 *
 * Rendered as a transparent Modal so the underlying screen stays mounted and
 * measurable.
 */
export function TourOverlay() {
  const [index, setIndex] = useState<number | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);

  useEffect(() => {
    const onReplay = () => setIndex(0);
    if (typeof window !== 'undefined' && window.addEventListener) {
      window.addEventListener(TOUR_REPLAY_EVENT, onReplay);
      return () => window.removeEventListener(TOUR_REPLAY_EVENT, onReplay);
    }
    return undefined;
  }, []);

  // Measure the active step's anchor whenever the step changes (or the layout
  // shifts). Mirrors the web `useLayoutEffect(measure, [index])` + resize/scroll
  // listeners.
  useLayoutEffect(() => {
    if (index === null) {
      setRect(null);
      return;
    }
    const anchorId = TOUR_STEPS[index]?.anchor;
    const ref = anchorId ? getTourAnchor(anchorId) : undefined;
    if (ref?.current) {
      ref.current.measure(
        (_x: number, _y: number, width: number, height: number, pageX: number, pageY: number) => {
          if (width > 0 && height > 0) {
            setRect({ x: pageX, y: pageY, width, height });
          } else {
            setRect(null);
          }
        },
      );
    } else {
      setRect(null);
    }
  }, [index]);

  if (index === null) return null;
  const step = TOUR_STEPS[index];
  if (!step) return null;
  const isLast = index + 1 >= TOUR_STEPS.length;
  const finish = () => setIndex(null);
  const next = () => (isLast ? finish() : setIndex(index + 1));
  const back = () => setIndex(Math.max(0, index - 1));

  // Position the card below the highlighted element, clamped to the viewport.
  const cardWidth = Math.min(320, (rect?.width ?? 0) + 40 || 320);
  const cardTop = rect ? Math.min(rect.y + rect.height + 12, vh() - 200) : vh() / 2 - 100;
  const cardLeft = rect
    ? Math.max(12, Math.min(rect.x, vw() - cardWidth - 12))
    : vw() / 2 - cardWidth / 2;
  const curVw = vw();
  const curVh = vh();

  return (
    <Modal transparent animationType="fade" visible onRequestClose={finish}>
      <Pressable className="flex-1 bg-black/60" onPress={back}>
        {/* Highlight ring around the anchored element. */}
        {rect && (
          <View
            pointerEvents="none"
            className="absolute rounded-xl"
            style={{
              top: rect.y - 6,
              left: rect.x - 6,
              width: rect.width + 12,
              height: rect.height + 12,
              borderWidth: 2,
              borderColor: '#42b59a',
            }}
          />
        )}
        {/* Tour card, positioned next to the anchor. */}
        <View
          className="absolute rounded-2xl border border-border bg-surface p-5"
          style={{
            top: cardTop,
            left: Math.max(12, Math.min(cardLeft, curVw - cardWidth - 12)),
            width: cardWidth,
          }}
        >
          <Text className="mb-1 text-xs text-text-muted">
            Step {index + 1} of {TOUR_STEPS.length}
          </Text>
          <Text className="mb-1 text-lg font-semibold text-foreground">{step.title}</Text>
          <Text className="mb-4 text-sm text-text-muted">{step.body}</Text>
          <View className="flex-row items-center justify-between">
            <Button variant="ghost" disabled={index === 0} onPress={back}>
              <ButtonText>Back</ButtonText>
            </Button>
            <View className="flex-row gap-2">
              <Button variant="ghost" onPress={finish}>
                <ButtonText>Skip</ButtonText>
              </Button>
              <Button onPress={next}>
                <ButtonText>{isLast ? 'Finish' : 'Next'}</ButtonText>
              </Button>
            </View>
          </View>
        </View>
        {/* Keep the whole overlay inside the viewport bounds. */}
        <View style={{ position: 'absolute', width: curVw, height: curVh }} pointerEvents="none" />
      </Pressable>
    </Modal>
  );
}

/** Ask the TourOverlay to (re)start the tour from Settings. */
export function triggerReplayTour() {
  if (typeof window !== 'undefined' && window.dispatchEvent) {
    window.dispatchEvent(new Event(TOUR_REPLAY_EVENT));
  }
}
