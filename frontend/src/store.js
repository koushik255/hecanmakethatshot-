import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export const useReaderStore = create(
  persist(
    (set) => ({
      manga: null,
      volume: null,
      volumes: [],
      manifest: null,
      step: 0,
      setSelection: ({ manga, volume, volumes = [] }) =>
        set({ manga, volume, volumes, step: 0, manifest: null }),
      setManifest: (manifest) => set({ manifest }),
      setStep: (step) => set({ step }),
      nextStep: () =>
        set((state) => {
          const max = Math.max((state.manifest?.steps?.length ?? 1) - 1, 0);
          return { step: Math.min(state.step + 1, max) };
        }),
      prevStep: () =>
        set((state) => ({ step: Math.max(state.step - 1, 0) })),
      nextVolume: () =>
        set((state) => {
          if (!state.volume || state.volumes.length === 0) return state;
          const currentIndex = state.volumes.findIndex((item) => item.number === state.volume);
          if (currentIndex === -1 || currentIndex + 1 >= state.volumes.length) return state;
          return {
            volume: state.volumes[currentIndex + 1].number,
            manifest: null,
            step: 0,
          };
        }),
      reset: () => set({ manga: null, volume: null, volumes: [], manifest: null, step: 0 }),
    }),
    {
      name: 'reader-state',
    },
  ),
);
