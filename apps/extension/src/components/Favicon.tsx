import { Globe } from 'lucide-react';
import { useState } from 'react';

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

export function Favicon({ url, size = 16, className = '' }: FaviconProps) {
  const [errored, setErrored] = useState(false);
  const domain = url ? domainFromUrl(url) : null;
  if (!domain || errored) {
    return (
      <Globe
        className={`text-muted-foreground ${className}`}
        style={{ width: size, height: size }}
        aria-hidden="true"
      />
    );
  }
  return (
    <img
      src={
        // VTRFIX-SEC-M31: default to a local letter avatar. The Google S2
        // fetch leaks every vault domain to Google and is opt-in only.
        (import.meta as unknown as { env?: Record<string, string> }).env?.VITE_ALLOW_FAVICON_LOOKUP === '1'
          ? `https://www.google.com/s2/favicons?domain=${domain}&sz=${size * 2}`
          : letterAvatarDataUrl(domain, size)
      }
      alt=""
      width={size}
      height={size}
      className={`rounded-sm ${className}`}
      onError={() => setErrored(true)}
    />
  );
}

/** VTRFIX-SEC-M31: a deterministic local letter avatar (no network call). */
function letterAvatarDataUrl(domain: string, size: number): string {
  const letter = (domain.replace(/^www\./, '').charAt(0) || '?').toUpperCase();
  let hue = 0;
  for (const c of domain) hue = (hue * 31 + c.charCodeAt(0)) >>> 0;
  hue %= 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}"><rect width="100%" height="100%" rx="4" fill="hsl(${hue},60%,45%)"/><text x="50%" y="55%" font-family="system-ui" font-size="${Math.floor(size * 0.6)}" font-weight="600" fill="#fff" text-anchor="middle" dominant-baseline="central">${letter}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}
