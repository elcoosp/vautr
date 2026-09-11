export interface PasswordStrength {
  score: number;
  label: string;
  color: string;
  suggestions: string[];
}

export function evaluatePasswordStrength(password: string): PasswordStrength {
  if (!password) {
    return { score: 0, label: 'Empty', color: 'text-text-muted', suggestions: [] };
  }
  let score = 0;
  const suggestions: string[] = [];
  if (password.length >= 8) score += 1;
  else suggestions.push('Use at least 8 characters');
  if (password.length >= 14) score += 1;
  if (/[a-z]/.test(password)) score += 1;
  else suggestions.push('Add lowercase letters');
  if (/[A-Z]/.test(password)) score += 1;
  else suggestions.push('Add uppercase letters');
  if (/[0-9]/.test(password)) score += 1;
  else suggestions.push('Add numbers');
  if (/[^a-zA-Z0-9]/.test(password)) score += 1;
  else suggestions.push('Add special characters');
  if (password.length >= 20) score += 1;
  if (/(.)\1{2,}/.test(password)) score -= 1;
  if (/(0123|1234|2345|3456|4567|5678|6789|abcd|bcde|cdef|qwerty|password|abc123|iloveyou)/i.test(password)) {
    score -= 2;
    suggestions.push('Avoid common patterns');
  }
  score = Math.max(0, Math.min(score, 7));
  let label: string;
  let color: string;
  if (score <= 1) { label = 'Very weak'; color = 'text-danger'; }
  else if (score <= 3) { label = 'Weak'; color = 'text-danger'; }
  else if (score <= 4) { label = 'Fair'; color = 'text-warn'; }
  else if (score <= 5) { label = 'Good'; color = 'text-accent'; }
  else { label = 'Strong'; color = 'text-accent'; }
  return { score, label, color, suggestions };
}
