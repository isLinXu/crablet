import { memo } from 'react';
import { Handle, Position, type NodeProps, type Node } from '@xyflow/react';
import { CheckCircle, AlertCircle, Loader2, Circle } from 'lucide-react';
import clsx from 'clsx';
import { type NodeData } from '../../../store/canvasStore';

const TaskNode = ({ data }: NodeProps<Node<NodeData>>) => {
  const status = (data.status as string) || 'pending';
  
  const statusIcon: Record<string, React.ReactNode> = {
    pending: <Circle size={14} className="text-gray-400" />,
    running: <Loader2 size={14} className="animate-spin text-blue-500" />,
    completed: <CheckCircle size={14} className="text-green-500" />,
    failed: <AlertCircle size={14} className="text-red-500" />,
  };
  
  const statusColor: Record<string, string> = {
    pending: 'border-gray-200 bg-gray-50',
    running: 'border-blue-400 bg-blue-50 animate-pulse',
    completed: 'border-green-400 bg-green-50',
    failed: 'border-red-400 bg-red-50',
  };

  return (
    <div className={clsx(
      "px-3 py-2 shadow-sm rounded-lg border max-w-[200px] text-xs",
      statusColor[status]
    )}>
      <Handle type="target" position={Position.Top} className="!bg-gray-400" />
      
      <div className="flex items-start gap-2">
        <div className="mt-0.5 shrink-0">
            {statusIcon[status]}
        </div>
        <div>
          <div className="font-medium text-gray-800 line-clamp-2">{data.label}</div>
          {data.details && (
            <div className="text-[10px] text-gray-500 mt-1 line-clamp-3 font-mono">{data.details}</div>
          )}
        </div>
      </div>

      <Handle type="source" position={Position.Bottom} className="!bg-gray-400" />
    </div>
  );
};

export default memo(TaskNode);