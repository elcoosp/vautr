export interface TourStep {
  id: string;
  anchor: string; // data-tour attribute value
  title: string;
  body: string;
  skippable: boolean;
}

export const TOUR_STEPS: TourStep[] = [
  {
    id: 'vault',
    anchor: 'vault',
    title: 'Your Vault',
    body: 'This is where your encrypted secrets live. Everything is decrypted only on your device.',
    skippable: true,
  },
  {
    id: 'add-secret',
    anchor: 'add-secret',
    title: 'Add a secret',
    body: 'Create a login, note, or card here. It is encrypted before it ever leaves your device.',
    skippable: true,
  },
  {
    id: 'emergency-kit',
    anchor: 'emergency-kit',
    title: 'Emergency Kit',
    body: 'Generate and store your Emergency Kit here — it is your only way to recover the account if you forget your master password.',
    skippable: true,
  },
  {
    id: 'audit',
    anchor: 'audit',
    title: 'Audit log',
    body: 'See who accessed what and when. The audit log is your security trail.',
    skippable: true,
  },
];
