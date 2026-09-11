import { useState } from 'react';
import { Globe } from 'lucide-react';

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
      <Globe className={`size-${size} text-text-muted ${className}`} aria-hidden="true" />
    );
  }
  return (
    <img
      src={`https://www.google.com/s2/favicons?domain=${domain}&sz=${size * 2}`}
      alt=""
      width={size}
      height={size}
      className={`rounded-sm ${className}`}
      onError={() => setErrored(true)}
    />
  );
}
