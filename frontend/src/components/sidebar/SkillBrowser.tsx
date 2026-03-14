import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useApi } from '../../hooks/useApi';
import type { BatchTestResult, RegistrySkillItem, SkillsShTopItem, Skill, SkillRunResult } from '@/types/domain';
import { skillService } from '@/services/skillService';
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card';
import { Button } from '../ui/Button';
import { Loader2, Terminal, CheckCircle, XCircle, Search, Download, PlayCircle, Wrench, Sparkles, Brain, Clock, FileText } from 'lucide-react';
import { EmptyState } from '../ui/EmptyState';
import toast from 'react-hot-toast';
import { Input } from '../ui/Input';
import { SemanticSearch } from '../skills/SemanticSearch';
import { SkillRunner, QuickRunButton } from '../skills/SkillRunner';
import { SkillLogs, ViewLogsButton } from '../skills/SkillLogs';
import { CreateSkillButton } from '../skills/SkillCreator';

export const SkillBrowser: React.FC = () => {
  const fetchSkillsApi = useCallback(() => skillService.listSkills().then((data) => ({ data })), []);
  const searchRegistryApi = useCallback((q: string) => skillService.searchRegistry(q).then((data) => ({ data })), []);
  const { data: skills, loading, execute: fetchSkills } = useApi<Skill[]>(fetchSkillsApi);
  const { loading: searching, execute: searchRegistry } = useApi<{ status: string; source?: string; items: RegistrySkillItem[] }, [string]>(
    searchRegistryApi
  );
  const [searchText, setSearchText] = useState('');
  const [registryQuery, setRegistryQuery] = useState('');
  const [registryResults, setRegistryResults] = useState<RegistrySkillItem[]>([]);
  const [installUrl, setInstallUrl] = useState('');
  const [selectedSkills, setSelectedSkills] = useState<string[]>([]);
  const [batchResults, setBatchResults] = useState<BatchTestResult[]>([]);
  const [batchTestExecuted, setBatchTestExecuted] = useState(false);
  const [searchSource, setSearchSource] = useState<string>('');
  const [topSkills, setTopSkills] = useState<SkillsShTopItem[]>([]);
  const [topSort, setTopSort] = useState<'installs' | 'name'>('installs');
  const [topPage, setTopPage] = useState(1);
  const [bulkCount, setBulkCount] = useState(10);
  const [bulkImporting, setBulkImporting] = useState(false);
  const [bulkProgress, setBulkProgress] = useState({ done: 0, total: 0, success: 0, failed: 0 });
  const [bulkFailedItems, setBulkFailedItems] = useState<SkillsShTopItem[]>([]);
  const topPageSize = 20;

  // 新增状态
  const [activeTab, setActiveTab] = useState<'installed' | 'search' | 'semantic' | 'top'>('installed');
  const [runningSkill, setRunningSkill] = useState<Skill | null>(null);
  const [viewingLogsSkill, setViewingLogsSkill] = useState<Skill | null>(null);
  const [viewingAllLogs, setViewingAllLogs] = useState(false);
  const [runResults, setRunResults] = useState<Record<string, SkillRunResult>>({});

  useEffect(() => {
    fetchSkills().catch(() => {});
    skillService.getTopSkills(100)
      .then((res: any) => setTopSkills(res?.items || []))
      .catch(() => setTopSkills([]));
  }, [fetchSkills]);

  const filteredSkills = useMemo(() => {
    const list = skills || [];
    const q = searchText.trim().toLowerCase();
    if (!q) return list;
    return list.filter((s) =>
      s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)
    );
  }, [skills, searchText]);

  const sortedTopSkills = useMemo(() => {
    const list = [...topSkills];
    if (topSort === 'name') {
      list.sort((a, b) => a.name.localeCompare(b.name));
      return list;
    }
    list.sort((a, b) => b.installs - a.installs);
    return list;
  }, [topSkills, topSort]);

  const topTotalPages = Math.max(1, Math.ceil(sortedTopSkills.length / topPageSize));
  const topPageItems = sortedTopSkills.slice((topPage - 1) * topPageSize, topPage * topPageSize);

  const runBatchTest = async (skillNames: string[]) => {
    const res: any = await skillService.batchTest(skillNames);
    const results = Array.isArray(res?.results) ? res.results : [];
    setBatchResults(results);
    setBatchTestExecuted(true);
    return results;
  };

  const handleToggle = async (skill: Skill) => {
    const nextEnabled = !skill.enabled;
    try {
      await skillService.toggleSkill(skill.name, nextEnabled);
      await fetchSkills();
      toast.success(`${skill.name} 已${nextEnabled ? '启用' : '禁用'}`);
    } catch {
      toast.error('更新技能状态失败');
    }
  };

  const handleRegistrySearch = async () => {
    try {
      const res = await searchRegistry(registryQuery);
      setSearchSource(res.source || '');
      setRegistryResults(res.items || []);
      if ((res.items || []).length === 0) {
        toast('未检索到技能，可尝试更换关键词');
      }
    } catch {
      toast.error('搜索技能市场失败');
    }
  };

  const handleInstall = async (item: RegistrySkillItem) => {
    try {
      await skillService.install({ name: item.name });
      toast.success(`已安装 ${item.name}`);
      await fetchSkills();
    } catch {
      toast.error(`安装 ${item.name} 失败`);
    }
  };

  const handleInstallByUrl = async () => {
    const url = installUrl.trim();
    if (!url) return;
    try {
      await skillService.install({ url });
      toast.success('通过Git地址安装成功');
      setInstallUrl('');
      await fetchSkills();
    } catch {
      toast.error('通过Git地址安装失败');
    }
  };

  const toggleSelect = (name: string) => {
    setSelectedSkills((prev) => (prev.includes(name) ? prev.filter((s) => s !== name) : [...prev, name]));
  };

  const handleBatchTest = async () => {
    if (selectedSkills.length === 0) return;
    try {
      const results = await runBatchTest(selectedSkills);
      if (results.length === 0) {
        toast.error('批量测试已执行，但未返回结果，请检查后端日志');
      } else {
        toast.success(`批量测试完成：共 ${results.length} 项`);
      }
    } catch {
      toast.error('批量测试失败');
    }
  };

  const handleQuickAction = async (skillName: string, keyword: string) => {
    try {
      const exists = (skills || []).some((s) => s.name === skillName);
      if (!exists) {
        await skillService.install({ name: skillName });
        await fetchSkills();
      }
      setSearchText(skillName);
      setRegistryQuery(keyword);
      setSelectedSkills([skillName]);
      const results = await runBatchTest([skillName]);
      if (results.length > 0) {
        toast.success(`${skillName} 快捷操作已就绪`);
      } else {
        toast.error(`${skillName} 测试无返回结果`);
      }
    } catch {
      toast.error(`${skillName} 快捷操作失败`);
    }
  };

  const fillInstallUrlFromTop = (item: SkillsShTopItem) => {
    setInstallUrl(`https://github.com/${item.source}.git`);
  };

  const handleInstallTopSkill = async (item: SkillsShTopItem) => {
    try {
      await skillService.install({ source: item.source, skill_id: item.skill_id });
      await fetchSkills();
      setSearchText(item.skill_id);
      toast.success(`已安装 Top Skill: ${item.name}`);
    } catch {
      toast.error(`Top Skill 安装失败: ${item.name}`);
    }
  };

  const installTopBatch = async (items: SkillsShTopItem[]) => {
    if (items.length === 0) return;
    setBulkImporting(true);
    setBulkProgress({ done: 0, total: items.length, success: 0, failed: 0 });
    const failed: SkillsShTopItem[] = [];
    let success = 0;
    let failedCount = 0;
    for (let i = 0; i < items.length; i += 1) {
      const item = items[i];
      try {
        const res: any = await skillService.install({ source: item.source, skill_id: item.skill_id });
        const status = res?.status;
        if (status === 'installed' || status === 'already_installed') {
          success += 1;
        } else {
          failed.push(item);
          failedCount += 1;
        }
      } catch {
        failed.push(item);
        failedCount += 1;
      }
      setBulkProgress({ done: i + 1, total: items.length, success, failed: failedCount });
    }
    setBulkFailedItems(failed);
    setBulkImporting(false);
    await fetchSkills();
    if (failed.length === 0) {
      toast.success(`批量导入完成：${success}/${items.length}`);
    } else {
      toast.error(`批量导入完成：成功${success}，失败${failed.length}`);
    }
  };

  const handleBulkImportTop = async () => {
    const count = Math.max(1, Math.min(100, bulkCount));
    const items = sortedTopSkills.slice(0, count);
    await installTopBatch(items);
  };

  const handleRetryFailed = async () => {
    await installTopBatch(bulkFailedItems);
  };

  const handleRunComplete = useCallback((result: SkillRunResult) => {
    setRunResults((prev) => ({ ...prev, [result.skill_name]: result }));
  }, []);

  return (
    <div className="h-full p-6 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <div className="flex justify-between items-center mb-4">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <Terminal className="w-6 h-6" />
            Skill Browser
        </h1>
        <div className="flex items-center gap-2">
          <CreateSkillButton onCreated={fetchSkills} />
          <Button onClick={() => setViewingAllLogs(true)} variant="secondary" size="sm">
            <Clock className="w-4 h-4 mr-1" />
            日志
          </Button>
          <Button onClick={() => fetchSkills()} variant="secondary" size="sm">
            Refresh
          </Button>
        </div>
      </div>

      {/* Tab 导航 */}
      <div className="flex items-center gap-2 mb-6 border-b dark:border-gray-700">
        {[
          { key: 'installed', label: '已安装', icon: Terminal },
          { key: 'search', label: '市场搜索', icon: Search },
          { key: 'semantic', label: '语义搜索', icon: Brain },
          { key: 'top', label: 'Top 100', icon: Sparkles },
        ].map((tab) => (
          <button
            key={tab.key}
            onClick={() => setActiveTab(tab.key as typeof activeTab)}
            className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
              activeTab === tab.key
                ? 'border-blue-500 text-blue-600 dark:text-blue-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400'
            }`}
          >
            <tab.icon className="w-4 h-4" />
            {tab.label}
          </button>
        ))}
      </div>

      {/* 已安装技能 Tab */}
      {activeTab === 'installed' && (
        <>
          <div className="mb-6 grid grid-cols-1 lg:grid-cols-3 gap-3">
            <Input
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
              placeholder="搜索已安装技能..."
              className="lg:col-span-2"
            />
            <Button onClick={handleBatchTest} disabled={selectedSkills.length === 0} variant="primary">
              <PlayCircle className="w-4 h-4 mr-2" />
              批量测试({selectedSkills.length})
            </Button>
          </div>
          <div className="mb-6 flex flex-wrap gap-2">
            <Button onClick={() => handleQuickAction('create-skills', 'create skills')} variant="primary">
              <Sparkles className="w-4 h-4 mr-2" />
              创建Skills
            </Button>
            <Button onClick={() => handleQuickAction('find-skills', 'find skills')} variant="secondary">
              <Search className="w-4 h-4 mr-2" />
              查找Skills
            </Button>
          </div>
        </>
      )}

      {/* 语义搜索 Tab */}
      {activeTab === 'semantic' && (
        <Card className="mb-6">
          <CardHeader>
            <CardTitle className="text-base flex items-center gap-2">
              <Brain className="w-4 h-4" />
              语义搜索技能
            </CardTitle>
          </CardHeader>
          <CardContent>
            <SemanticSearch
              onSelectSkill={(name) => {
                setSearchText(name);
                setActiveTab('installed');
              }}
              onRunSkill={(name) => {
                const skill = skills?.find((s) => s.name === name);
                if (skill) {
                  setRunningSkill(skill);
                }
              }}
            />
          </CardContent>
        </Card>
      )}
      {/* 市场搜索 Tab */}
      {activeTab === 'search' && (
        <Card className="mb-6">
          <CardHeader className="pb-2">
            <CardTitle className="text-base flex items-center gap-2">
              <Search className="w-4 h-4" />
              技能市场搜索与安装
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex gap-2">
              <Input
                value={registryQuery}
                onChange={(e) => setRegistryQuery(e.target.value)}
                placeholder="输入关键字搜索技能市场"
              />
              <Button onClick={handleRegistrySearch} loading={searching}>
                搜索
              </Button>
            </div>
            <div className="flex gap-2">
              <Input
                value={installUrl}
                onChange={(e) => setInstallUrl(e.target.value)}
                placeholder="通过Git URL安装技能（https://...git）"
              />
              <Button onClick={handleInstallByUrl} variant="secondary">
                <Download className="w-4 h-4 mr-2" />
                导入安装
              </Button>
            </div>
            {registryResults.length > 0 && (
              <div className="space-y-2 max-h-56 overflow-y-auto">
                {registryResults.map((item) => (
                  <div key={item.name} className="border rounded-md p-3 dark:border-gray-700 flex justify-between items-center gap-3">
                    <div className="min-w-0">
                      <div className="font-semibold truncate">{item.display_name || item.name}</div>
                      <div className="text-xs text-gray-500 truncate">{item.description}</div>
                    </div>
                    <Button size="sm" onClick={() => handleInstall(item)}>
                      安装
                    </Button>
                  </div>
                ))}
              </div>
            )}
            {registryResults.length > 0 && (
              <div className="text-xs text-gray-500">搜索来源：{searchSource || 'unknown'}</div>
            )}
          </CardContent>
        </Card>
      )}

      {/* Top 100 Tab */}
      {activeTab === 'top' && (
        <Card className="mb-6">
          <CardHeader className="pb-2">
            <CardTitle className="text-base flex items-center justify-between">
              <span>Skills.sh Top 100</span>
              <div className="flex gap-2">
                <Button size="sm" variant={topSort === 'installs' ? 'primary' : 'secondary'} onClick={() => setTopSort('installs')}>
                  按安装量
                </Button>
                <Button size="sm" variant={topSort === 'name' ? 'primary' : 'secondary'} onClick={() => setTopSort('name')}>
                  按名称
                </Button>
              </div>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="mb-3 flex flex-wrap items-center gap-2">
              <Input
                value={String(bulkCount)}
                onChange={(e) => setBulkCount(Number.isNaN(Number(e.target.value)) ? 10 : Number(e.target.value))}
                className="w-24"
                placeholder="10"
              />
              <Button onClick={handleBulkImportTop} disabled={bulkImporting || topSkills.length === 0}>
                {bulkImporting ? '导入中...' : `导入前${Math.max(1, Math.min(100, bulkCount))}`}
              </Button>
              <Button variant="secondary" onClick={handleRetryFailed} disabled={bulkImporting || bulkFailedItems.length === 0}>
                重试失败({bulkFailedItems.length})
              </Button>
            </div>
            {(bulkProgress.total > 0 || bulkImporting) && (
              <div className="mb-3 text-xs text-gray-500">
                进度 {bulkProgress.done}/{bulkProgress.total}，成功 {bulkProgress.success}，失败 {bulkProgress.failed}
              </div>
            )}
            {topSkills.length === 0 ? (
              <div className="text-sm text-gray-500">暂无可用Top100数据</div>
            ) : (
              <div className="space-y-2 max-h-80 overflow-y-auto">
                {topPageItems.map((item, idx) => (
                  <div key={`${item.source}-${item.skill_id}-${idx}`} className="border rounded-md p-2 dark:border-gray-700 flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <div className="text-sm font-medium truncate">#{(topPage - 1) * topPageSize + idx + 1} {item.name}</div>
                      <div className="text-xs text-gray-500 truncate">{item.source} / {item.skill_id} / installs {item.installs}</div>
                    </div>
                    <div className="flex gap-2">
                      <Button size="sm" onClick={() => handleInstallTopSkill(item)}>一键安装</Button>
                      <Button size="sm" variant="ghost" onClick={() => fillInstallUrlFromTop(item)}>填入安装</Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
            {topSkills.length > 0 && (
              <div className="mt-3 flex justify-between items-center">
                <Button size="sm" variant="secondary" disabled={topPage <= 1} onClick={() => setTopPage((p) => Math.max(1, p - 1))}>上一页</Button>
                <span className="text-xs text-gray-500">第 {topPage} / {topTotalPages} 页</span>
                <Button size="sm" variant="secondary" disabled={topPage >= topTotalPages} onClick={() => setTopPage((p) => Math.min(topTotalPages, p + 1))}>下一页</Button>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* 已安装技能列表 */}
      {activeTab === 'installed' && (
        <>
          {loading && !skills ? (
            <div className="flex justify-center p-10">
              <Loader2 className="w-8 h-8 animate-spin text-blue-500" />
            </div>
          ) : filteredSkills.length === 0 ? (
            <EmptyState 
                title="No skills found" 
                description="Install skills or plugins to see them here." 
                icon={<Terminal className="w-12 h-12 text-gray-300" />}
            />
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
              {filteredSkills.map((skill) => (
                <Card key={skill.name} className="hover:shadow-lg transition-shadow">
                  <CardHeader className="pb-3">
                    <div className="flex justify-between items-start">
                        <div className="flex items-center gap-2">
                          <input
                            type="checkbox"
                            checked={selectedSkills.includes(skill.name)}
                            onChange={() => toggleSelect(skill.name)}
                          />
                          <CardTitle className="text-lg">{skill.name}</CardTitle>
                        </div>
                        {skill.enabled ? (
                            <CheckCircle className="w-5 h-5 text-green-500" />
                        ) : (
                            <XCircle className="w-5 h-5 text-gray-300" />
                        )}
                    </div>
                    <div className="text-xs text-gray-500 dark:text-gray-400 font-mono mt-1">
                        v{skill.version}
                    </div>
                  </CardHeader>
                  <CardContent>
                    <p className="text-sm text-gray-600 dark:text-gray-300 min-h-[40px]">
                      {skill.description || "No description provided."}
                    </p>
                    
                    {/* 运行结果展示 */}
                    {runResults[skill.name] && (
                      <div className={`mt-2 p-2 rounded text-xs ${runResults[skill.name].success ? 'bg-green-50 text-green-700 dark:bg-green-900/20' : 'bg-red-50 text-red-700 dark:bg-red-900/20'}`}>
                        <div className="flex items-center gap-1">
                          {runResults[skill.name].success ? <CheckCircle className="w-3 h-3" /> : <XCircle className="w-3 h-3" />}
                          <span>上次运行: {runResults[skill.name].execution_time_ms}ms</span>
                        </div>
                      </div>
                    )}
                    
                    <div className="mt-4 flex justify-end gap-2">
                        <Button size="sm" variant="secondary" onClick={() => setViewingLogsSkill(skill)}>
                          <Clock className="w-4 h-4 mr-1" />
                          日志
                        </Button>
                        <Button size="sm" variant="primary" onClick={() => setRunningSkill(skill)}>
                          <PlayCircle className="w-4 h-4 mr-1" />
                          运行
                        </Button>
                        <Button size="sm" variant={skill.enabled ? 'secondary' : 'primary'} onClick={() => handleToggle(skill)}>
                          <Wrench className="w-4 h-4 mr-1" />
                          {skill.enabled ? '禁用' : '启用'}
                        </Button>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
          {batchTestExecuted && (
            <Card className="mt-6">
              <CardHeader className="pb-2">
                <CardTitle className="text-base">批量测试结果</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                {batchResults.length === 0 ? (
                  <div className="text-sm text-red-500">未返回任何测试结果</div>
                ) : (
                  batchResults.map((r) => (
                    <div key={r.name} className="text-sm flex justify-between border-b py-1 dark:border-gray-700">
                      <span>{r.name}</span>
                      <span className={r.passed ? 'text-green-600' : 'text-red-500'}>
                        {r.passed ? '通过' : `未通过 (installed=${String(r.installed)}, enabled=${String(r.enabled)})`}
                      </span>
                    </div>
                  ))
                )}
              </CardContent>
            </Card>
          )}
        </>
      )}

      {/* 运行技能弹窗 */}
      <SkillRunner
        skill={runningSkill || undefined}
        isOpen={!!runningSkill}
        onClose={() => setRunningSkill(null)}
        onRun={handleRunComplete}
      />

      {/* 查看日志弹窗 */}
      <SkillLogs
        skill={viewingLogsSkill || undefined}
        isOpen={!!viewingLogsSkill || viewingAllLogs}
        onClose={() => {
          setViewingLogsSkill(null);
          setViewingAllLogs(false);
        }}
      />
    </div>
  );
};
