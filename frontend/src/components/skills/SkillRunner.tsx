import React, { useState, useCallback, useEffect } from 'react';
import { Play, Clock, CheckCircle, XCircle, Terminal } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Modal } from '@/components/ui/Modal';
import { skillService } from '@/services/skillService';
import type { Skill, SkillRunResult } from '@/types/domain';
import toast from 'react-hot-toast';

interface SkillRunnerProps {
  skill?: Skill;
  skillName?: string;
  isOpen: boolean;
  onClose: () => void;
  onRun?: (result: SkillRunResult) => void;
}

export const SkillRunner: React.FC<SkillRunnerProps> = ({
  skill,
  skillName,
  isOpen,
  onClose,
  onRun,
}) => {
  const [args, setArgs] = useState<Record<string, string>>({});
  const [jsonArgs, setJsonArgs] = useState('{}');
  const [useJsonMode, setUseJsonMode] = useState(false);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<SkillRunResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const targetSkillName = skill?.name || skillName || '';
  const targetSkillDesc = skill?.description || '';

  useEffect(() => {
    if (isOpen) {
      setArgs({});
      setJsonArgs('{}');
      setResult(null);
      setError(null);
    }
  }, [isOpen, targetSkillName]);

  const handleAddArg = useCallback(() => {
    const key = `param${Object.keys(args).length + 1}`;
    setArgs((prev) => ({ ...prev, [key]: '' }));
  }, [args]);

  const handleRemoveArg = useCallback((key: string) => {
    setArgs((prev) => {
      const newArgs = { ...prev };
      delete newArgs[key];
      return newArgs;
    });
  }, []);

  const handleArgChange = useCallback((key: string, value: string) => {
    setArgs((prev) => ({ ...prev, [key]: value }));
  }, []);

  const handleRun = useCallback(async () => {
    if (!targetSkillName) {
      toast.error('未指定技能');
      return;
    }

    setLoading(true);
    setResult(null);
    setError(null);

    try {
      let parsedArgs: Record<string, unknown> = {};

      if (useJsonMode) {
        try {
          parsedArgs = JSON.parse(jsonArgs);
        } catch {
          toast.error('JSON 格式错误');
          setLoading(false);
          return;
        }
      } else {
        // 转换字符串值为适当类型
        parsedArgs = Object.entries(args).reduce((acc, [key, value]) => {
          // 尝试解析数字
          if (/^-?\d+$/.test(value)) {
            acc[key] = parseInt(value, 10);
          } else if (/^-?\d+\.\d+$/.test(value)) {
            acc[key] = parseFloat(value);
          } else if (value.toLowerCase() === 'true') {
            acc[key] = true;
          } else if (value.toLowerCase() === 'false') {
            acc[key] = false;
          } else {
            acc[key] = value;
          }
          return acc;
        }, {} as Record<string, unknown>);
      }

      const res = await skillService.runSkill(targetSkillName, parsedArgs, 60);

      if (res.status === 'ok' && res.result) {
        setResult(res.result);
        onRun?.(res.result);
        toast.success(`技能执行成功 (${res.result.execution_time_ms}ms)`);
      } else {
        setError(res.error || '执行失败');
        toast.error(res.error || '执行失败');
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : '执行出错';
      setError(msg);
      toast.error(msg);
    } finally {
      setLoading(false);
    }
  }, [targetSkillName, args, jsonArgs, useJsonMode, onRun]);

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={`运行技能: ${targetSkillName}`}>
      <div className="space-y-4">
        {targetSkillDesc && (
          <div className="text-sm text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-800/50 p-3 rounded">
            {targetSkillDesc}
          </div>
        )}

        <div className="flex items-center gap-2 mb-2">
          <button
            onClick={() => setUseJsonMode(false)}
            className={`px-3 py-1 text-sm rounded ${
              !useJsonMode
                ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                : 'text-gray-600 dark:text-gray-400'
            }`}
          >
            表单模式
          </button>
          <button
            onClick={() => setUseJsonMode(true)}
            className={`px-3 py-1 text-sm rounded ${
              useJsonMode
                ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                : 'text-gray-600 dark:text-gray-400'
            }`}
          >
            JSON 模式
          </button>
        </div>

        {useJsonMode ? (
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              参数 (JSON)
            </label>
            <textarea
              value={jsonArgs}
              onChange={(e) => setJsonArgs(e.target.value)}
              className="w-full h-32 px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 font-mono text-sm"
              placeholder='{"key": "value"}'
            />
          </div>
        ) : (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                参数
              </label>
              <Button size="sm" variant="ghost" onClick={handleAddArg}>
                + 添加参数
              </Button>
            </div>

            {Object.keys(args).length === 0 && (
              <div className="text-sm text-gray-500 italic">点击"添加参数"按钮添加输入参数</div>
            )}

            {Object.entries(args).map(([key, value]) => (
              <div key={key} className="flex gap-2">
                <Input
                  value={key}
                  onChange={(e) => {
                    const newKey = e.target.value;
                    if (newKey !== key) {
                      setArgs((prev) => {
                        const newArgs = { ...prev };
                        delete newArgs[key];
                        newArgs[newKey] = value;
                        return newArgs;
                      });
                    }
                  }}
                  placeholder="参数名"
                  className="w-1/3"
                />
                <Input
                  value={value}
                  onChange={(e) => handleArgChange(key, e.target.value)}
                  placeholder="参数值"
                  className="flex-1"
                />
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => handleRemoveArg(key)}
                  className="text-red-500"
                >
                  删除
                </Button>
              </div>
            ))}
          </div>
        )}

        <Button
          onClick={handleRun}
          loading={loading}
          className="w-full flex items-center justify-center gap-2"
        >
          <Play className="w-4 h-4" />
          {loading ? '执行中...' : '运行技能'}
        </Button>

        {result && (
          <Card className={result.success ? 'border-green-200' : 'border-red-200'}>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm flex items-center gap-2">
                {result.success ? (
                  <CheckCircle className="w-4 h-4 text-green-500" />
                ) : (
                  <XCircle className="w-4 h-4 text-red-500" />
                )}
                执行结果
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex items-center gap-4 text-xs text-gray-500 mb-2">
                <span className="flex items-center gap-1">
                  <Clock className="w-3 h-3" />
                  {result.execution_time_ms}ms
                </span>
                <span>{new Date(result.timestamp).toLocaleString()}</span>
              </div>
              <div className="bg-gray-900 text-gray-100 p-3 rounded text-sm font-mono whitespace-pre-wrap max-h-64 overflow-y-auto">
                {result.output || '(无输出)'}
              </div>
            </CardContent>
          </Card>
        )}

        {error && (
          <Card className="border-red-200">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm flex items-center gap-2 text-red-600">
                <XCircle className="w-4 h-4" />
                执行错误
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="bg-red-50 dark:bg-red-900/20 text-red-800 dark:text-red-200 p-3 rounded text-sm">
                {error}
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </Modal>
  );
};

// 快速运行按钮组件
interface QuickRunButtonProps {
  skill: Skill;
  onRun?: (result: SkillRunResult) => void;
}

export const QuickRunButton: React.FC<QuickRunButtonProps> = ({ skill, onRun }) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <Button size="sm" variant="primary" onClick={() => setIsOpen(true)}>
        <Terminal className="w-4 h-4 mr-1" />
        运行
      </Button>
      <SkillRunner
        skill={skill}
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
        onRun={onRun}
      />
    </>
  );
};
