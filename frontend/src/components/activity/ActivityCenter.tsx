import React from 'react';
import { Activity, Bot, Workflow } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { cognitiveLayerLabel } from '@/utils/cognitive';
import { useActivityState } from './useActivityState';
import { parseAllocatorDecision, parseRagObservation, ragModeLabel, formatSwarmContent } from './activityTypes';

export const ActivityCenter: React.FC = () => {
  const s = useActivityState();

  return (
    <div className="h-full p-6 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2">
          <Activity className="w-6 h-6" /> Activity
        </h1>
        <div className="flex gap-2">
          {([['all', '全部'], ['trace', 'Trace'], ['swarm', 'Swarm'], ['rag', 'RAG'], ['config', '配置']] as const).map(([val, label]) => (
            <Button key={val} variant={s.filter === val ? 'primary' : 'secondary'} size="sm" onClick={() => s.setFilter(val)}>{label}</Button>
          ))}
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-6 gap-4 mb-6">
        {[
          { label: '会话数', value: s.sessions.length },
          { label: 'Trace 事件', value: s.traceCount },
          { label: 'Swarm 事件', value: s.swarmCount },
          { label: '层级切换', value: s.switchCount },
          { label: '配置事件', value: s.configCount },
          { label: 'RAG 检索', value: s.ragCount },
        ].map(({ label, value }) => (
          <Card key={label}><CardContent className="p-4"><div className="text-sm text-gray-500 dark:text-gray-400">{label}</div><div className="text-2xl font-semibold">{value}</div></CardContent></Card>
        ))}
      </div>

      {/* RAG Timeline */}
      <Card className="mb-6">
        <CardHeader>
          <div className="flex items-center justify-between gap-2">
            <CardTitle>RAG 流程时间线</CardTitle>
            <Button variant="secondary" size="sm" onClick={s.exportRagSnapshot}>导出RAG快照</Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
            <select value={s.ragSessionFilter} onChange={(e) => s.setRagSessionFilter(e.target.value)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部会话</option>{s.sessions.map((sv) => <option key={sv.id} value={sv.id}>{sv.title}</option>)}
            </select>
            <select value={s.ragRangeFilter} onChange={(e) => s.setRagRangeFilter(e.target.value as any)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部时间</option><option value="24h">近24小时</option><option value="7d">近7天</option><option value="30d">近30天</option>
            </select>
          </div>
          <div className="grid grid-cols-2 md:grid-cols-6 gap-2">
            {[
              { label: '检索总数', value: s.ragMetrics.total }, { label: '命中数', value: s.ragMetrics.hit },
              { label: '未命中', value: s.ragMetrics.miss }, { label: '命中率', value: `${s.ragMetrics.hitRate}%` },
              { label: 'GraphRAG', value: s.ragMetrics.graph }, { label: 'Semantic', value: s.ragMetrics.semantic },
            ].map(({ label, value }) => (
              <div key={label} className="rounded border border-gray-200 dark:border-gray-700 p-2">
                <div className="text-[11px] text-gray-500">{label}</div><div className="text-sm font-semibold">{value}</div>
              </div>
            ))}
          </div>
          {s.ragTimeline.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无RAG流程数据</div>
          ) : (
            <div className="space-y-2">
              {s.ragTimeline.slice(0, 80).map((item, idx) => (
                <div key={`${item.sessionId}-${item.time}-${idx}`} className="rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800">
                  <div className="flex items-center justify-between">
                    <div className="text-xs font-medium text-gray-800 dark:text-gray-100">{item.title}</div>
                    <div className="text-[11px] text-gray-500">{new Date(item.time).toLocaleString()}</div>
                  </div>
                  <div className="mt-1 text-xs text-gray-600 dark:text-gray-300">模式 {ragModeLabel(item.retrieval)} · 命中 {item.refsCount} · Query: {item.query || 'N/A'}</div>
                  {item.graphEntities.length > 0 && <div className="mt-1 text-[11px] text-gray-500">图实体：{item.graphEntities.slice(0, 8).join('、')}</div>}
                  {item.refs.length > 0 && <div className="mt-1 text-[11px] text-gray-500">Top Ref：{item.refs.slice(0, 2).map((r) => `${r.source}(${r.score.toFixed(2)})`).join('，')}</div>}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Cognitive Stats */}
      <Card className="mb-6">
        <CardHeader><CardTitle>认知系统检查</CardTitle></CardHeader>
        <CardContent className="grid grid-cols-1 md:grid-cols-5 gap-3">
          {[
            { label: 'System 1', value: s.cognitiveStats.s1 },
            { label: 'System 2', value: s.cognitiveStats.s2 },
            { label: 'System 3', value: s.cognitiveStats.s3 },
            { label: '未分类', value: s.cognitiveStats.unknown },
            { label: '当前层级', value: cognitiveLayerLabel(s.cognitiveStats.latestLayer as any) },
          ].map(({ label, value }) => (
            <div key={label} className="rounded border border-gray-200 dark:border-gray-700 p-3">
              <div className="text-xs text-gray-500">{label}</div><div className="text-xl font-semibold">{value}</div>
            </div>
          ))}
        </CardContent>
      </Card>

      {/* Layer Switch Timeline */}
      <Card className="mb-6">
        <CardHeader><CardTitle>层级切换时间线</CardTitle></CardHeader>
        <CardContent className="space-y-2">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
            <select value={s.sessionFilter} onChange={(e) => s.setSessionFilter(e.target.value)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部会话</option>{s.sessions.map((sv) => <option key={sv.id} value={sv.id}>{sv.title}</option>)}
            </select>
            <select value={s.layerFilter} onChange={(e) => s.setLayerFilter(e.target.value as any)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部层级</option><option value="system1">System 1</option><option value="system2">System 2</option><option value="system3">System 3</option>
            </select>
            <select value={s.rangeFilter} onChange={(e) => s.setRangeFilter(e.target.value as any)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部时间</option><option value="24h">近24小时</option><option value="7d">近7天</option><option value="30d">近30天</option>
            </select>
          </div>
          {s.timelineByTransition.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无层级切换记录</div>
          ) : (
            s.timelineByTransition.slice(0, 120).map((item, idx) => (
              <div key={`${item.sessionId}-${item.time}-${idx}`} className="rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800">
                <div className="flex items-center justify-between">
                  <div className="text-xs font-medium text-gray-800 dark:text-gray-100">{item.title}</div>
                  <div className="text-[11px] text-gray-500">{new Date(item.time).toLocaleString()}</div>
                </div>
                <div className="mt-1 text-xs text-gray-600 dark:text-gray-300">切换到 <span className="font-semibold">{cognitiveLayerLabel(item.layer as any)}</span> · {item.reason}</div>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      {/* Sankey Flow */}
      <Card className="mb-6">
        <CardHeader><CardTitle>S1/S2/S3迁移流向</CardTitle></CardHeader>
        <CardContent className="space-y-3">
          <div className="text-xs text-gray-500 flex items-center justify-between">
            <span>总迁移量：{s.sankeyLinks.total}</span>
            {s.selectedTransition && <button className="px-2 py-0.5 rounded border border-gray-300 dark:border-gray-700" onClick={() => s.setSelectedTransition(null)}>清除连线过滤</button>}
          </div>
          {s.sankeyLinks.links.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无迁移数据</div>
          ) : (
            <div className="relative rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800 overflow-x-auto">
              <svg width="100%" height="220" viewBox="0 0 760 220" preserveAspectRatio="xMidYMid meet">
                {['system1', 'system2', 'system3'].map((layer, idx) => (
                  <React.Fragment key={`layer-nodes-${layer}`}>
                    <g key={`left-${layer}`}><rect x="24" y={24 + idx * 62} width="90" height="30" rx="6" className="fill-blue-100 dark:fill-blue-900/30" /><text x="69" y={44 + idx * 62} textAnchor="middle" className="fill-gray-700 dark:fill-gray-200 text-[11px]">{cognitiveLayerLabel(layer as any)}</text></g>
                    <g key={`right-${layer}`}><rect x="646" y={24 + idx * 62} width="90" height="30" rx="6" className="fill-emerald-100 dark:fill-emerald-900/30" /><text x="691" y={44 + idx * 62} textAnchor="middle" className="fill-gray-700 dark:fill-gray-200 text-[11px]">{cognitiveLayerLabel(layer as any)}</text></g>
                  </React.Fragment>
                ))}
                {s.sankeyLinks.links.map((link) => {
                  const fromIdx = link.from === 'system1' ? 0 : link.from === 'system2' ? 1 : 2;
                  const toIdx = link.to === 'system1' ? 0 : link.to === 'system2' ? 1 : 2;
                  const y1 = 39 + fromIdx * 62; const y2 = 39 + toIdx * 62;
                  const strokeW = 2 + (link.value / s.sankeyLinks.max) * 12;
                  const selected = s.selectedTransition === `${link.from}->${link.to}`;
                  return (
                    <g key={`${link.from}-${link.to}`}>
                      <path d={`M 114 ${y1} C 260 ${y1}, 500 ${y2}, 646 ${y2}`} fill="none" stroke="currentColor" className={selected ? 'text-fuchsia-500' : 'text-violet-500/70'} strokeWidth={strokeW} strokeLinecap="round"
                        onMouseEnter={(e) => { s.setHoverLink(link); s.setHoverPos({ x: e.clientX, y: e.clientY }); }}
                        onMouseMove={(e) => s.setHoverPos({ x: e.clientX, y: e.clientY })}
                        onMouseLeave={() => { s.setHoverLink(null); s.setHoverPos(null); }}
                        onClick={() => s.setSelectedTransition((prev) => (prev === `${link.from}->${link.to}` ? null : `${link.from}->${link.to}`))}
                        onContextMenu={(e) => { e.preventDefault(); const key = `${link.from}->${link.to}`; s.setPinnedTransitions((prev) => prev.some((p) => p.key === key) ? prev.filter((p) => p.key !== key) : [...prev, { key, from: link.from, to: link.to }]); }}
                        style={{ cursor: 'pointer' }}
                      />
                      <text x="380" y={(y1 + y2) / 2 - 3} textAnchor="middle" className="fill-gray-600 dark:fill-gray-300 text-[10px]">{link.value}</text>
                    </g>
                  );
                })}
              </svg>
              {s.hoverLink && s.hoverPos && (
                <div className="fixed z-50 text-[11px] rounded border border-gray-300 dark:border-gray-700 bg-white/95 dark:bg-gray-900/95 px-2 py-1 shadow-lg min-w-[180px]" style={{ left: s.hoverPos.x + 12, top: s.hoverPos.y + 12 }}>
                  <div className="font-semibold">{cognitiveLayerLabel(s.hoverLink.from as any)} → {cognitiveLayerLabel(s.hoverLink.to as any)} · {s.hoverLink.value}</div>
                  <div className="text-gray-500">占比 {s.sankeyLinks.total > 0 ? Math.round((s.hoverLink.value / s.sankeyLinks.total) * 100) : 0}% · 24h {s.flowMatrix24h[`${s.hoverLink.from}->${s.hoverLink.to}`] || 0}</div>
                  <div className="mt-1 text-gray-500">来源会话</div>
                  {(s.transitionSessions[`${s.hoverLink.from}->${s.hoverLink.to}`] || []).slice(0, 3).map((ss) => (
                    <div key={`${ss.title}-${ss.count}`} className="flex items-center justify-between gap-2"><span className="truncate max-w-[130px]">{ss.title}</span><span>{ss.count}</span></div>
                  ))}
                </div>
              )}
            </div>
          )}
          {/* Pinned transitions */}
          {s.pinnedTransitions.length > 0 && (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              <div className="md:col-span-2 flex justify-end gap-2">
                <button className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-700" onClick={s.copyPinnedSummary}>复制固定卡片摘要</button>
                <button className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-700" onClick={s.exportPinnedCsv}>导出固定卡片CSV</button>
                <button className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-700" onClick={() => s.setPinnedTransitions([])}>清空固定卡片</button>
              </div>
              {s.pinnedTransitions.map((pin) => {
                const value = s.flowMatrix[pin.key] || 0; const ratio = s.sankeyLinks.total > 0 ? Math.round((value / s.sankeyLinks.total) * 100) : 0; const list = s.transitionSessions[pin.key] || [];
                return (
                  <div key={pin.key} className="rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800">
                    <div className="flex items-center justify-between">
                      <div className="text-xs font-semibold">{cognitiveLayerLabel(pin.from as any)} → {cognitiveLayerLabel(pin.to as any)}</div>
                      <button className="text-[11px] px-2 py-0.5 rounded border border-gray-300 dark:border-gray-700" onClick={() => s.setPinnedTransitions((prev) => prev.filter((p) => p.key !== pin.key))}>移除</button>
                    </div>
                    <div className="text-xs text-gray-500 mt-1">当前 {value} · 占比 {ratio}% · 24h {s.flowMatrix24h[pin.key] || 0}</div>
                    <div className="mt-1 space-y-0.5">{list.slice(0, 3).map((ss) => <div key={`${pin.key}-${ss.title}`} className="text-[11px] flex items-center justify-between"><span className="truncate max-w-[180px]">{ss.title}</span><span>{ss.count}</span></div>)}</div>
                  </div>
                );
              })}
            </div>
          )}
          {/* Matrix grid */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
            {['system1', 'system2', 'system3'].map((from) => (
              <div key={from} className="rounded border border-gray-200 dark:border-gray-700 p-2">
                <div className="text-xs text-gray-500 mb-1">{cognitiveLayerLabel(from as any)} 出发</div>
                {['system1', 'system2', 'system3'].map((to) => {
                  const value = s.flowMatrix[`${from}->${to}`] || 0; const ratio = s.sankeyLinks.total > 0 ? Math.round((value / s.sankeyLinks.total) * 100) : 0;
                  return <div key={`${from}-${to}`} className="text-xs flex items-center justify-between"><span>{cognitiveLayerLabel(to as any)}</span><span className="font-semibold">{value} · {ratio}%</span></div>;
                })}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Recent Activity */}
      <Card>
        <CardHeader><CardTitle>最近活动</CardTitle></CardHeader>
        <CardContent className="space-y-3">
          {s.filtered.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无活动数据，可先在 Chat 或 Swarm 中触发一次执行。</div>
          ) : (
            s.filtered.slice(0, 120).map((item, idx) => (
              <div key={`${item.sessionId}-${item.time}-${idx}`} className="rounded border border-gray-200 dark:border-gray-700 p-3 bg-white dark:bg-gray-800">
                <div className="flex items-center justify-between mb-1">
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">{item.title}</div>
                  <div className="text-xs text-gray-500">{new Date(item.time).toLocaleString()}</div>
                </div>
                {item.kind === 'trace' ? (
                  <div className="text-xs text-gray-600 dark:text-gray-300 space-y-1">
                    {item.action === 'rag_retrieve' ? (
                      <div className="space-y-2">
                        <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-fuchsia-100 dark:bg-fuchsia-900/30 text-fuchsia-700 dark:text-fuchsia-300"><Activity className="w-3 h-3" /> RAG 检索流程</div>
                        {(() => { const parsed = parseRagObservation(item.observation); if (!parsed) return <div>{item.thought || item.action || item.observation || '无详细内容'}</div>;
                          return (<div className="space-y-1"><div>检索模式：{parsed.retrieval} · 命中：{parsed.refs_count}</div><div>Query：{item.input || 'N/A'}</div>{parsed.graph_entities.length > 0 && <div>图实体：{parsed.graph_entities.slice(0, 6).join('、')}</div>}{parsed.refs.length > 0 && (<div className="mt-1 rounded border border-fuchsia-200 dark:border-fuchsia-800 p-2 bg-fuchsia-50/60 dark:bg-fuchsia-900/20 space-y-1">{parsed.refs.slice(0, 3).map((ref, refIdx) => (<div key={`${item.sessionId}-${item.time}-${refIdx}`}><span className="font-medium">[{ref.source}] {ref.score.toFixed(2)}</span><span> · {ref.content}</span></div>))}</div>)}</div>);
                        })()}
                      </div>
                    ) : item.action === 'graph_rag_mode_changed' ? (
                      <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-violet-100 dark:bg-violet-900/30 text-violet-700 dark:text-violet-300"><Activity className="w-3 h-3" /> GraphRAG 模式切换</div>
                    ) : (
                      <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300"><Bot className="w-3 h-3" /> Trace</div>
                    )}
                    {item.action !== 'rag_retrieve' && <div>{item.thought || item.action || item.observation || '无详细内容'}</div>}
                  </div>
                ) : (
                  <div className="text-xs text-gray-600 dark:text-gray-300 space-y-1">
                    <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300"><Workflow className="w-3 h-3" /> Swarm</div>
                    <div>{item.from} → {item.to} · {item.eventType}</div>
                    <div>{formatSwarmContent(item.eventType, item.content)}</div>
                    {item.eventType === 'AllocatorDecision' && (() => {
                      const parsed = parseAllocatorDecision(item.eventType, item.content);
                      if (!parsed || parsed.candidates.length === 0) return null;
                      const chartMode = s.getAllocatorMode(item.sessionId);
                      const maxScore = Math.max(...parsed.candidates.map((c) => c.final_score), 0.0001);
                      const maxContribution = Math.max(...parsed.candidates.map((c) => c.expertise_match + c.ucb_bonus + c.performance_bonus + c.preferred_bonus + c.load_penalty), 0.0001);
                      return (
                        <div className="mt-2 rounded border border-gray-200 dark:border-gray-700 p-2 bg-gray-50 dark:bg-gray-900/40 space-y-1">
                          <div className="flex items-center justify-between">
                            <div className="text-[11px] text-gray-500">分配评分细节</div>
                            <div className="inline-flex rounded border border-gray-300 dark:border-gray-700 overflow-hidden">
                              <button className={`px-2 py-0.5 text-[10px] ${s.allocatorChartScope === 'global' ? 'bg-indigo-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`} onClick={() => s.setAllocatorChartScope('global')}>全局</button>
                              <button className={`px-2 py-0.5 text-[10px] border-l border-gray-300 dark:border-gray-700 ${s.allocatorChartScope === 'session' ? 'bg-indigo-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`} onClick={() => s.setAllocatorChartScope('session')}>会话</button>
                            </div>
                            <div className="inline-flex rounded border border-gray-300 dark:border-gray-700 overflow-hidden">
                              <button className={`px-2 py-0.5 text-[10px] ${chartMode === 'bar' ? 'bg-emerald-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`} onClick={() => s.setAllocatorModeFor('bar', item.sessionId)}>单柱</button>
                              <button className={`px-2 py-0.5 text-[10px] border-l border-gray-300 dark:border-gray-700 ${chartMode === 'stacked' ? 'bg-emerald-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`} onClick={() => s.setAllocatorModeFor('stacked', item.sessionId)}>堆叠</button>
                            </div>
                          </div>
                          {parsed.candidates.map((c) => {
                            const width = Math.max(4, Math.round((c.final_score / maxScore) * 100));
                            return (
                              <div key={`${item.sessionId}-${item.time}-${c.role}`} className="space-y-0.5">
                                <div className="flex items-center justify-between text-[11px]"><span className="font-medium">{c.role}</span><span className="text-gray-500">final {c.final_score.toFixed(2)} · match {c.expertise_match.toFixed(2)} · ucb {c.ucb_bonus.toFixed(2)} · perf {c.performance_bonus.toFixed(2)} · pref {c.preferred_bonus.toFixed(2)} · load -{c.load_penalty.toFixed(2)}</span></div>
                                {chartMode === 'bar' ? (
                                  <div className="h-1.5 rounded bg-gray-200 dark:bg-gray-700 overflow-hidden"><div className="h-full bg-emerald-500 dark:bg-emerald-400" style={{ width: `${width}%` }} /></div>
                                ) : (
                                  <div className="space-y-0.5">
                                    <div className="h-1.5 rounded bg-gray-200 dark:bg-gray-700 overflow-hidden flex">
                                      {[[c.expertise_match, 'bg-blue-500', 'match'], [c.ucb_bonus, 'bg-violet-500', 'ucb'], [c.performance_bonus, 'bg-emerald-500', 'perf'], [c.preferred_bonus, 'bg-cyan-500', 'pref'], [c.load_penalty, 'bg-rose-500', 'load']].map(([val, cls, label]: any, i: number) => (
                                        <div key={i} className={`h-full ${cls}`} style={{ width: `${Math.max(1, Math.round((val as number / maxContribution) * 100))}%` }} title={`${label} ${(val as number).toFixed(2)}`} />
                                      ))}
                                    </div>
                                    <div className="text-[10px] text-gray-500">({(c.expertise_match + c.ucb_bonus + c.performance_bonus + c.preferred_bonus).toFixed(2)}) - ({c.load_penalty.toFixed(2)}) = {c.final_score.toFixed(2)}</div>
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      );
                    })()}
                  </div>
                )}
              </div>
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
};
