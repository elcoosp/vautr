export interface TourStep {
  id: string;
  title: string;
  body: string;
  skippable: boolean;
}

export const TOUR_STEPS: TourStep[] = [
  {
    id: 'vault',
    title: 'Your Vault',
    body: 'This is where your encrypted secrets live. Everything is decrypted only on your device.',
    skippable: true,
  },
  {
    id: 'add-secret',
    title: 'Add a secret',
    body: 'Tap the + on the Vault tab to create a login, note, or card. It is encrypted before it leaves your device.',
    skippable: true,
  },
  {
    id: 'emergency-kit',
    title: 'Emergency Kit',
    body: 'Generate and store your Emergency Kit from MFA & security — your only way to recover the account if you forget your master password.',
    skippable: true,
  },
  {
    id: 'audit',
    title: 'Audit log',
    body: 'See who accessed what and when. The audit log is your security trail.',
    skippable: true,
  },
];
