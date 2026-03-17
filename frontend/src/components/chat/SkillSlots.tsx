import React, { useState } from 'react';
import { Terminal, Code, FileText, Search, Wrench, Sparkles, X, GripVertical, Plus } from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export interface Skill {
  id: string;
  name: string;
  icon: 'terminal' | 'code' | 'file' | 'search' | 'tool' | 'sparkles';
  description: string;
  color: string;
}

interface SkillSlotsProps {
  skills: Skill[];
  onSkillRemove?: (id: string) => void;
  onSkillAdd?: () => void;
  maxSlots?: number;
  className?: string;
}

const skillIcons = {
  terminal: Terminal,
  code: Code,
  file: FileText,
  search: Search,
  tool: Wrench,
  sparkles: Sparkles,
};

const skillColors = {
  blue: 'bg-blue-500/10 text-blue-400 border-blue-500/20 hover:bg-blue-500/20',
  emerald: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20 hover:bg-emerald-500/20',
  amber: 'bg-amber-500/10 text-amber-400 border-amber-500/20 hover:bg-amber-500/20',
  purple: 'bg-purple-500/10 text-purple-400 border-purple-500/20 hover:bg-purple-500/20',
  rose: 'bg-rose-500/10 text-rose-400 border-rose-500/20 hover:bg-rose-500/20',
  cyan: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/20 hover:bg-cyan-500/20',
};

// 默认技能
const defaultSkills: Skill[] = [
  { id: '1', name: '代码审查', icon: 'code', description: '分析代码质量和潜在问题', color: 'blue' },
  { id: '2', name: '文档生成', icon: 'file', description: '自动生成代码文档', color: 'emerald' },
  { id: '3', name: '智能搜索', icon: 'search', description: '深度检索知识库', color: 'amber' },
];

export const SkillSlots: React.FC<SkillSlotsProps> = ({
  skills = defaultSkills,
  onSkillRemove,
  onSkillAdd,
  maxSlots = 4,
  className,
}) => {
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);
  // 使用 skills prop 作为数据源，不维护内部状态
  const slots = skills;

  const handleDragStart = (index: number) => {
    setDraggedIndex(index);
  };

  const handleDragOver = (e: React.DragEvent, index: number) => {
    e.preventDefault();
    // 拖拽排序功能暂时禁用，避免状态管理复杂性
    if (draggedIndex === null || draggedIndex === index) return;
  };

  const handleDragEnd = () => {
    setDraggedIndex(null);
  };

  const handleRemove = (id: string) => {
    onSkillRemove?.(id);
  };

  return (
    <div className={cn("flex items-center gap-2 flex-wrap", className)}>
      <span className="text-xs text-zinc-500 mr-1">技能:</span>
      
      {slots.map((skill, index) => {
        const Icon = skillIcons[skill.icon];
        return (
          <div
            key={skill.id}
            draggable
            onDragStart={() => handleDragStart(index)}
            onDragOver={(e) => handleDragOver(e, index)}
            onDragEnd={handleDragEnd}
            className={cn(
              "group flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border text-xs font-medium cursor-move transition-all duration-200",
              skillColors[skill.color as keyof typeof skillColors] || skillColors.blue,
              draggedIndex === index && "opacity-50 scale-105"
            )}
            title={skill.description}
          >
            <GripVertical className="w-3 h-3 opacity-0 group-hover:opacity-50 transition-opacity" />
            <Icon className="w-3.5 h-3.5" />
            <span>{skill.name}</span>
            <button
              onClick={() => handleRemove(skill.id)}
              className="ml-1 p-0.5 rounded hover:bg-black/10 dark:hover:bg-white/10 opacity-0 group-hover:opacity-100 transition-opacity"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        );
      })}

      {/* 添加技能按钮 */}
      {slots.length < maxSlots && (
        <button
          onClick={onSkillAdd}
          className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg border border-dashed border-zinc-300 dark:border-zinc-700 text-xs text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
        >
          <Plus className="w-3.5 h-3.5" />
          <span>添加</span>
        </button>
      )}
    </div>
  );
};

export default SkillSlots;
