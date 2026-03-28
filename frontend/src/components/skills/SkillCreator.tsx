import React, { useState, useCallback } from 'react';
import { Sparkles, Wand2, Code, FileText, CheckCircle, ArrowRight, ArrowLeft, Save } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Modal } from '@/components/ui/Modal';
import { skillService } from '@/services/skillService';
import toast from 'react-hot-toast';

interface SkillCreatorProps {
  isOpen: boolean;
  onClose: () => void;
  onCreated?: () => void;
}

type WizardStep = 'basic' | 'parameters' | 'code' | 'review';

export const SkillCreator: React.FC<SkillCreatorProps> = ({
  isOpen,
  onClose,
  onCreated,
}) => {
  const [step, setStep] = useState<WizardStep>('basic');
  const [loading, setLoading] = useState(false);

  // 基本信息
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [version, setVersion] = useState('1.0.0');
  const [author, setAuthor] = useState('');

  // 参数
  const [parameters, setParameters] = useState<Array<{ name: string; type: string; description: string; required: boolean }>>([]);

  // 代码
  const [runtime, setRuntime] = useState<'python' | 'node' | 'shell'>('python');
  const [code, setCode] = useState('');

  const resetForm = useCallback(() => {
    setStep('basic');
    setName('');
    setDescription('');
    setVersion('1.0.0');
    setAuthor('');
    setParameters([]);
    setRuntime('python');
    setCode('');
  }, []);

  const handleClose = useCallback(() => {
    resetForm();
    onClose();
  }, [resetForm, onClose]);

  const validateStep = useCallback(() => {
    switch (step) {
      case 'basic':
        if (!name.trim()) {
          toast.error('请输入技能名称');
          return false;
        }
        if (!description.trim()) {
          toast.error('请输入技能描述');
          return false;
        }
        if (!/^[a-z0-9_-]+$/.test(name)) {
          toast.error('技能名称只能包含小写字母、数字、下划线和连字符');
          return false;
        }
        return true;
      case 'parameters':
        return true;
      case 'code':
        return true;
      default:
        return true;
    }
  }, [step, name, description]);

  const handleNext = useCallback(() => {
    if (!validateStep()) return;

    switch (step) {
      case 'basic':
        setStep('parameters');
        break;
      case 'parameters':
        setStep('code');
        // 生成默认代码模板
        if (!code) {
          setCode(generateCodeTemplate(runtime, name, parameters));
        }
        break;
      case 'code':
        setStep('review');
        break;
    }
  }, [step, validateStep, runtime, name, parameters, code]);

  const handleBack = useCallback(() => {
    switch (step) {
      case 'parameters':
        setStep('basic');
        break;
      case 'code':
        setStep('parameters');
        break;
      case 'review':
        setStep('code');
        break;
    }
  }, [step]);

  const handleAddParameter = useCallback(() => {
    setParameters((prev) => [
      ...prev,
      { name: '', type: 'string', description: '', required: true },
    ]);
  }, []);

  const handleRemoveParameter = useCallback((index: number) => {
    setParameters((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const handleParameterChange = useCallback((index: number, field: string, value: unknown) => {
    setParameters((prev) =>
      prev.map((param, i) =>
        i === index ? { ...param, [field]: value } : param
      )
    );
  }, []);

  const handleCreate = useCallback(async () => {
    setLoading(true);
    try {
      // 这里可以调用创建技能的 API
      // 目前使用 create-skills 技能来创建
      const result = await skillService.runSkill('create-skills', {
        name,
        description,
        version,
        author,
        parameters,
        runtime,
        code,
      });

      if (result.status === 'ok') {
        toast.success('技能创建成功！');
        onCreated?.();
        handleClose();
      } else {
        toast.error(result.error || '创建失败');
      }
    } catch {
      toast.error('创建技能时出错');
    } finally {
      setLoading(false);
    }
  }, [name, description, version, author, parameters, runtime, code, onCreated, handleClose]);

  const generateSkillYaml = useCallback(() => {
    const params: Record<string, unknown> = {};
    parameters.forEach((p) => {
      params[p.name] = {
        type: p.type,
        description: p.description,
      };
    });

    return `name: ${name}
description: ${description}
version: ${version}
author: ${author || 'Unknown'}
parameters:
  type: object
  properties:
${parameters.map((p) => `    ${p.name}:
      type: ${p.type}
      description: ${p.description}`).join('\n')}
entrypoint: ${runtime === 'python' ? 'python main.py' : runtime === 'node' ? 'node main.js' : 'bash main.sh'}
runtime: ${runtime === 'python' ? 'python3' : runtime === 'node' ? 'node' : 'bash'}
`;
  }, [name, description, version, author, parameters, runtime]);

  const renderStep = () => {
    switch (step) {
      case 'basic':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                技能名称 <span className="text-red-500">*</span>
              </label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="my-awesome-skill"
              />
              <p className="text-xs text-gray-500 mt-1">
                只能包含小写字母、数字、下划线和连字符
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                描述 <span className="text-red-500">*</span>
              </label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100"
                rows={3}
                placeholder="描述这个技能的功能..."
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  版本
                </label>
                <Input
                  value={version}
                  onChange={(e) => setVersion(e.target.value)}
                  placeholder="1.0.0"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  作者
                </label>
                <Input
                  value={author}
                  onChange={(e) => setAuthor(e.target.value)}
                  placeholder="Your Name"
                />
              </div>
            </div>
          </div>
        );

      case 'parameters':
        return (
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="font-medium">参数定义</h3>
              <Button size="sm" variant="secondary" onClick={handleAddParameter}>
                + 添加参数
              </Button>
            </div>

            {parameters.length === 0 && (
              <div className="text-center py-8 text-gray-500">
                <p>暂无参数</p>
                <p className="text-sm">点击"添加参数"按钮添加输入参数</p>
              </div>
            )}

            {parameters.map((param, index) => (
              <Card key={index}>
                <CardContent className="p-3 space-y-3">
                  <div className="grid grid-cols-2 gap-3">
                    <Input
                      value={param.name}
                      onChange={(e) => handleParameterChange(index, 'name', e.target.value)}
                      placeholder="参数名"
                    />
                    <select
                      value={param.type}
                      onChange={(e) => handleParameterChange(index, 'type', e.target.value)}
                      className="px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-md bg-white dark:bg-gray-800"
                    >
                      <option value="string">字符串 (string)</option>
                      <option value="number">数字 (number)</option>
                      <option value="boolean">布尔值 (boolean)</option>
                      <option value="array">数组 (array)</option>
                      <option value="object">对象 (object)</option>
                    </select>
                  </div>
                  <Input
                    value={param.description}
                    onChange={(e) => handleParameterChange(index, 'description', e.target.value)}
                    placeholder="参数描述"
                  />
                  <div className="flex items-center justify-between">
                    <label className="flex items-center gap-2 text-sm">
                      <input
                        type="checkbox"
                        checked={param.required}
                        onChange={(e) => handleParameterChange(index, 'required', e.target.checked)}
                      />
                      必填
                    </label>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => handleRemoveParameter(index)}
                      className="text-red-500"
                    >
                      删除
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        );

      case 'code':
        return (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                运行时环境
              </label>
              <div className="flex gap-2">
                {(['python', 'node', 'shell'] as const).map((r) => (
                  <button
                    key={r}
                    onClick={() => {
                      setRuntime(r);
                      setCode(generateCodeTemplate(r, name, parameters));
                    }}
                    className={`px-4 py-2 rounded-md text-sm ${
                      runtime === r
                        ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                        : 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300'
                    }`}
                  >
                    {r === 'python' ? 'Python' : r === 'node' ? 'Node.js' : 'Shell'}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                代码
              </label>
              <textarea
                value={code}
                onChange={(e) => setCode(e.target.value)}
                className="w-full h-64 px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-md bg-gray-900 text-gray-100 font-mono text-sm"
                spellCheck={false}
              />
            </div>
          </div>
        );

      case 'review':
        return (
          <div className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">skill.yaml</CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="bg-gray-900 text-gray-100 p-3 rounded text-xs overflow-x-auto">
                  {generateSkillYaml()}
                </pre>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">
                  main.{runtime === 'python' ? 'py' : runtime === 'node' ? 'js' : 'sh'}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="bg-gray-900 text-gray-100 p-3 rounded text-xs overflow-x-auto max-h-48">
                  {code}
                </pre>
              </CardContent>
            </Card>

            <div className="flex items-center gap-2 text-sm text-amber-600 bg-amber-50 dark:bg-amber-900/20 p-3 rounded">
              <CheckCircle className="w-4 h-4" />
              确认创建后，技能将被保存到 ./skills/{name}/ 目录
            </div>
          </div>
        );
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title="创建新技能" className="max-w-2xl">
      <div className="space-y-4">
        {/* 步骤指示器 */}
        <div className="flex items-center justify-center gap-2 mb-6">
          {[
            { key: 'basic', label: '基本信息', icon: FileText },
            { key: 'parameters', label: '参数', icon: Wand2 },
            { key: 'code', label: '代码', icon: Code },
            { key: 'review', label: '确认', icon: CheckCircle },
          ].map((s, index) => (
            <React.Fragment key={s.key}>
              <div
                className={`flex items-center gap-1 px-3 py-1 rounded-full text-sm ${
                  step === s.key
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                    : 'text-gray-500'
                }`}
              >
                <s.icon className="w-4 h-4" />
                <span>{s.label}</span>
              </div>
              {index < 3 && <ArrowRight className="w-4 h-4 text-gray-300" />}
            </React.Fragment>
          ))}
        </div>

        {renderStep()}

        <div className="flex justify-between pt-4 border-t">
          <Button
            variant="secondary"
            onClick={step === 'basic' ? handleClose : handleBack}
          >
            {step === 'basic' ? '取消' : <><ArrowLeft className="w-4 h-4 mr-1" /> 上一步</>}
          </Button>

          {step === 'review' ? (
            <Button
              onClick={handleCreate}
              loading={loading}
              className="flex items-center gap-2"
            >
              <Save className="w-4 h-4" />
              创建技能
            </Button>
          ) : (
            <Button onClick={handleNext} className="flex items-center gap-2">
              下一步 <ArrowRight className="w-4 h-4" />
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
};

// 生成代码模板
function generateCodeTemplate(
  runtime: 'python' | 'node' | 'shell',
  skillName: string,
  parameters: Array<{ name: string; type: string }>
): string {
  const paramNames = parameters.map((p) => p.name).join(', ');

  switch (runtime) {
    case 'python':
      return `#!/usr/bin/env python3
"""
${skillName} skill
"""
import json
import sys

def main(${paramNames}):
    """
    Main function for ${skillName}
    """
    # TODO: Implement your skill logic here
    result = {
        "status": "success",
        "message": "Hello from ${skillName}!"
    }
    return result

if __name__ == "__main__":
    # Parse arguments from JSON
    if len(sys.argv) > 1:
        args = json.loads(sys.argv[1])
    else:
        args = {}
    
    result = main(**args)
    print(json.dumps(result))
`;

    case 'node':
      return `#!/usr/bin/env node
/**
 * ${skillName} skill
 */

function main(args) {
    // TODO: Implement your skill logic here
    const result = {
        status: 'success',
        message: 'Hello from ${skillName}!'
    };
    return result;
}

// Parse arguments from command line
const args = process.argv[2] ? JSON.parse(process.argv[2]) : {};
const result = main(args);
console.log(JSON.stringify(result));
`;

    case 'shell':
      return `#!/bin/bash
# ${skillName} skill

# Parse arguments (passed as JSON string in $1)
ARGS="$1"

# TODO: Implement your skill logic here
echo '{
    "status": "success",
    "message": "Hello from ${skillName}!"
}'
`;

    default:
      return '';
  }
}

// 创建技能按钮
export const CreateSkillButton: React.FC<{ onCreated?: () => void }> = ({ onCreated }) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <Button onClick={() => setIsOpen(true)} className="flex items-center gap-2">
        <Sparkles className="w-4 h-4" />
        创建技能
      </Button>
      <SkillCreator isOpen={isOpen} onClose={() => setIsOpen(false)} onCreated={onCreated} />
    </>
  );
};
