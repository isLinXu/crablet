import React, { useState, useEffect, useCallback } from 'react';
import { Clock, CheckCircle, XCircle, Terminal, RefreshCw, Filter, Download } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Modal } from '@/components/ui/Modal';
import { skillService } from '@/services/skillService';
import type { Skill, SkillExecutionLog } from '@/types/domain';
import toast from 'react-hot-toast';

interface SkillLogsProps {
  skill?: Skill;
  skillName?: string;
  isOpen: boolean;
  onClose: () => void;
}

export const SkillLogs: React.FC<SkillLogsProps> = ({
  skill,
  skillName,
  isOpen,
  onClose,
}) => {
  const [logs, setLogs] = useState<SkillExecutionLog[]>([]);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<'all' | 'success' | 'error'>('all');

  const targetSkillName = skill?.name || skillName;

  const fetchLogs = useCallback(async () => {
    if (!targetSkillName) {
      // 获取所有日志
      setLoading(true);
      try {
        const res = await skillService.getAllLogs(100);
        setLogs(res.logs || []);
      } catch (error) {
        toast.error('获取日志失败');
      } finally {
        setLoading(false);
      }
      return;
    }

    setLoading(true);
    try {
      const res = await skillService.getSkillLogs(targetSkillName, 50);
      setLogs(res.logs || []);
    } catch (error) {
      toast.error('获取日志失败');
    } finally {
      setLoading(false);
    }
  }, [targetSkillName]);

  useEffect(() => {
    if (isOpen) {
      fetchLogs();
    }
  }, [isOpen, fetchLogs]);

  const filteredLogs = logs.filter((log) => {
    if (filter === 'success') return log.success;
    if (filter === 'error') return !log.success;
    return true;
  });

  const handleExport = useCallback(() => {
    const dataStr = JSON.stringify(filteredLogs, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `skill-logs-${targetSkillName || 'all'}-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
    toast.success('日志已导出');
  }, [filteredLogs, targetSkillName]);

  const formatDuration = (ms: number) => {
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={targetSkillName ? `执行日志: ${targetSkillName}` : '所有技能执行日志'}
      className="max-w-4xl"
    >
      <div className="space-y-4">
        <div className="flex items-center justify-between flex-wrap gap-2">
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant={filter === 'all' ? 'primary' : 'secondary'}
              onClick={() => setFilter('all')}
            >
              全部
            </Button>
            <Button
              size="sm"
              variant={filter === 'success' ? 'primary' : 'secondary'}
              onClick={() => setFilter('success')}
            >
              成功
            </Button>
            <Button
              size="sm"
              variant={filter === 'error' ? 'primary' : 'secondary'}
              onClick={() => setFilter('error')}
            >
              失败
            </Button>
          </div>

          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={fetchLogs}
              loading={loading}
            >
              <RefreshCw className="w-4 h-4 mr-1" />
              刷新
            </Button>
            <Button size="sm" variant="secondary" onClick={handleExport}>
              <Download className="w-4 h-4 mr-1" />
              导出
            </Button>
          </div>
        </div>

        <div className="text-sm text-gray-500">
          共 {filteredLogs.length} 条记录
          {filter !== 'all' && ` (已筛选: ${filter})`}
        </div>

        <div className="space-y-2 max-h-96 overflow-y-auto">
          {loading ? (
            <div className="flex justify-center py-8">
              <RefreshCw className="w-8 h-8 animate-spin text-blue-500" />
            </div>
          ) : filteredLogs.length === 0 ? (
            <div className="text-center py-8 text-gray-500">
              <Terminal className="w-12 h-12 mx-auto mb-3 text-gray-300" />
              <p>暂无执行日志</p>
            </div>
          ) : (
            filteredLogs.map((log, index) => (
              <Card
                key={index}
                className={log.success ? 'border-l-4 border-l-green-400' : 'border-l-4 border-l-red-400'}
              >
                <CardContent className="p-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        {log.success ? (
                          <CheckCircle className="w-4 h-4 text-green-500" />
                        ) : (
                          <XCircle className="w-4 h-4 text-red-500" />
                        )}
                        <span className="font-medium text-sm">{log.skill_name}</span>
                        <span className="text-xs text-gray-400">
                          {new Date(log.timestamp).toLocaleString()}
                        </span>
                      </div>

                      <div className="flex items-center gap-2 text-xs text-gray-500 mb-2">
                        <Clock className="w-3 h-3" />
                        <span>{formatDuration(log.execution_time_ms)}</span>
                      </div>

                      {log.output && (
                        <div className="bg-gray-900 text-gray-100 p-2 rounded text-xs font-mono whitespace-pre-wrap max-h-32 overflow-y-auto">
                          {log.output.length > 500
                            ? log.output.substring(0, 500) + '...'
                            : log.output}
                        </div>
                      )}

                      {log.error && (
                        <div className="bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 p-2 rounded text-xs mt-2">
                          {log.error}
                        </div>
                      )}
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))
          )}
        </div>
      </div>
    </Modal>
  );
};

// 日志查看按钮
interface ViewLogsButtonProps {
  skill?: Skill;
  skillName?: string;
  variant?: 'primary' | 'secondary' | 'ghost';
  size?: 'sm' | 'md';
}

export const ViewLogsButton: React.FC<ViewLogsButtonProps> = ({
  skill,
  skillName,
  variant = 'secondary',
  size = 'sm',
}) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <Button size={size} variant={variant} onClick={() => setIsOpen(true)}>
        <Clock className="w-4 h-4 mr-1" />
        日志
      </Button>
      <SkillLogs
        skill={skill}
        skillName={skillName}
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
      />
    </>
  );
};
