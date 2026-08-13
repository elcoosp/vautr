import type { MachineAccount, Project, Secret } from '@vautr/api-contract';
import type { DecryptedOverview } from '@vautr/client-sdk';
import { create } from 'zustand';

export type PopupStatus = 'locked' | 'unlocking' | 'unlocked' | 'busy';

interface PopupState {
  status: PopupStatus;
  username: string | null;
  activeTab: string;
  notice: string | null;
  error: string | null;

  items: DecryptedOverview[];
  projects: Project[];
  secrets: Secret[];
  machines: MachineAccount[];

  /** Plaintext passwords revealed this session (in-memory only) for reused detection. */
  revealedPasswords: string[];

  setStatus: (status: PopupStatus) => void;
  setActiveTab: (tab: string) => void;
  setNotice: (notice: string | null) => void;
  setError: (error: string | null) => void;
  setUsername: (username: string | null) => void;

  setItems: (items: DecryptedOverview[]) => void;
  upsertItem: (item: DecryptedOverview) => void;
  removeItem: (uuid: string) => void;

  setProjects: (projects: Project[]) => void;
  upsertProject: (project: Project) => void;
  removeProject: (uuid: string) => void;

  setSecrets: (secrets: Secret[]) => void;
  upsertSecret: (secret: Secret) => void;
  removeSecret: (uuid: string) => void;

  setMachines: (machines: MachineAccount[]) => void;

  addRevealedPassword: (password: string) => void;
  reset: () => void;
}

export const usePopupStore = create<PopupState>((set) => ({
  status: 'locked',
  username: null,
  activeTab: 'vault',
  notice: null,
  error: null,

  items: [],
  projects: [],
  secrets: [],
  machines: [],
  revealedPasswords: [],

  setStatus: (status) => set({ status }),
  setActiveTab: (activeTab) => set({ activeTab }),
  setNotice: (notice) => set({ notice, error: null }),
  setError: (error) => set({ error, notice: null }),
  setUsername: (username) => set({ username }),

  setItems: (items) => set({ items }),
  upsertItem: (item) =>
    set((s) => {
      const idx = s.items.findIndex((i) => i.uuid === item.uuid);
      if (idx >= 0) {
        const next = [...s.items];
        next[idx] = item;
        return { items: next };
      }
      return { items: [...s.items, item] };
    }),
  removeItem: (uuid) => set((s) => ({ items: s.items.filter((i) => i.uuid !== uuid) })),

  setProjects: (projects) => set({ projects }),
  upsertProject: (project) =>
    set((s) => {
      const idx = s.projects.findIndex((p) => p.uuid === project.uuid);
      if (idx >= 0) {
        const next = [...s.projects];
        next[idx] = project;
        return { projects: next };
      }
      return { projects: [...s.projects, project] };
    }),
  removeProject: (uuid) => set((s) => ({ projects: s.projects.filter((p) => p.uuid !== uuid) })),

  setSecrets: (secrets) => set({ secrets }),
  upsertSecret: (secret) =>
    set((s) => {
      const idx = s.secrets.findIndex((x) => x.uuid === secret.uuid);
      if (idx >= 0) {
        const next = [...s.secrets];
        next[idx] = secret;
        return { secrets: next };
      }
      return { secrets: [...s.secrets, secret] };
    }),
  removeSecret: (uuid) => set((s) => ({ secrets: s.secrets.filter((x) => x.uuid !== uuid) })),

  setMachines: (machines) => set({ machines }),

  addRevealedPassword: (password) =>
    set((s) => ({ revealedPasswords: [...s.revealedPasswords, password] })),

  reset: () =>
    set({
      status: 'locked',
      username: null,
      activeTab: 'vault',
      notice: null,
      error: null,
      items: [],
      projects: [],
      secrets: [],
      machines: [],
      revealedPasswords: [],
    }),
}));
