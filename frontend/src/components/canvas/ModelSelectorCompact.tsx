import React from 'react';

interface ModelSelectorCompactProps {
  value: string;
  onChange: (model: string, provider: { id: string; vendor: string } | null) => void;
  placeholder?: string;
}

export const ModelSelectorCompact: React.FC<ModelSelectorCompactProps> = ({
  value,
  onChange,
  placeholder,
}) => {
  const models = [
    { id: 'qwen-plus', name: 'Qwen Plus', vendor: 'aliyun' },
    { id: 'gpt-4', name: 'GPT-4', vendor: 'openai' },
    { id: 'kimi', name: 'Kimi', vendor: 'moonshot' },
  ];

  return (
    <select
      value={value}
      onChange={(e) => {
        const selected = models.find((m) => m.id === e.target.value);
        if (selected) {
          onChange(selected.id, { id: selected.id, vendor: selected.vendor });
        }
      }}
      className="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
    >
      <option value="">{placeholder || 'Select model...'}</option>
      {models.map((model) => (
        <option key={model.id} value={model.id}>
          {model.name}
        </option>
      ))}
    </select>
  );
};
