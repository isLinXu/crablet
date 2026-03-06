import React, { useMemo, useState } from 'react';
import { Card } from '../ui/Card';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import type { HitlReview } from '@/types/domain';

interface HitlReviewPanelProps {
  reviews: HitlReview[];
  loading: boolean;
  onRefresh: () => Promise<void>;
  onApprove: (taskId: string) => Promise<void>;
  onReject: (taskId: string, reason: string) => Promise<void>;
  onEdit: (taskId: string, content: string) => Promise<void>;
  onFeedback: (taskId: string, feedback: string) => Promise<void>;
  onSelect: (taskId: string, index: number) => Promise<void>;
}

type ReviewAction = 'approve' | 'reject' | 'edit' | 'feedback' | 'select';

const toReviewType = (reviewType: HitlReview['review_type']): string => {
  if (typeof reviewType === 'string') return reviewType;
  return String(reviewType?.type || 'Approval');
};

export const HitlReviewPanel: React.FC<HitlReviewPanelProps> = ({
  reviews,
  loading,
  onRefresh,
  onApprove,
  onReject,
  onEdit,
  onFeedback,
  onSelect,
}) => {
  const [actionTaskId, setActionTaskId] = useState<string | null>(null);
  const [actionType, setActionType] = useState<ReviewAction | null>(null);
  const [textValue, setTextValue] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [submitting, setSubmitting] = useState(false);

  const sorted = useMemo(
    () => [...reviews].sort((a, b) => new Date(a.deadline).getTime() - new Date(b.deadline).getTime()),
    [reviews]
  );

  const openAction = (taskId: string, action: ReviewAction) => {
    setActionTaskId(taskId);
    setActionType(action);
    setTextValue('');
    setSelectedIndex(0);
  };

  const closeAction = () => {
    setActionTaskId(null);
    setActionType(null);
    setTextValue('');
    setSelectedIndex(0);
  };

  const submitAction = async () => {
    if (!actionTaskId || !actionType) return;
    setSubmitting(true);
    try {
      if (actionType === 'approve') {
        await onApprove(actionTaskId);
      } else if (actionType === 'reject') {
        await onReject(actionTaskId, textValue || 'Rejected by reviewer');
      } else if (actionType === 'edit') {
        await onEdit(actionTaskId, textValue);
      } else if (actionType === 'feedback') {
        await onFeedback(actionTaskId, textValue);
      } else if (actionType === 'select') {
        await onSelect(actionTaskId, selectedIndex);
      }
      closeAction();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Card className="h-[400px] flex flex-col p-0 overflow-hidden">
      <div className="p-3 border-b bg-gray-50 dark:bg-gray-800 flex justify-between items-center">
        <h3 className="font-semibold text-sm">Human Reviews</h3>
        <Button size="sm" variant="ghost" onClick={() => void onRefresh()} loading={loading}>
          刷新
        </Button>
      </div>
      <div className="flex-1 overflow-y-auto p-3 space-y-3">
        {sorted.length === 0 ? (
          <div className="text-center text-sm text-gray-500 py-12">暂无待审批任务</div>
        ) : (
          sorted.map((review) => {
            const reviewType = toReviewType(review.review_type);
            const isActive = actionTaskId === review.task_id;
            return (
              <div key={review.review_id} className="border rounded-lg p-2 space-y-2 bg-white dark:bg-gray-900/40">
                <div className="flex items-center justify-between gap-2">
                  <div className="text-xs text-gray-500">task {review.task_id.slice(0, 8)} · graph {review.graph_id.slice(0, 8)}</div>
                  <span className="text-[10px] px-2 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300">{reviewType}</span>
                </div>
                <div className="text-xs text-gray-700 dark:text-gray-200 whitespace-pre-wrap line-clamp-3">{review.agent_output}</div>
                <div className="text-[10px] text-gray-500">截止 {new Date(review.deadline).toLocaleString()}</div>
                <div className="flex flex-wrap gap-1">
                  <Button size="sm" onClick={() => openAction(review.task_id, 'approve')}>批准</Button>
                  <Button size="sm" variant="secondary" onClick={() => openAction(review.task_id, 'edit')}>编辑</Button>
                  <Button size="sm" variant="secondary" onClick={() => openAction(review.task_id, 'feedback')}>反馈</Button>
                  <Button size="sm" variant="secondary" onClick={() => openAction(review.task_id, 'select')}>选择</Button>
                  <Button size="sm" variant="danger" onClick={() => openAction(review.task_id, 'reject')}>拒绝</Button>
                </div>
                {isActive && (
                  <div className="border-t pt-2 space-y-2">
                    {(actionType === 'reject' || actionType === 'edit' || actionType === 'feedback') && (
                      <textarea
                        value={textValue}
                        onChange={(e) => setTextValue(e.target.value)}
                        className="w-full min-h-20 rounded-md border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800 px-2 py-1 text-sm"
                        placeholder={actionType === 'edit' ? '输入修改后的输出' : actionType === 'reject' ? '输入拒绝原因' : '输入反馈内容'}
                      />
                    )}
                    {actionType === 'select' && (
                      <div className="space-y-1">
                        {review.options.length > 0 ? (
                          <select
                            value={selectedIndex}
                            onChange={(e) => setSelectedIndex(Number(e.target.value))}
                            className="w-full h-9 rounded-md border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800 px-2 text-sm"
                          >
                            {review.options.map((opt, idx) => (
                              <option key={`${review.review_id}-${idx}`} value={idx}>{idx + 1}. {opt}</option>
                            ))}
                          </select>
                        ) : (
                          <Input
                            type="number"
                            min={0}
                            value={selectedIndex}
                            onChange={(e) => setSelectedIndex(Number(e.target.value || '0'))}
                            placeholder="输入候选索引"
                          />
                        )}
                      </div>
                    )}
                    <div className="flex gap-2">
                      <Button size="sm" onClick={() => void submitAction()} loading={submitting}>提交</Button>
                      <Button size="sm" variant="ghost" onClick={closeAction}>取消</Button>
                    </div>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </Card>
  );
};
