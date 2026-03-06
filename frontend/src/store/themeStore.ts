import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface ThemeState {
  theme: 'light' | 'dark' | 'system';
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      theme: 'system',
      setTheme: (theme) => {
        set({ theme });
        if (theme === 'dark') {
          document.documentElement.classList.add('dark');
        } else if (theme === 'light') {
          document.documentElement.classList.remove('dark');
        } else {
          if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
            document.documentElement.classList.add('dark');
          } else {
            document.documentElement.classList.remove('dark');
          }
        }
      },
    }),
    {
      name: 'theme-storage',
      onRehydrateStorage: () => (state) => {
        if (state) {
           const theme = state.theme;
           if (theme === 'dark') {
             document.documentElement.classList.add('dark');
           } else if (theme === 'light') {
             document.documentElement.classList.remove('dark');
           } else {
             if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
               document.documentElement.classList.add('dark');
             } else {
               document.documentElement.classList.remove('dark');
             }
           }
        }
      }
    }
  )
);
