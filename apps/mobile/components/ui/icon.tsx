import type { LucideProps } from 'lucide-react-native';
import type { ComponentType } from 'react';

/**
 * lucide-react-native renders its internal <Path> using its own `color` prop,
 * which defaults to black and IGNORES any `text-foreground` className. So icons
 * styled only via a tailwind text color render black-on-dark (invisible). This
 * wrapper passes an explicit `color` to the underlying icon.
 *
 * Colors mirror the dark theme tokens in global.css (the app is dark by
 * default; `:root` holds the dark values). Keep these in sync with the tokens
 * if the palette changes. `tone` picks which token to use.
 */
const COLORS = {
  foreground: '#e8eaed', // --foreground: 220 14% 92%
  muted: '#949aa8', // --muted-foreground: 224 10% 62%
  primary: '#41b499', // --primary: 166 47% 48%
  primaryForeground: '#0c1713', // --primary-foreground: 226 18% 11%
} as const;

type Tone = keyof typeof COLORS;

export type ThemedIconProps = Omit<LucideProps, 'color'> & {
  icon: ComponentType<LucideProps>;
  tone?: Tone;
  color?: string;
};

export function ThemedIcon({ icon: Icon, tone = 'foreground', color, ...props }: ThemedIconProps) {
  return <Icon {...props} color={color ?? COLORS[tone]} />;
}
