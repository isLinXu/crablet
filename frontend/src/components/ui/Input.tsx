import React from 'react';
import { cn } from './cn';

type InputProps = React.InputHTMLAttributes<HTMLInputElement>;

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, ...props }, ref) => {
    return (
      <input
        ref={ref}
        className={cn(
          'block h-10 w-full appearance-none rounded-md border border-gray-300 bg-white px-3 py-2 text-left text-sm leading-5 [text-indent:0] text-gray-900 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-gray-800 dark:border-gray-700 dark:text-gray-100 dark:placeholder:text-gray-400',
          className
        )}
        {...props}
      />
    );
  }
);

Input.displayName = 'Input';
