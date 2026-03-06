import React from 'react';
import { cn } from './Button';

interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  className?: string;
}

export const EmptyState: React.FC<EmptyStateProps> = ({ icon, title, description, className }) => {
  return (
    <div className={cn("flex flex-col items-center justify-center p-8 text-center text-gray-500", className)}>
      {icon && <div className="mb-4">{icon}</div>}
      <h3 className="mb-2 text-lg font-semibold text-gray-900 dark:text-gray-100">{title}</h3>
      {description && <p className="text-sm text-gray-500 dark:text-gray-400">{description}</p>}
    </div>
  );
};
