import { useEffect } from 'react';

type KeyCombo = string; // e.g. 'Cmd+K', 'Ctrl+Enter'

export function useKeyboard(keyMap: Record<KeyCombo, (e: KeyboardEvent) => void>) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const parts = [];
      if (e.metaKey || e.ctrlKey) parts.push(e.metaKey ? 'Cmd' : 'Ctrl'); // Normalize Mac Cmd to Cmd
      if (e.shiftKey) parts.push('Shift');
      if (e.altKey) parts.push('Alt');
      
      // Handle special keys
      if (e.key === 'Enter') parts.push('Enter');
      else if (e.key === 'Escape') parts.push('Esc');
      else if (e.key.length === 1) parts.push(e.key.toUpperCase());
      else return; // Ignore other special keys for now or add them as needed

      const combo = parts.join('+');
      
      // Try exact match
      if (keyMap[combo]) {
        e.preventDefault();
        keyMap[combo](e);
        return;
      }
      
      // Also try normalize 'Ctrl' to 'Cmd' for Windows users if keyMap uses 'Cmd'
      if (e.ctrlKey && !e.metaKey) {
          const macCombo = combo.replace('Ctrl', 'Cmd');
          if (keyMap[macCombo]) {
              e.preventDefault();
              keyMap[macCombo](e);
          }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [keyMap]);
}
