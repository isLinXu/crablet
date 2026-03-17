/**
 * ModelSelector Component
 * Unified model selection component that syncs with modelStore
 */

import React, { useMemo } from 'react';
import { useModelStore, type ModelProvider } from '@/store/modelStore';
import { Brain, Bot, Sparkles } from 'lucide-react';

interface ModelSelectorProps {
  value: string;
  onChange: (value: string, provider?: ModelProvider) => void;
  filter?: (provider: ModelProvider) => boolean;
  showProvider?: boolean;
  showIcon?: boolean;
  className?: string;
  disabled?: boolean;
  placeholder?: string;
}

// Vendor icons mapping
const vendorIcons: Record<string, React.ReactNode> = {
  OpenAI: <Sparkles className="w-4 h-4" />,
  Anthropic: <Brain className="w-4 h-4" />,
  Google: <Bot className="w-4 h-4" />,
};

// Get vendor color
const getVendorColor = (vendor: string): string => {
  const colors: Record<string, string> = {
    OpenAI: '#10a37f',
    Anthropic: '#d97757',
    Google: '#4285f4',
    阿里云: '#ff6a00',
    腾讯云: '#00a3ff',
    字节豆包: '#1a1a1a',
    Kimi: '#1a1a1a',
    智谱: '#1a1a1a',
    Ollama: '#ff6b35',
    Backend: '#6366f1',
  };
  return colors[vendor] || '#6366f1';
};

export const ModelSelector: React.FC<ModelSelectorProps> = ({
  value,
  onChange,
  filter,
  showProvider = true,
  showIcon = true,
  className = '',
  disabled = false,
  placeholder = 'Select a model...',
}) => {
  const { providers } = useModelStore();

  // Filter enabled providers
  const availableModels = useMemo(() => {
    const enabled = providers.filter((p) => p.enabled);
    if (filter) {
      return enabled.filter(filter);
    }
    // Default: only chat models for workflow nodes
    return enabled.filter((p) => !p.modelType || p.modelType === 'chat');
  }, [providers, filter]);

  // Group by vendor
  const groupedModels = useMemo(() => {
    const groups: Record<string, ModelProvider[]> = {};
    availableModels.forEach((provider) => {
      if (!groups[provider.vendor]) {
        groups[provider.vendor] = [];
      }
      groups[provider.vendor].push(provider);
    });
    return groups;
  }, [availableModels]);

  // Get current provider
  const currentProvider = useMemo(() => {
    return availableModels.find((p) => p.model === value || p.id === value);
  }, [availableModels, value]);

  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const selectedId = e.target.value;
    const provider = availableModels.find((p) => p.id === selectedId);
    if (provider) {
      onChange(provider.model, provider);
    }
  };

  // If no models available, show warning
  if (availableModels.length === 0) {
    return (
      <div className={`text-sm text-amber-600 bg-amber-50 px-3 py-2 rounded-lg ${className}`}>
        No models configured. Please add models in Settings.
      </div>
    );
  }

  return (
    <div className={`relative ${className}`}>
      <select
        value={currentProvider?.id || ''}
        onChange={handleChange}
        disabled={disabled}
        className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <option value="" disabled>
          {placeholder}
        </option>
        {Object.entries(groupedModels).map(([vendor, models]) => (
          <optgroup key={vendor} label={vendor}>
            {models.map((provider) => (
              <option key={provider.id} value={provider.id}>
                {showProvider ? `${provider.vendor} - ${provider.model}` : provider.model}
                {provider.version && ` (${provider.version})`}
              </option>
            ))}
          </optgroup>
        ))}
      </select>
      {showIcon && currentProvider && (
        <div
          className="absolute right-8 top-1/2 -translate-y-1/2 pointer-events-none"
          style={{ color: getVendorColor(currentProvider.vendor) }}
        >
          {vendorIcons[currentProvider.vendor] || <Brain className="w-4 h-4" />}
        </div>
      )}
    </div>
  );
};

// Compact version for inline use
export const ModelSelectorCompact: React.FC<Omit<ModelSelectorProps, 'showProvider' | 'showIcon'>> = ({
  value,
  onChange,
  filter,
  className = '',
  disabled = false,
  placeholder = 'Model...',
}) => {
  const { providers } = useModelStore();

  const availableModels = useMemo(() => {
    const enabled = providers.filter((p) => p.enabled);
    if (filter) {
      return enabled.filter(filter);
    }
    return enabled.filter((p) => !p.modelType || p.modelType === 'chat');
  }, [providers, filter]);

  const currentProvider = useMemo(() => {
    return availableModels.find((p) => p.model === value || p.id === value);
  }, [availableModels, value]);

  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const selectedId = e.target.value;
    const provider = availableModels.find((p) => p.id === selectedId);
    if (provider) {
      onChange(provider.model, provider);
    }
  };

  if (availableModels.length === 0) {
    return (
      <span className="text-xs text-amber-600">No models</span>
    );
  }

  return (
    <select
      value={currentProvider?.id || ''}
      onChange={handleChange}
      disabled={disabled}
      className={`text-xs px-2 py-1 border border-gray-200 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-blue-500 ${className}`}
    >
      {availableModels.map((provider) => (
        <option key={provider.id} value={provider.id}>
          {provider.model}
        </option>
      ))}
    </select>
  );
};

// Hook for getting default model
export const useDefaultModel = (): { model: string; provider?: ModelProvider } => {
  const { providers } = useModelStore();

  return useMemo(() => {
    const enabled = providers.filter((p) => p.enabled && (!p.modelType || p.modelType === 'chat'));
    const sorted = enabled.sort((a, b) => a.priority - b.priority);
    const defaultProvider = sorted[0];
    return {
      model: defaultProvider?.model || 'gpt-4',
      provider: defaultProvider,
    };
  }, [providers]);
};

// Hook for model validation
export const useModelValidation = (modelId: string): { isValid: boolean; provider?: ModelProvider } => {
  const { providers } = useModelStore();

  return useMemo(() => {
    const provider = providers.find(
      (p) => p.enabled && (p.id === modelId || p.model === modelId)
    );
    return {
      isValid: !!provider,
      provider,
    };
  }, [providers, modelId]);
};

export default ModelSelector;
