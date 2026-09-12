import { Globe } from 'lucide-react-native';
import { useState } from 'react';
import { Image, View } from 'react-native';
import { ThemedIcon } from '../components/ui/icon';

interface FaviconProps {
  url?: string;
  size?: number;
  className?: string;
}

function domainFromUrl(url: string): string | null {
  try {
    const u = new URL(url.startsWith('http') ? url : `https://${url}`);
    return u.hostname;
  } catch {
    return null;
  }
}

export function Favicon({ url, size = 16, className }: FaviconProps) {
  const [errored, setErrored] = useState(false);
  const domain = url ? domainFromUrl(url) : null;
  if (!domain || errored) {
    return (
      <View
        style={{ width: size, height: size }}
        className={`items-center justify-center ${className ?? ''}`}
      >
        <ThemedIcon icon={Globe} size={size} tone="muted" />
      </View>
    );
  }
  return (
    <Image
      source={{ uri: `https://www.google.com/s2/favicons?domain=${domain}&sz=${size * 2}` }}
      style={{ width: size, height: size, borderRadius: size * 0.125 }}
      className={className}
      onError={() => setErrored(true)}
      accessibilityLabel={`Favicon for ${domain}`}
    />
  );
}
