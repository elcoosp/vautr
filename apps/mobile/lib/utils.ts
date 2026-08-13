import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Merge tailwind classes (shadcn/ui convention). */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
