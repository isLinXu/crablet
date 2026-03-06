import { memo } from 'react';
import { Handle, Position, type NodeProps, type Node } from '@xyflow/react';
import { Bot, User } from 'lucide-react';
import clsx from 'clsx';
import { type NodeData } from '../../../store/canvasStore';

const AgentNode = ({ data }: NodeProps<Node<NodeData>>) => {
  const isUser = data.label === 'User' || data.label === 'user_proxy';
  
  return (
    <div className={clsx(
      "px-4 py-2 shadow-md rounded-md border-2 bg-white min-w-[150px]",
      isUser ? "border-blue-500" : "border-purple-500"
    )}>
      <Handle type="target" position={Position.Top} className="w-16 !bg-gray-400" />
      
      <div className="flex items-center">
        <div className={clsx(
          "rounded-full w-8 h-8 flex justify-center items-center mr-2",
          isUser ? "bg-blue-100 text-blue-600" : "bg-purple-100 text-purple-600"
        )}>
          {isUser ? <User size={16} /> : <Bot size={16} />}
        </div>
        <div>
          <div className="text-sm font-bold">{data.label}</div>
          <div className="text-xs text-gray-500">{data.status || 'Active'}</div>
        </div>
      </div>

      <Handle type="source" position={Position.Bottom} className="w-16 !bg-gray-400" />
    </div>
  );
};

export default memo(AgentNode);